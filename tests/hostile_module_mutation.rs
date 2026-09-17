#![cfg(all(feature = "compile", feature = "verify"))]
//! Systematic mutation of compiled modules across the attacker-supplied
//! bytecode path.
//!
//! # The threat model this exercises
//!
//! `Vm::new` runs `verify` on bytecode the host did not compile. The two
//! documented arrivals are a precompiled artifact and a hot-swapped module,
//! and the crate's own `tests/typed_conformance.rs` names the same model.
//! That file carries **five** mutations, each hand-written to recreate one
//! audit finding. This file is the systematic form, and it carries every
//! mutant through **both** arrivals: a fresh load into a new virtual machine,
//! and a hot swap into one that is already live.
//!
//! Both are needed, and the second was added after the first had run for a
//! while. A hot swap runs compatibility checks a fresh load has no occasion
//! to: the schema hash is compared **only** there, so before the swap stage
//! existed every mutation of that field was accepted and counted as a pass.
//!
//! # What is mutated
//!
//! - Every instruction: operands, opcode substitution, deletion, duplication.
//! - Chunk metadata: local and parameter counts.
//! - The module entry point.
//! - The **module-level descriptor tables** the typed operand-stack pass
//!   (A.2.1) seeds operand shapes from — per-chunk signatures, native return
//!   shapes, and enum layouts — plus the schema hash. Four audit findings (B1,
//!   B2, B6, B8) were about trusting a compiler-baked value an attacker
//!   supplies, and B8 was an enum's payload padding hint specifically. The
//!   instruction mutations exercise the ops that CONSUME these tables; these
//!   exercise the seeding side.
//!
//! # Why the module is mutated and the artifact re-encoded
//!
//! `tests/format_fingerprint.rs` records, from measurement, that a byte-level
//! edit to a serialized module "is caught by a checksum long before the header
//! is read". A byte-flipping fuzz therefore measures the cyclic redundancy
//! check, not the verifier. An attacker constructing bytecode computes a valid
//! checksum as easily as the compiler does, so the faithful model mutates the
//! `Module` and re-encodes around it. The artifact each mutant travels as is
//! consequently well formed at every layer below the one under test.
//!
//! # The invariant
//!
//! For every mutant, one of these holds:
//!
//! - the encoder refuses it (it tested nothing; counted separately);
//! - the loader rejects it;
//! - `verify` rejects it;
//! - a live virtual machine refuses it as a hot swap;
//! - it runs to a result, an error, or a yield, without panicking or hanging.
//!
//! **Nothing here asserts a mutant computes anything in particular.** A
//! hostile module is entitled to produce a wrong answer. It is not entitled to
//! panic, to read out of bounds, or to fail to terminate, because the claim the
//! verifier makes about an untrusted input is a definitive bound.
//!
//! # Reach
//!
//! A run in which every mutant died at the encoder would be green and would
//! say nothing about the verifier. The reach census inside
//! [`a_hostile_module_is_rejected_or_runs_without_panicking_or_hanging`] is
//! therefore part of the contract: it fails when the census collapses, and it
//! prints its counts so a reader need not make it fail to see them. The counts
//! come from the real outcome of each stage, never from searching an error
//! message for a substring, and each mutant carries its family as a field
//! rather than having one parsed back out of its printed identity.
//!
//! **The census is per family, and that is not decoration.** An aggregate
//! count hides a family that produces nothing: at 5184 mutants this file
//! looked healthy while the native-return-shape table had never been touched,
//! because no corpus program called a native. A per-family floor caught it in
//! one run. The same structure is what shows the schema-hash mutants are
//! refused at the swap rather than silently passing.
//!
//! # What this does NOT cover
//!
//! - Signature forgery, covered by the signing tests.
//! - Wire-container corruption, covered by `tests/wire_fuzz.rs` for the
//!   decoders and by `keleusma-wire`'s own suite for the container.
//! - Auxiliary-body structure the `Module` type does not express. The
//!   module-level descriptor tables ARE `Module` fields and are mutated here;
//!   an earlier version of this note said otherwise and was wrong about its
//!   own subject.
//! - A process abort. A stack overflow aborts rather than unwinding, so no
//!   watchdog and no `catch_unwind` can turn it into a named failure; it ends
//!   the harness. The first run of this file did exactly that, and the defect
//!   it found is pinned in `tests/verify_hostile_termination.rs`.
//! - Whether an accepted mutant computes the RIGHT answer. Only that it does
//!   not panic, hang, or read out of bounds.
//!
//! # What it found, and what it did not
//!
//! Its first run found audit H1, a verifier that could be hung or made to
//! abort by one branch operand. Since then it has found no further defect. The
//! descriptor tables are broadly attacker-controllable and the module is still
//! disposed of safely: a wrong flat shape widens what the typed pass will
//! accept, and the runtime bounds guard the pass defers to holds. **That is a
//! result, not an absence of one**, but it is a result about the mutations
//! enumerated here, which is why the census prints what it covered.
//!
//! # Every phase is watchdogged, and that is not the original design
//!
//! The first version of this file watchdogged only execution, reasoning that
//! only execution had a documented reason to be unbounded. That reasoning was
//! wrong in the most direct way available: the first run hung inside
//! `verify`, which is the phase whose entire purpose is to bound an untrusted
//! input. Encoding, loading, sizing and verification are now inside the
//! watchdog with execution.

