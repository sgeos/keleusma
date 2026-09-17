#![cfg(all(feature = "compile", feature = "verify"))]
//! Systematic mutation of compiled modules across the attacker-supplied
//! bytecode path.
//!
//! # The threat model this exercises
//!
//! `Vm::new` runs `verify` on bytecode the host did not compile. The two
//! documented arrivals are a hot-swapped module and a precompiled artifact,
//! and the crate's own `tests/typed_conformance.rs` names the same model.
//! That file carries **five** mutations, each hand-written to recreate one
//! audit finding. This file is the systematic form: it enumerates mutations
//! over every instruction of a corpus of real programs and carries each mutant
//! through the public entry a host would use.
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
//! message for a substring.
//!
//! # What this does NOT cover
//!
//! - Signature forgery, covered by the signing tests.
//! - Wire-container corruption, covered by `tests/wire_fuzz.rs` for the
//!   decoders and by `keleusma-wire`'s own suite for the container.
//! - Mutations of the auxiliary tables that the `Module` type does not
//!   express. This file reaches what a `Module` can hold.
//! - A process abort. A stack overflow aborts rather than unwinding, so no
//!   watchdog and no `catch_unwind` can turn it into a named failure; it ends
//!   the harness. The first run of this file did exactly that, and the defect
//!   it found is pinned in `tests/verify_hostile_termination.rs`.
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
    ArrayElem, EnumField, Module, NewCompositeOperand, Op, StructField, TupleField,
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

/// One mutation applied to one module, with the identity that reproduces it.
struct Mutant {
    /// Stable identity, printable and sufficient to re-derive the mutation
    /// without re-running anything. The harness is enumerative rather than
    /// random, so this is the whole reproduction.
    id: alloc::string::String,
    module: Module,
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
                        id: alloc::format!("{prog}/c{ci}/op{oi}/sub={name}"),
                        module: m,
                    });
                }

                let mut deleted = base.clone();
                deleted.chunks[ci].ops.remove(oi);
                out.push(Mutant {
                    id: alloc::format!("{prog}/c{ci}/op{oi}/delete"),
                    module: deleted,
                });

                let mut doubled = base.clone();
                doubled.chunks[ci].ops.insert(oi, here);
                out.push(Mutant {
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
                        id: alloc::format!("{prog}/c{ci}/param_count={v}"),
                        module: m,
                    });
                }
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
                    id: alloc::format!("{prog}/entry_point={v}"),
                    module: m,
                });
            }
        }
        let mut no_entry = base.clone();
        no_entry.entry_point = None;
        out.push(Mutant {
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
fn drive(m: Mutant) -> Result<Stage, Breach> {
    let id = m.id;
    let module = m.module;

    let (tx, rx) = mpsc::channel();
    let _worker = std::thread::spawn(move || {
        let outcome = catch_unwind(AssertUnwindSafe(|| pipeline(module)));
        let _ = tx.send(outcome);
    });

    match rx.recv_timeout(WATCHDOG) {
        Ok(Ok(stage)) => Ok(stage),
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
    Stage::Ran
}

/// The whole corpus, driven once, with the census and every breach.
fn census() -> (BTreeMap<Stage, usize>, alloc::vec::Vec<Breach>) {
    let mut counts: BTreeMap<Stage, usize> = BTreeMap::new();
    let mut breaches = alloc::vec::Vec::new();
    for m in mutants() {
        match drive(m) {
            Ok(s) => *counts.entry(s).or_insert(0) += 1,
            Err(b) => breaches.push(b),
        }
    }
    (counts, breaches)
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
    let at = |s: Stage| counts.get(&s).copied().unwrap_or(0);
    assert!(
        at(Stage::LoadRejected) + at(Stage::VerifyRejected) + at(Stage::Ran) > total / 2,
        "more than half the mutants never reached the loader; census {counts:?}"
    );
    assert!(
        at(Stage::VerifyRejected) > 50,
        "only {} mutants reached a verification rejection; census {counts:?}",
        at(Stage::VerifyRejected)
    );
    assert!(
        at(Stage::Ran) > 10,
        "only {} mutants were accepted by verification and run, so the execution \
         half of this file tested almost nothing; census {counts:?}",
        at(Stage::Ran)
    );
}