extern crate alloc;

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::time::Duration;

use keleusma::Arena;
use keleusma::bytecode::{
    ArrayElem, EnumField, Module, NewCompositeOperand, Op, StructField, TupleField, WireShape,
};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::value_layout::{CompositeKind, ScalarKind};
use keleusma::verify::verify;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, required_persistent_capacity_for};
use keleusma::wire_format::{module_from_wire_bytes, module_to_wire_bytes};

/// Wall-clock allowance for one mutant, across every phase.
///
/// Generous on purpose: this is a liveness watchdog, not a performance budget.
/// A mutant that has not finished in this long has stopped making progress in
/// a way the verifier was supposed to have ruled out.
const WATCHDOG: Duration = Duration::from_secs(10);

/// Largest persistent region a mutant may demand before the harness declines
/// to allocate it.
///
/// A mutated data layout can claim an arbitrary region. Honouring the claim
/// would have the harness exhaust memory rather than test anything, so the
/// claim is refused and counted. Refusing is not a verdict about the module:
/// a host sizes its own arena and would refuse the same way.
const PERSISTENT_CAP: usize = 1 << 20;

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------

/// Programs whose compiled form the mutations are applied to.
///
/// Chosen for opcode variety rather than for size: between them they emit
/// composite construction and field reads in flat and nested form, array
/// indexing, enum discrimination, tuple access, a bounded loop, a call, and
/// checked arithmetic. Each returns a `Word` and takes no argument, so a
/// mutant can be driven with an empty argument list.
const CORPUS: &[(&str, &str)] = &[
    (
        "arith",
        "fn main() -> Word {\n\
         \x20 let a = 7;\n\
         \x20 let b = a * 3 - 4;\n\
         \x20 if b > 10 { b / 2 } else { b + 1 }\n\
         }",
    ),
    (
        "struct_field",
        "struct Point { x: Word, y: Word }\n\
         fn main() -> Word {\n\
         \x20 let p = Point { x: 3, y: 4 };\n\
         \x20 p.x + p.y\n\
         }",
    ),
    (
        "nested_struct",
        "struct Inner { a: Word, b: Word }\n\
         struct Outer { i: Inner, c: Word }\n\
         fn main() -> Word {\n\
         \x20 let o = Outer { i: Inner { a: 1, b: 2 }, c: 3 };\n\
         \x20 o.i.a + o.i.b + o.c\n\
         }",
    ),
    (
        "array_loop_data",
        "private data d { total: Word }\n\
         fn main() -> Word {\n\
         \x20 let xs = [1, 2, 3, 4];\n\
         \x20 for i in 0..4 { d.total = d.total + xs[i]; }\n\
         \x20 d.total\n\
         }",
    ),
    (
        "enum_match",
        "enum Shape { Dot, Line(Word), Box(Word, Word) }\n\
         fn area(s: Shape) -> Word {\n\
         \x20 match s { Shape::Dot => 0, Shape::Line(n) => n, Shape::Box(w, h) => w * h }\n\
         }\n\
         fn main() -> Word { area(Shape::Box(3, 4)) + area(Shape::Line(5)) }",
    ),
    (
        "tuple_call",
        "fn split(n: Word) -> (Word, Word) { (n / 2, n % 2) }\n\
         fn main() -> Word {\n\
         \x20 let t = split(9);\n\
         \x20 t.0 + t.1\n\
         }",
    ),
    // A COROUTINE, because without one the Stream, Reset and Yield opcodes and
    // the productivity classification receive mutants only by accident. H1 was
    // found in `compute_always_yielding`, which exists for Stream chunks, and
    // was reached only because that walk runs over every chunk in the module.
    (
        "stream_yield",
        "loop main(seed: Word) -> Word {\n\
         \x20 let a = seed + 1;\n\
         \x20 if a > 3 { yield a } else { yield seed }\n\
         }",
    ),
    // A NATIVE call, because without one `native_return_shapes` is empty and
    // that whole descriptor table receives no mutant. The per-family census
    // caught exactly this: the aggregate looked healthy at 5184 mutants while
    // one table had never been touched.
    (
        "native_call",
        "use math::sqrt\n\
         fn main() -> Float { math::sqrt(9.0) }",
    ),
    (
        "stream_delegating",
        "fn bump(n: Word) -> Word { n + 1 }\n\
         loop main(seed: Word) -> Word {\n\
         \x20 let a = bump(seed);\n\
         \x20 yield a\n\
         }",
    ),
];

/// Compiles a corpus program, naming it if it fails.
///
/// The name matters: a corpus entry that stops compiling reports a parse span
/// and nothing else, and the span alone does not say which of the programs it
/// belongs to.
fn compile_source(prog: &str, src: &str) -> Module {
    let tokens =
        tokenize(src).unwrap_or_else(|e| panic!("corpus program {prog} failed to lex: {e:?}"));
    let program =
        parse(&tokens).unwrap_or_else(|e| panic!("corpus program {prog} failed to parse: {e:?}"));
    compile(&program).unwrap_or_else(|e| panic!("corpus program {prog} failed to compile: {e:?}"))
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

/// Values a numeric operand is rewritten to.
///
/// Each is a boundary rather than a sample: zero, one past a plausible count,
/// and the field maximum. A pool index, a local slot, a byte offset and a
/// jump target are all reached by the same set, because what matters is
/// whether the operand is validated at all, not which specific illegal value
/// is used.
const OPERAND_VALUES: &[u16] = &[0, 1, 2, 255, 4096, u16::MAX];

/// Rewrites the first numeric operand of `op`, where it has one.
///
/// Returns `None` for an operand-free opcode, which the substitution and
/// structural mutations cover instead. The rewrite is deliberately blunt: it
/// does not try to keep the operand meaningful, because an operand that stays
/// meaningful tests nothing.
fn rewrite_operand(op: &Op, v: u16) -> Option<Op> {
    let byte = (v & 0xff) as u8;
    Some(match *op {
        Op::Const(_) => Op::Const(v),
        Op::GetLocal(_) => Op::GetLocal(v),
        Op::SetLocal(_) => Op::SetLocal(v),
        Op::GetData(_) => Op::GetData(v as u32),
        Op::SetData(_) => Op::SetData(v as u32),
        Op::GetDataIndexed(_, n) => Op::GetDataIndexed(v as u32, n),
        Op::SetDataIndexed(_, n) => Op::SetDataIndexed(v as u32, n),
        Op::BoundsCheck(_) => Op::BoundsCheck(v),
        Op::If(_) => Op::If(v),
        Op::Else(_) => Op::Else(v),
        Op::Loop(_) => Op::Loop(v),
        Op::EndLoop(_) => Op::EndLoop(v),
        Op::Break(_) => Op::Break(v),
        Op::BreakIf(_) => Op::BreakIf(v),
        Op::Call(_, n) => Op::Call(v, n),
        Op::CallVerifiedNative(_, n) => Op::CallVerifiedNative(v, n),
        Op::CallExternalNative(_, n) => Op::CallExternalNative(v, n),
        Op::IsEnum(_, b, c) => Op::IsEnum(v, b, c),
        Op::IsStruct(_) => Op::IsStruct(v),
        Op::Trap(_) => Op::Trap(v),
        Op::WordToFixed(_) => Op::WordToFixed(byte),
        Op::FixedToWord(_) => Op::FixedToWord(byte),
        Op::FixedMul(_) => Op::FixedMul(byte),
        Op::FixedDiv(_) => Op::FixedDiv(byte),
        Op::CheckedMul(_) => Op::CheckedMul(byte),
        Op::CheckedDiv(_) => Op::CheckedDiv(byte),
        Op::PushImmediate(_) => Op::PushImmediate(byte),
        Op::PopN(_) => Op::PopN(byte),
        Op::GetField(f) => Op::GetField(match f {
            StructField::Flat { kind, .. } => StructField::Flat { offset: v, kind },
            StructField::FlatNested { size, variant, .. } => StructField::FlatNested {
                offset: v,
                size,
                variant,
            },
            StructField::Boxed { .. } => StructField::Boxed { name_const: v },
        }),
        Op::GetTupleField(f) => Op::GetTupleField(match f {
            TupleField::Flat { kind, .. } => TupleField::Flat { offset: v, kind },
            TupleField::FlatNested { size, variant, .. } => TupleField::FlatNested {
                offset: v,
                size,
                variant,
            },
            TupleField::Boxed { .. } => TupleField::Boxed { index: byte },
        }),
        Op::GetEnumField(f) => Op::GetEnumField(match f {
            EnumField::Flat { kind, .. } => EnumField::Flat { offset: v, kind },
            EnumField::FlatNested { size, variant, .. } => EnumField::FlatNested {
                offset: v,
                size,
                variant,
            },
            EnumField::Boxed { .. } => EnumField::Boxed { index: byte },
        }),
        Op::GetIndex(e) => Op::GetIndex(match e {
            ArrayElem::Flat { kind } => ArrayElem::Flat { kind },
            ArrayElem::FlatNested { variant, .. } => ArrayElem::FlatNested { size: v, variant },
            ArrayElem::Boxed => ArrayElem::Boxed,
        }),
        Op::NewComposite(c) => Op::NewComposite(match c {
            NewCompositeOperand::Flat {
                kind, byte_size, ..
            } => NewCompositeOperand::Flat {
                kind,
                count: v,
                byte_size,
            },
            NewCompositeOperand::Boxed { kind, meta, .. } => NewCompositeOperand::Boxed {
                kind,
                count: v,
                meta,
            },
        }),
        _ => return None,
    })
}

/// Rewrites the *second* operand where one exists.
///
/// Separated from [`rewrite_operand`] because the two carry different meanings
/// — an argument count beside a callee index, a body size beside an offset —
/// and a mutation that moved both at once could not be attributed to either.
fn rewrite_second_operand(op: &Op, v: u16) -> Option<Op> {
    let byte = (v & 0xff) as u8;
    Some(match *op {
        Op::Call(a, _) => Op::Call(a, byte),
        Op::CallVerifiedNative(a, _) => Op::CallVerifiedNative(a, byte),
        Op::CallExternalNative(a, _) => Op::CallExternalNative(a, byte),
        Op::GetDataIndexed(a, _) => Op::GetDataIndexed(a, v as u32),
        Op::SetDataIndexed(a, _) => Op::SetDataIndexed(a, v as u32),
        Op::IsEnum(a, _, c) => Op::IsEnum(a, v, c),
        Op::GetField(StructField::FlatNested {
            offset, variant, ..
        }) => Op::GetField(StructField::FlatNested {
            offset,
            size: v,
            variant,
        }),
        Op::GetTupleField(TupleField::FlatNested {
            offset, variant, ..
        }) => Op::GetTupleField(TupleField::FlatNested {
            offset,
            size: v,
            variant,
        }),
        Op::GetEnumField(EnumField::FlatNested {
            offset, variant, ..
        }) => Op::GetEnumField(EnumField::FlatNested {
            offset,
            size: v,
            variant,
        }),
        Op::NewComposite(NewCompositeOperand::Flat { kind, count, .. }) => {
            Op::NewComposite(NewCompositeOperand::Flat {
                kind,
                count,
                byte_size: v,
            })
        }
        Op::NewComposite(NewCompositeOperand::Boxed { kind, count, .. }) => {
            Op::NewComposite(NewCompositeOperand::Boxed {
                kind,
                count,
                meta: v,
            })
        }
        _ => return None,
    })
}

/// Opcodes substituted in place of an existing one.
///
/// A spread across the structural classes the verifier reasons about: stack
/// arity, control-flow nesting, composite construction and access, the call
/// boundary, and the coroutine boundary. Each carries an operand chosen to be
/// out of range for any program in the corpus, so a substitution that is
/// accepted is accepted on the opcode rather than on a plausible operand.
fn substitutes() -> alloc::vec::Vec<(&'static str, Op)> {
    alloc::vec![
        ("return", Op::Return),
        ("yield", Op::Yield),
        ("reset", Op::Reset),
        ("stream", Op::Stream),
        ("dup", Op::Dup),
        ("add", Op::Add),
        ("not", Op::Not),
        ("len", Op::Len),
        ("popn_far", Op::PopN(200)),
        ("endif", Op::EndIf),
        ("endloop_far", Op::EndLoop(9)),
        ("break_far", Op::Break(9)),
        ("const_far", Op::Const(u16::MAX)),
        ("getlocal_far", Op::GetLocal(u16::MAX)),
        ("setlocal_far", Op::SetLocal(u16::MAX)),
        ("call_far", Op::Call(u16::MAX, 3)),
        ("getdata_far", Op::GetData(u32::MAX)),
        (
            "getfield_far",
            Op::GetField(StructField::Flat {
                offset: u16::MAX,
                kind: ScalarKind::Int,
            }),
        ),
        (
            "newcomposite_far",
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Struct,
                count: u16::MAX,
                byte_size: u16::MAX,
            }),
        ),
    ]
}

/// Flat shapes substituted into the descriptor tables.
///
/// `Top` is the lattice top and is documented as reproducing the unseeded
/// behaviour, so it is the benign end. The rest are wrong in the ways that
/// matter to a pass reading a body at a baked offset: a scalar kind that is
/// not a kind, a composite body claimed larger than any that exists, and one
/// claimed empty.
const SHAPES: &[WireShape] = &[
    WireShape::Top,
    WireShape::Scalar { kind: 0 },
    WireShape::Scalar { kind: 255 },
    WireShape::Flat {
        kind: 0,
        size: u32::MAX,
    },
    WireShape::Flat { kind: 255, size: 0 },
];

/// One mutation applied to one module, with the identity that reproduces it.
struct Mutant {
    /// Stable identity, printable and sufficient to re-derive the mutation
    /// without re-running anything. The harness is enumerative rather than
    /// random, so this is the whole reproduction.
    id: alloc::string::String,
    /// Which family of mutation produced this mutant.
    ///
    /// Carried explicitly rather than parsed back out of [`Self::id`]. Reading
    /// a category out of a formatted string is the same crude-instrument
    /// mistake this suite has paid for elsewhere: it silently miscounts the
    /// moment an identity's shape changes, and a census that miscounts is
    /// worse than no census, because it still reads as evidence.
    family: Family,
    module: Module,
}

/// Mutation families, so the reach census can say which part of the module a
/// mutant attacked rather than only how many there were.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    /// An instruction's operand, opcode, presence or duplication.
    Instruction,
    /// A chunk's local or parameter counts.
    ChunkMeta,
    /// The module's entry point.
    EntryPoint,
    /// A per-chunk signature: a parameter, the return, the resume value, or
    /// the table's own length.
    Signature,
    /// An enum layout: a discriminant, the payload padding hint, or the
    /// variant list.
    EnumLayout,
    /// A native return shape.
    NativeShape,
    /// The flat-layout schema hash.
    SchemaHash,
}

/// Every mutation of every program in the corpus.
///
/// Enumerative and therefore fully deterministic: the identity of a failing
/// mutant names the program, chunk, instruction and mutation, and applying
/// that mutation again reproduces it exactly. There is no seed to record
/// because there is no generator.
fn mutants() -> alloc::vec::Vec<Mutant> {
    let subs = substitutes();
    let mut out = alloc::vec::Vec::new();
    for (prog, src) in CORPUS {
        let base = compile_source(prog, src);
        for (ci, chunk) in base.chunks.iter().enumerate() {
            for oi in 0..chunk.ops.len() {
                let here = chunk.ops[oi];

                for &v in OPERAND_VALUES {
                    if let Some(op) = rewrite_operand(&here, v)
                        && op != here
                    {
                        let mut m = base.clone();
                        m.chunks[ci].ops[oi] = op;
                        out.push(Mutant {
                            family: Family::Instruction,
                            id: alloc::format!("{prog}/c{ci}/op{oi}/operand1={v}"),
                            module: m,
                        });
                    }
                    if let Some(op) = rewrite_second_operand(&here, v)
                        && op != here
                    {
                        let mut m = base.clone();
                        m.chunks[ci].ops[oi] = op;
                        out.push(Mutant {
                            family: Family::Instruction,
                            id: alloc::format!("{prog}/c{ci}/op{oi}/operand2={v}"),
                            module: m,
                        });
                    }
                }

                for (name, op) in &subs {
                    if *op == here {
                        continue;
                    }
                    let mut m = base.clone();
                    m.chunks[ci].ops[oi] = *op;
                    out.push(Mutant {
                        family: Family::Instruction,
                        id: alloc::format!("{prog}/c{ci}/op{oi}/sub={name}"),
                        module: m,
                    });
                }

                let mut deleted = base.clone();
                deleted.chunks[ci].ops.remove(oi);
                out.push(Mutant {
                    family: Family::Instruction,
                    id: alloc::format!("{prog}/c{ci}/op{oi}/delete"),
                    module: deleted,
                });

                let mut doubled = base.clone();
                doubled.chunks[ci].ops.insert(oi, here);
                out.push(Mutant {
                    family: Family::Instruction,
                    id: alloc::format!("{prog}/c{ci}/op{oi}/duplicate"),
                    module: doubled,
                });
            }

            // Chunk metadata, which the instruction stream is validated
            // against. A local count that no longer covers the slots the ops
            // address is the cheapest way to reach the slot-bounds logic
            // without touching an instruction.
            for &v in &[0u16, 1, u16::MAX] {
                if chunk.local_count != v {
                    let mut m = base.clone();
                    m.chunks[ci].local_count = v;
                    out.push(Mutant {
                        family: Family::ChunkMeta,
                        id: alloc::format!("{prog}/c{ci}/local_count={v}"),
                        module: m,
                    });
                }
            }
            for &v in &[0u8, 1, u8::MAX] {
                if chunk.param_count != v {
                    let mut m = base.clone();
                    m.chunks[ci].param_count = v;
                    out.push(Mutant {
                        family: Family::ChunkMeta,
                        id: alloc::format!("{prog}/c{ci}/param_count={v}"),
                        module: m,
                    });
                }
            }
        }

        // ------------------------------------------------------------------
        // Module-level DESCRIPTOR TABLES.
        //
        // The typed operand-stack pass (A.2.1) reconstructs each operand's
        // flat shape by seeding from three additive tables: per-chunk
        // signatures, native return shapes, and enum layouts. Four audit
        // findings (B1, B2, B6, B8) were about trusting a compiler-baked value
        // an attacker supplies, and B8 was `min_payload` specifically. The
        // instruction mutations above exercise the ops that CONSUME these
        // tables; these exercise the seeding side, which nothing did.
        // ------------------------------------------------------------------

        for (si, sig) in base.signatures.iter().enumerate() {
            for (wi, w) in SHAPES.iter().enumerate() {
                let mut m = base.clone();
                m.signatures[si].ret = *w;
                out.push(Mutant {
                    family: Family::Signature,
                    id: alloc::format!("{prog}/sig{si}/ret=shape{wi}"),
                    module: m,
                });
                let mut m = base.clone();
                m.signatures[si].resume = *w;
                out.push(Mutant {
                    family: Family::Signature,
                    id: alloc::format!("{prog}/sig{si}/resume=shape{wi}"),
                    module: m,
                });
                for pi in 0..sig.params.len() {
                    let mut m = base.clone();
                    m.signatures[si].params[pi] = *w;
                    out.push(Mutant {
                        family: Family::Signature,
                        id: alloc::format!("{prog}/sig{si}/param{pi}=shape{wi}"),
                        module: m,
                    });
                }
            }
            // An extra parameter the callee does not have, and one fewer than
            // it does: the arity the pass checks a `Call`'s arguments against.
            let mut m = base.clone();
            m.signatures[si].params.push(WireShape::Scalar { kind: 0 });
            out.push(Mutant {
                family: Family::Signature,
                id: alloc::format!("{prog}/sig{si}/param_added"),
                module: m,
            });
            if !sig.params.is_empty() {
                let mut m = base.clone();
                m.signatures[si].params.pop();
                out.push(Mutant {
                    family: Family::Signature,
                    id: alloc::format!("{prog}/sig{si}/param_dropped"),
                    module: m,
                });
            }
        }

        // The whole signature table removed, and one entry short of the chunk
        // count. An absent table is documented as reproducing the unseeded
        // behaviour, so this is a claim under test rather than an attack.
        for (what, f) in [("sigs_cleared", 0usize), ("sigs_truncated", 1usize)] {
            if base.signatures.len() >= f {
                let mut m = base.clone();
                m.signatures
                    .truncate(base.signatures.len().saturating_sub(if f == 0 {
                        base.signatures.len()
                    } else {
                        f
                    }));
                out.push(Mutant {
                    family: Family::Instruction,
                    id: alloc::format!("{prog}/{what}"),
                    module: m,
                });
            }
        }

        for (ei, layout) in base.enum_layouts.iter().enumerate() {
            // B8: `min_payload` is the padding hint a flat enum's fixed body
            // size is computed from, so it decides where every payload field
            // is read.
            for &v in &[0u32, 1, 7, u32::MAX] {
                if layout.min_payload != v {
                    let mut m = base.clone();
                    m.enum_layouts[ei].min_payload = v;
                    out.push(Mutant {
                        family: Family::EnumLayout,
                        id: alloc::format!("{prog}/enum{ei}/min_payload={v}"),
                        module: m,
                    });
                }
            }
            for vi in 0..layout.variants.len() {
                for &d in &[-1i64, 0, 1, i64::MAX] {
                    if layout.variants[vi].disc != d {
                        let mut m = base.clone();
                        m.enum_layouts[ei].variants[vi].disc = d;
                        out.push(Mutant {
                            family: Family::EnumLayout,
                            id: alloc::format!("{prog}/enum{ei}/variant{vi}/disc={d}"),
                            module: m,
                        });
                    }
                }
                let mut m = base.clone();
                m.enum_layouts[ei].variants.remove(vi);
                out.push(Mutant {
                    family: Family::EnumLayout,
                    id: alloc::format!("{prog}/enum{ei}/variant{vi}/removed"),
                    module: m,
                });
            }
            let mut m = base.clone();
            m.enum_layouts[ei].variants.clear();
            out.push(Mutant {
                family: Family::EnumLayout,
                id: alloc::format!("{prog}/enum{ei}/variants_cleared"),
                module: m,
            });
        }
        if !base.enum_layouts.is_empty() {
            let mut m = base.clone();
            m.enum_layouts.clear();
            out.push(Mutant {
                family: Family::EnumLayout,
                id: alloc::format!("{prog}/enum_layouts_cleared"),
                module: m,
            });
        }

        for (ni, _) in base.native_return_shapes.iter().enumerate() {
            for (wi, w) in SHAPES.iter().enumerate() {
                let mut m = base.clone();
                m.native_return_shapes[ni] = *w;
                out.push(Mutant {
                    family: Family::NativeShape,
                    id: alloc::format!("{prog}/native{ni}=shape{wi}"),
                    module: m,
                });
            }
        }

        // The schema hash. **It is not a load-time fingerprint**, which an
        // earlier version of this comment claimed: it is
        // `compute_schema_hash(data_layout)` and the only place it is compared
        // is the HOT SWAP, where the running module's hash must match the
        // incoming one. So a fresh load of a module carrying any hash at all
        // is correct behaviour, and these mutants earn their keep at the swap
        // stage below rather than here.
        for &h in &[0u32, 1, u32::MAX] {
            if base.schema_hash != h {
                let mut m = base.clone();
                m.schema_hash = h;
                out.push(Mutant {
                    family: Family::SchemaHash,
                    id: alloc::format!("{prog}/schema_hash={h}"),
                    module: m,
                });
            }
        }

        // Module-level metadata: an entry point naming a chunk that is not
        // there, or naming a different real chunk, is what a hand-built
        // artifact most easily gets wrong.
        for v in [0usize, base.chunks.len(), base.chunks.len() + 7] {
            if base.entry_point != Some(v) {
                let mut m = base.clone();
                m.entry_point = Some(v);
                out.push(Mutant {
                    family: Family::EntryPoint,
                    id: alloc::format!("{prog}/entry_point={v}"),
                    module: m,
                });
            }
        }
        let mut no_entry = base.clone();
        no_entry.entry_point = None;
        out.push(Mutant {
            family: Family::EntryPoint,
            id: alloc::format!("{prog}/entry_point=none"),
            module: no_entry,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// The pipeline each mutant travels
// ---------------------------------------------------------------------------

/// Where a mutant stopped. Derived from the real result of each stage, never
/// from the text of an error message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    /// The encoder declined to produce an artifact. The mutant tested nothing
    /// below that point and is counted apart from the rejections.
    EncodeRefused,
    /// The mutant demanded more persistent memory than the harness will
    /// allocate. Also not a rejection.
    PersistentRefused,
    /// The loader rejected the artifact.
    LoadRejected,
    /// `verify` rejected the module.
    VerifyRejected,
    /// The module verified and execution returned, yielded, or faulted
    /// cleanly.
    Ran,
    /// The module ran, and was then refused by a live virtual machine's hot
    /// swap.
    SwapRejected,
    /// The module ran, and a live virtual machine accepted it as a hot swap
    /// and then executed it.
    Swapped,
}

/// A breach of the invariant, with the identity that reproduces it.
#[derive(Debug)]
struct Breach {
    id: alloc::string::String,
    what: &'static str,
}

/// Carries one mutant from `Module` to a verdict, entirely inside a watchdog.
///
/// The whole pipeline runs on the worker so that a hang in ANY phase is
/// reported as a named failure rather than stalling the harness. A worker that
/// has not answered is detached rather than joined: a thread stuck in a
/// non-terminating walk cannot be asked to stop, and joining it would make the
/// harness inherit the hang it exists to report.
fn drive(m: Mutant) -> Result<(Family, Stage), Breach> {
    let id = m.id;
    let family = m.family;
    let module = m.module;

    let (tx, rx) = mpsc::channel();
    let _worker = std::thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(|| pipeline(module)));
        let _ = tx.send(outcome);
    });

    match rx.recv_timeout(WATCHDOG) {
        Ok(Ok(stage)) => Ok((family, stage)),
        Ok(Err(_)) => Err(Breach {
            id,
            what: "a phase panicked",
        }),
        Err(_) => Err(Breach {
            id,
            what: "no phase finished within the watchdog",
        }),
    }
}

/// Encode, load, size, verify, and — if verification accepted it — run.
///
/// A panic anywhere here is caught by the caller and reported against the
/// mutant's identity. Returning normally means the mutant was disposed of by
/// one of the stages without breaching anything.
fn pipeline(module: Module) -> Stage {
    let Ok(encoded) = module_to_wire_bytes(&module) else {
        return Stage::EncodeRefused;
    };
    let Ok(loaded) = module_from_wire_bytes(&encoded) else {
        return Stage::LoadRejected;
    };
    let need = required_persistent_capacity_for(&loaded);
    if need > PERSISTENT_CAP {
        return Stage::PersistentRefused;
    }
    if verify(&loaded).is_err() {
        return Stage::VerifyRejected;
    }
    let Ok(mut arena) = Arena::try_with_capacity(DEFAULT_ARENA_CAPACITY + need) else {
        return Stage::PersistentRefused;
    };
    if arena.resize_persistent(need).is_err() {
        return Stage::Ran;
    }
    if let Ok(mut vm) = Vm::new(loaded, &arena) {
        let _ = vm.call(&[]);
    }

    // The SECOND untrusted arrival. The verifier's documented job is to
    // protect a hot-swapped module and a precompiled artifact; everything
    // above is the precompiled path only. A hot swap enters a virtual machine
    // that is already live, with a running module whose data layout the
    // incoming one must be compatible with, so it exercises checks the fresh
    // load has no occasion to run -- the schema-hash comparison among them,
    // which is the ONLY place that field is ever compared.
    swap_into_a_live_vm(&encoded)
}

/// Source of the module a hot swap is attempted against.
///
/// Deliberately a different program from anything in the corpus: a swap whose
/// incoming module happens to match the running one tests the easy case, and
/// the hostile case is a module that does not belong here at all.
const SWAP_HOST: &str = "fn main() -> Word { 21 + 21 }";

/// Stands a live virtual machine up and offers it `bytes` as a hot swap.
fn swap_into_a_live_vm(bytes: &[u8]) -> Stage {
    let host = compile_source("swap_host", SWAP_HOST);
    let need = required_persistent_capacity_for(&host);
    if need > PERSISTENT_CAP {
        return Stage::Ran;
    }
    let Ok(mut arena) = Arena::try_with_capacity(DEFAULT_ARENA_CAPACITY + need) else {
        return Stage::Ran;
    };
    if arena.resize_persistent(need).is_err() {
        return Stage::Ran;
    }
    let Ok(mut vm) = Vm::new(host, &arena) else {
        return Stage::Ran;
    };
    let _ = vm.call(&[]);
    // `replace_module` with a separately decoded module rather than
    // `replace_module_from_bytes`, which requires the `signatures` feature.
    // Reaching a gated path from an ungated test is the defect that turned
    // three continuous-integration jobs red earlier in this line's history,
    // and this file is gated on `compile` and `verify` only. The crate's own
    // documentation states the two are equivalent when no verifying key is
    // registered, and the signature layer has its own tests.
    let Ok(incoming) = module_from_wire_bytes(bytes) else {
        return Stage::SwapRejected;
    };
    if vm.replace_module(incoming, Vec::new()).is_err() {
        return Stage::SwapRejected;
    }
    let _ = vm.call(&[]);
    Stage::Swapped
}

/// The whole corpus, driven once, with the census and every breach.
fn census() -> (BTreeMap<(Family, Stage), usize>, alloc::vec::Vec<Breach>) {
    let mut counts: BTreeMap<(Family, Stage), usize> = BTreeMap::new();
    let mut breaches = alloc::vec::Vec::new();
    for m in mutants() {
        match drive(m) {
            Ok(k) => *counts.entry(k).or_insert(0) += 1,
            Err(b) => breaches.push(b),
        }
    }
    (counts, breaches)
}

/// Mutants of `family` that reached `stage`.
fn at(counts: &BTreeMap<(Family, Stage), usize>, family: Family, stage: Stage) -> usize {
    counts.get(&(family, stage)).copied().unwrap_or(0)
}

/// Mutants of `family`, whatever became of them.
fn total_for(counts: &BTreeMap<(Family, Stage), usize>, family: Family) -> usize {
    counts
        .iter()
        .filter(|((f, _), _)| *f == family)
        .map(|(_, n)| *n)
        .sum()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The invariant, and the reach that makes a green result mean something.
///
/// One test rather than two because driving the corpus twice would double the
/// harness's cost for no additional evidence: the census and the breaches come
/// out of the same pass.
#[test]
fn a_hostile_module_is_rejected_or_runs_without_panicking_or_hanging() {
    let (counts, breaches) = census();

    let total: usize = counts.values().sum::<usize>() + breaches.len();
    // Printed rather than only asserted on: the census is the evidence that a
    // green result means anything, and a reader who wants to know what this
    // file covered should not have to make it fail to find out.
    println!("hostile-module mutation census over {total} mutants: {counts:?}");
    assert!(
        total > 500,
        "the mutation enumeration collapsed to {total} mutants; the corpus or the \
         mutation set is not producing what this file claims to test"
    );

    assert!(
        breaches.is_empty(),
        "{} of {total} mutants breached the invariant:\n{}",
        breaches.len(),
        breaches
            .iter()
            .map(|b| alloc::format!("  {} -- {}", b.id, b.what))
            .collect::<alloc::vec::Vec<_>>()
            .join("\n")
    );

    // Reach. A green run in which nothing arrived at the verifier, or nothing
    // survived it, would be evidence about the harness and none about the
    // tree. These are the floors that make the assertion above meaningful;
    // they are deliberately low, because their purpose is to catch a collapse
    // rather than to pin a number that every corpus change would move.
    // Reach, per family. A green run in which a family never arrived at the
    // verifier would be evidence about the harness and none about the tree,
    // and a single total hides exactly that: the instruction family is large
    // enough to satisfy any aggregate floor on its own.
    for family in [
        Family::Instruction,
        Family::ChunkMeta,
        Family::EntryPoint,
        Family::Signature,
        Family::EnumLayout,
        Family::NativeShape,
        Family::SchemaHash,
    ] {
        assert!(
            total_for(&counts, family) > 0,
            "the {family:?} family produced no mutants at all; census {counts:?}"
        );
        assert_eq!(
            at(&counts, family, Stage::EncodeRefused) + at(&counts, family, Stage::LoadRejected),
            0,
            "{family:?} mutants died before the verifier, so they tested nothing there; \
             census {counts:?}"
        );
    }

    // The descriptor tables specifically. These seed the typed operand-stack
    // pass, and the point of mutating them is to reach that pass -- which
    // means being ACCEPTED by verification and then run, not rejected early.
    // If these ever stop running, the tables have started being validated and
    // this file's claim about them needs rewriting rather than reasserting.
    let reached_execution = |f: Family| {
        at(&counts, f, Stage::Ran)
            + at(&counts, f, Stage::Swapped)
            + at(&counts, f, Stage::SwapRejected)
    };
    for family in [Family::Signature, Family::EnumLayout, Family::SchemaHash] {
        assert!(
            reached_execution(family) > 0,
            "no {family:?} mutant reached execution, so the seeding side of the typed \
             pass was not exercised; census {counts:?}"
        );
    }

    // The hot-swap arrival must actually be reached, and both of its outcomes
    // must occur. If every mutant were refused at the swap, the stage would be
    // testing the swap's front door and nothing behind it; if every mutant were
    // accepted, the compatibility checks would not be running at all.
    let swapped: usize = counts
        .iter()
        .filter(|((_, s), _)| *s == Stage::Swapped)
        .map(|(_, n)| *n)
        .sum();
    let swap_rejected: usize = counts
        .iter()
        .filter(|((_, s), _)| *s == Stage::SwapRejected)
        .map(|(_, n)| *n)
        .sum();
    assert!(
        swapped > 0 && swap_rejected > 0,
        "the hot-swap stage saw {swapped} accepted and {swap_rejected} refused; both \
         outcomes must occur or it is not exercising the compatibility checks; \
         census {counts:?}"
    );

    let ran: usize = counts
        .iter()
        .filter(|((_, s), _)| matches!(s, Stage::Ran | Stage::Swapped | Stage::SwapRejected))
        .map(|(_, n)| *n)
        .sum();
    let rejected: usize = counts
        .iter()
        .filter(|((_, s), _)| *s == Stage::VerifyRejected)
        .map(|(_, n)| *n)
        .sum();
    assert!(
        rejected > 50 && ran > 10,
        "census collapsed: {rejected} rejected, {ran} ran; {counts:?}"
    );
}
