//! The C5 review's mutation corpus.
//!
//! Every module below is corrupted by every applicable mutation. Each mutant is given to the
//! reference verifier and to the self-hosted validator, at `verify` and at admission, and the
//! self-hosted validator must never accept a mutant the reference rejects. The review record is
//! `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`.
//!
//! The generator follows `tests/hostile_module_mutation.rs`, which drives the reference and the
//! virtual machine and never reaches the self-hosted validator. The families are operand
//! rewrites at boundary values, access-kind rewrites, opcode substitution, deletion,
//! duplication, adjacent transposition, chunk metadata, the module descriptor tables and the
//! data layout. The generator is enumerative, so a mutant's identity reproduces it and there is
//! no seed to record.
//!
//! **What this does not establish.** Agreement on these mutants is evidence about these
//! mutants. It is not a proof that the self-hosted validator is sound for arbitrary bytecode.
//! Both validators read the same host transport tables, such as `analyze_class` and
//! `op_depth_effect`, so a defect in a shared table is invisible to this comparison.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use keleusma::bytecode::{
    ArrayElem, BlockType, EnumField, Module, NewCompositeOperand, Op, StructField, TupleField,
    WireShape,
};
use keleusma::selfhost::{
    VerificationVerdict, admit_module_via_kel, compile_src, verify_module_via_kel,
};
use keleusma::value_layout::{CompositeKind, ScalarKind};

/// Programs chosen for opcode and table variety. Between them they reach flat and nested
/// composites, arrays, enums, tuples, bounded loops with and without a `break`, calls, a
/// native, private and shared data, text, and a stream that yields directly and one that
/// delegates.
const CORPUS: &[(&str, &str)] = &[
    (
        "arith",
        "fn main() -> Word { let a = 7; let b = a * 3 - 4; if b > 10 { b / 2 } else { b + 1 } }",
    ),
    (
        "struct_field",
        "struct Point { x: Word, y: Word } fn main() -> Word { let p = Point { x: 3, y: 4 }; p.x + p.y }",
    ),
    (
        "nested_struct",
        "struct Inner { a: Word, b: Word } struct Outer { i: Inner, c: Word } \
         fn main() -> Word { let o = Outer { i: Inner { a: 1, b: 2 }, c: 3 }; o.i.a + o.i.b + o.c }",
    ),
    (
        "array_loop_data",
        "private data d { total: Word } fn main() -> Word { let xs = [1, 2, 3, 4]; \
         for i in 0..4 { d.total = d.total + xs[i]; } d.total }",
    ),
    (
        "enum_match",
        "enum Shape { Dot, Line(Word), Box(Word, Word) } \
         fn area(s: Shape) -> Word { match s { Shape::Dot => 0, Shape::Line(n) => n, Shape::Box(w, h) => w * h } } \
         fn main() -> Word { area(Shape::Box(3, 4)) + area(Shape::Line(5)) }",
    ),
    (
        "tuple_call",
        "fn split(n: Word) -> (Word, Word) { (n / 2, n % 2) } fn main() -> Word { let t = split(9); t.0 + t.1 }",
    ),
    (
        "stream_yield",
        "loop main(seed: Word) -> Word { let a = seed + 1; if a > 3 { yield a } else { yield seed } }",
    ),
    (
        "native_call",
        "use math::bump\nfn main(x: Word) -> Word { math::bump(x) }",
    ),
    (
        "stream_delegating",
        "fn bump(n: Word) -> Word { n + 1 } loop main(seed: Word) -> Word { let a = bump(seed); yield a }",
    ),
    (
        "shared_data",
        "shared data io { a: Word, b: Word } loop main(x: Word) -> Word { io.b = io.a + x; yield io.b }",
    ),
    (
        "loop_break",
        "fn main(a: [Word; 8]) -> Word { for i in 0..8 { if a[i] > 0 { break; } } 0 }",
    ),
    ("text", "fn main() -> Text { \"hello\" }"),
    (
        "private_struct",
        "struct P { x: Word, y: Word } private data d { p: P, n: Word } \
         fn main() -> Word { d.n = d.n + 1; d.p.x + d.n }",
    ),
];

const OPERAND_VALUES: &[u16] = &[0, 1, 2, 255, 4096, u16::MAX];

fn rewrite_operand(op: &Op, v: u16) -> Option<Op> {
    let byte = (v & 0xff) as u8;
    Some(match *op {
        Op::Const(_) => Op::Const(v),
        Op::GetLocal(_) => Op::GetLocal(v),
        Op::SetLocal(_) => Op::SetLocal(v),
        Op::GetData(_) => Op::GetData(u32::from(v)),
        Op::SetData(_) => Op::SetData(u32::from(v)),
        Op::GetDataIndexed(_, n) => Op::GetDataIndexed(u32::from(v), n),
        Op::SetDataIndexed(_, n) => Op::SetDataIndexed(u32::from(v), n),
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

fn rewrite_second_operand(op: &Op, v: u16) -> Option<Op> {
    let byte = (v & 0xff) as u8;
    Some(match *op {
        Op::Call(a, _) => Op::Call(a, byte),
        Op::CallVerifiedNative(a, _) => Op::CallVerifiedNative(a, byte),
        Op::CallExternalNative(a, _) => Op::CallExternalNative(a, byte),
        Op::GetDataIndexed(a, _) => Op::GetDataIndexed(a, u32::from(v)),
        Op::SetDataIndexed(a, _) => Op::SetDataIndexed(a, u32::from(v)),
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

/// The scalar kind of a flat access rewritten to another kind, which reaches the typed pass's
/// field-kind and text checks without moving the offset.
fn rewrite_kind(op: &Op, kind: ScalarKind) -> Option<Op> {
    Some(match *op {
        Op::GetField(StructField::Flat { offset, kind: k }) if k != kind => {
            Op::GetField(StructField::Flat { offset, kind })
        }
        Op::GetTupleField(TupleField::Flat { offset, kind: k }) if k != kind => {
            Op::GetTupleField(TupleField::Flat { offset, kind })
        }
        Op::GetEnumField(EnumField::Flat { offset, kind: k }) if k != kind => {
            Op::GetEnumField(EnumField::Flat { offset, kind })
        }
        Op::GetIndex(ArrayElem::Flat { kind: k }) if k != kind => {
            Op::GetIndex(ArrayElem::Flat { kind })
        }
        _ => return None,
    })
}

const KINDS: &[ScalarKind] = &[
    ScalarKind::Int,
    ScalarKind::Bool,
    ScalarKind::Byte,
    ScalarKind::Text,
];

fn substitutes() -> Vec<(&'static str, Op)> {
    vec![
        ("return", Op::Return),
        ("yield", Op::Yield),
        ("reset", Op::Reset),
        ("stream", Op::Stream),
        ("dup", Op::Dup),
        ("add", Op::Add),
        ("not", Op::Not),
        ("len", Op::Len),
        ("popn1", Op::PopN(1)),
        ("popn_far", Op::PopN(200)),
        ("endif", Op::EndIf),
        ("endloop_far", Op::EndLoop(9)),
        ("break_far", Op::Break(9)),
        ("trap", Op::Trap(0)),
        ("const_far", Op::Const(u16::MAX)),
        ("getlocal_far", Op::GetLocal(u16::MAX)),
        ("setlocal_far", Op::SetLocal(u16::MAX)),
        ("call_far", Op::Call(u16::MAX, 3)),
        ("call0", Op::Call(0, 0)),
        ("getdata_far", Op::GetData(u32::MAX)),
        ("getdata0", Op::GetData(0)),
        ("setdata0", Op::SetData(0)),
        ("native0", Op::CallVerifiedNative(0, 1)),
        ("immediate", Op::PushImmediate(1)),
        (
            "getfield_far",
            Op::GetField(StructField::Flat {
                offset: u16::MAX,
                kind: ScalarKind::Int,
            }),
        ),
        (
            "getfield0",
            Op::GetField(StructField::Flat {
                offset: 0,
                kind: ScalarKind::Int,
            }),
        ),
        (
            "getindex_int",
            Op::GetIndex(ArrayElem::Flat {
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
        (
            "newcomposite_small",
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Tuple,
                count: 2,
                byte_size: 8,
            }),
        ),
    ]
}

const SHAPES: &[WireShape] = &[
    WireShape::Top,
    WireShape::Scalar { kind: 0 },
    WireShape::Scalar { kind: 255 },
    WireShape::Flat {
        kind: 0,
        size: u32::MAX,
    },
    WireShape::Flat { kind: 255, size: 0 },
    WireShape::Flat { kind: 0, size: 8 },
];

/// One mutation of one module, with the identity that reproduces it and the family it
/// belongs to, carried as a field rather than parsed back out of the identity.
struct Mutant {
    id: String,
    family: &'static str,
    module: Module,
}

fn push(out: &mut Vec<Mutant>, family: &'static str, id: String, module: Module) {
    out.push(Mutant { id, family, module });
}

fn instruction_mutants(prog: &str, base: &Module, out: &mut Vec<Mutant>) {
    let subs = substitutes();
    for (ci, chunk) in base.chunks.iter().enumerate() {
        for oi in 0..chunk.ops.len() {
            let here = chunk.ops[oi];
            for &v in OPERAND_VALUES {
                for (which, op) in [
                    (1, rewrite_operand(&here, v)),
                    (2, rewrite_second_operand(&here, v)),
                ] {
                    if let Some(op) = op.filter(|op| *op != here) {
                        let mut m = base.clone();
                        m.chunks[ci].ops[oi] = op;
                        let id = format!("{prog}/c{ci}/op{oi}/operand{which}={v}");
                        push(out, "operand", id, m);
                    }
                }
            }
            for &kind in KINDS {
                if let Some(op) = rewrite_kind(&here, kind) {
                    let mut m = base.clone();
                    m.chunks[ci].ops[oi] = op;
                    let id = format!("{prog}/c{ci}/op{oi}/kind={kind:?}");
                    push(out, "access kind", id, m);
                }
            }
            for (name, op) in &subs {
                if *op != here {
                    let mut m = base.clone();
                    m.chunks[ci].ops[oi] = *op;
                    let id = format!("{prog}/c{ci}/op{oi}/sub={name}");
                    push(out, "substitution", id, m);
                }
            }
            let mut m = base.clone();
            m.chunks[ci].ops.remove(oi);
            push(out, "deletion", format!("{prog}/c{ci}/op{oi}/delete"), m);
            let mut m = base.clone();
            m.chunks[ci].ops.insert(oi, here);
            let id = format!("{prog}/c{ci}/op{oi}/duplicate");
            push(out, "duplication", id, m);
            if oi + 1 < chunk.ops.len() && chunk.ops[oi + 1] != here {
                let mut m = base.clone();
                m.chunks[ci].ops.swap(oi, oi + 1);
                let id = format!("{prog}/c{ci}/op{oi}/swap");
                push(out, "transposition", id, m);
            }
        }
    }
}

fn chunk_mutants(prog: &str, base: &Module, out: &mut Vec<Mutant>) {
    for (ci, chunk) in base.chunks.iter().enumerate() {
        for v in [0u16, 1, chunk.local_count.saturating_sub(1), 300] {
            if chunk.local_count != v {
                let mut m = base.clone();
                m.chunks[ci].local_count = v;
                let id = format!("{prog}/c{ci}/local_count={v}");
                push(out, "chunk metadata", id, m);
            }
        }
        for v in [0u8, 1, u8::MAX] {
            if chunk.param_count != v {
                let mut m = base.clone();
                m.chunks[ci].param_count = v;
                let id = format!("{prog}/c{ci}/param_count={v}");
                push(out, "chunk metadata", id, m);
            }
        }
        for bt in [BlockType::Func, BlockType::Reentrant, BlockType::Stream] {
            if chunk.block_type != bt {
                let mut m = base.clone();
                m.chunks[ci].block_type = bt;
                let id = format!("{prog}/c{ci}/block_type={bt:?}");
                push(out, "chunk metadata", id, m);
            }
        }
        if !chunk.constants.is_empty() {
            let mut m = base.clone();
            m.chunks[ci].constants.pop();
            let id = format!("{prog}/c{ci}/constants_pop");
            push(out, "chunk metadata", id, m);
        }
        if !chunk.struct_templates.is_empty() {
            let mut m = base.clone();
            m.chunks[ci].struct_templates.clear();
            let id = format!("{prog}/c{ci}/templates_cleared");
            push(out, "chunk metadata", id, m);
        }
    }
}

fn table_mutants(prog: &str, base: &Module, out: &mut Vec<Mutant>) {
    for (si, sig) in base.signatures.iter().enumerate() {
        for (wi, w) in SHAPES.iter().enumerate() {
            let mut m = base.clone();
            m.signatures[si].ret = *w;
            push(out, "signature", format!("{prog}/sig{si}/ret=shape{wi}"), m);
            let mut m = base.clone();
            m.signatures[si].resume = *w;
            let id = format!("{prog}/sig{si}/resume=shape{wi}");
            push(out, "signature", id, m);
            for pi in 0..sig.params.len() {
                let mut m = base.clone();
                m.signatures[si].params[pi] = *w;
                let id = format!("{prog}/sig{si}/param{pi}=shape{wi}");
                push(out, "signature", id, m);
            }
        }
        let mut m = base.clone();
        m.signatures[si].params.push(WireShape::Scalar { kind: 0 });
        push(out, "signature", format!("{prog}/sig{si}/param_added"), m);
        if !sig.params.is_empty() {
            let mut m = base.clone();
            m.signatures[si].params.pop();
            push(out, "signature", format!("{prog}/sig{si}/param_dropped"), m);
        }
    }
    for (ei, layout) in base.enum_layouts.iter().enumerate() {
        for v in [0u32, 1, 7, u32::MAX] {
            if layout.min_payload != v {
                let mut m = base.clone();
                m.enum_layouts[ei].min_payload = v;
                let id = format!("{prog}/enum{ei}/min_payload={v}");
                push(out, "enum layout", id, m);
            }
        }
        for vi in 0..layout.variants.len() {
            for d in [-1i64, 0, 1, i64::MAX] {
                if layout.variants[vi].disc != d {
                    let mut m = base.clone();
                    m.enum_layouts[ei].variants[vi].disc = d;
                    let id = format!("{prog}/enum{ei}/variant{vi}/disc={d}");
                    push(out, "enum layout", id, m);
                }
            }
            let mut m = base.clone();
            m.enum_layouts[ei].variants.remove(vi);
            let id = format!("{prog}/enum{ei}/variant{vi}/removed");
            push(out, "enum layout", id, m);
        }
    }
    for ni in 0..base.native_return_shapes.len() {
        for (wi, w) in SHAPES.iter().enumerate() {
            let mut m = base.clone();
            m.native_return_shapes[ni] = *w;
            let id = format!("{prog}/native{ni}=shape{wi}");
            push(out, "native shape", id, m);
        }
    }
    if !base.native_names.is_empty() {
        let mut m = base.clone();
        m.native_names.clear();
        push(out, "native shape", format!("{prog}/natives_cleared"), m);
    }
}

fn layout_mutants(prog: &str, base: &Module, out: &mut Vec<Mutant>) {
    use keleusma::bytecode::SlotVisibility;
    let Some(layout) = base.data_layout.as_ref() else {
        return;
    };
    let mut m = base.clone();
    m.data_layout = None;
    push(out, "data layout", format!("{prog}/layout_removed"), m);
    for fi in 0..layout.fields.len() {
        let mut m = base.clone();
        if let Some(l) = m.data_layout.as_mut() {
            let field = &mut l.fields[fi];
            field.visibility = match field.visibility {
                SlotVisibility::Shared => SlotVisibility::Private,
                SlotVisibility::Private => SlotVisibility::Shared,
            };
        }
        push(
            out,
            "data layout",
            format!("{prog}/field{fi}/visibility"),
            m,
        );
        let mut m = base.clone();
        if let Some(l) = m.data_layout.as_mut() {
            l.fields.remove(fi);
        }
        push(out, "data layout", format!("{prog}/field{fi}/removed"), m);
    }
    // The shared fields' buffer offsets and kinds, which the retired shared layout table held.
    let shared = layout
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.visibility == SlotVisibility::Shared);
    for (fi, _) in shared {
        for v in [0u32, 1, 4096, u32::MAX] {
            let mut m = base.clone();
            if let Some(l) = m.data_layout.as_mut() {
                l.fields[fi].offset = Some(v);
            }
            let id = format!("{prog}/shared{fi}/offset={v}");
            push(out, "data layout", id, m);
        }
        for k in [0u8, 3, 7, 0x80, 0xff] {
            let mut m = base.clone();
            if let Some(l) = m.data_layout.as_mut() {
                l.fields[fi].kind = k;
            }
            push(out, "data layout", format!("{prog}/shared{fi}/kind={k}"), m);
        }
    }
    // The pooled private fields' slots and offsets, which the retired private composite
    // placement table held.
    let pooled = layout
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.visibility == SlotVisibility::Private && f.offset.is_some());
    for (fi, _) in pooled {
        for v in [0u32, 1, u32::from(u16::MAX)] {
            let mut m = base.clone();
            if let Some(l) = m.data_layout.as_mut() {
                l.fields[fi].first_slot = v;
            }
            let id = format!("{prog}/private{fi}/slot={v}");
            push(out, "data layout", id, m);
        }
        for v in [0u32, 1, u32::MAX] {
            let mut m = base.clone();
            if let Some(l) = m.data_layout.as_mut() {
                l.fields[fi].offset = Some(v);
            }
            let id = format!("{prog}/private{fi}/offset={v}");
            push(out, "data layout", id, m);
        }
    }
    for v in [0u32, 1, u32::MAX] {
        if base.shared_data_bytes != v {
            let mut m = base.clone();
            m.shared_data_bytes = v;
            let id = format!("{prog}/shared_data_bytes={v}");
            push(out, "data layout", id, m);
        }
        if base.persistent_composite_bytes != v {
            let mut m = base.clone();
            m.persistent_composite_bytes = v;
            let id = format!("{prog}/persistent_composite_bytes={v}");
            push(out, "data layout", id, m);
        }
    }
}

fn mutants() -> Vec<Mutant> {
    let mut out = Vec::new();
    for (prog, src) in CORPUS {
        let base = compile_src(src);
        assert!(
            keleusma::verify::verify(&base).is_ok(),
            "{prog}: the unmutated module must verify"
        );
        instruction_mutants(prog, &base, &mut out);
        chunk_mutants(prog, &base, &mut out);
        table_mutants(prog, &base, &mut out);
        layout_mutants(prog, &base, &mut out);
    }
    out
}

/// One validator's verdict on one mutant. A panic is recorded, never propagated, so that one
/// defect cannot hide the rest of the census.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Verdict {
    Accept,
    Reject,
    Unavailable,
    Panic,
}

fn quiet<T>(f: impl FnOnce() -> T) -> Result<T, ()> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = catch_unwind(AssertUnwindSafe(f)).map_err(|_| ());
    std::panic::set_hook(prev);
    r
}

fn from_kel<E>(r: Result<Result<VerificationVerdict, E>, ()>) -> Verdict {
    match r {
        Ok(Ok(VerificationVerdict::Accepted)) => Verdict::Accept,
        Ok(Ok(VerificationVerdict::Rejected { .. })) => Verdict::Reject,
        Ok(Err(_)) => Verdict::Unavailable,
        Err(()) => Verdict::Panic,
    }
}

fn from_reference(r: Result<bool, ()>) -> Verdict {
    match r {
        Ok(true) => Verdict::Accept,
        Ok(false) => Verdict::Reject,
        Err(()) => Verdict::Panic,
    }
}

/// The four verdicts on one module, in the order reference `verify`, self-hosted
/// `verify_module_via_kel`, reference admission, and self-hosted `admit_module_via_kel`.
/// Admission is `verify` followed by `verify_resource_bounds` at the default arena capacity.
/// It is computed only when the corresponding `verify` accepted, since a `verify` rejection
/// already decides it.
fn judge(m: &Module) -> [Verdict; 4] {
    use keleusma::vm::DEFAULT_ARENA_CAPACITY;
    let ref_verify = from_reference(quiet(|| keleusma::verify::verify(m).is_ok()));
    let ref_admit = match ref_verify {
        Verdict::Accept => from_reference(quiet(|| {
            keleusma::verify::verify_resource_bounds(m, DEFAULT_ARENA_CAPACITY).is_ok()
        })),
        other => other,
    };
    let kel_verify = from_kel(quiet(|| verify_module_via_kel(m)));
    let kel_admit = match kel_verify {
        Verdict::Accept => from_kel(quiet(|| admit_module_via_kel(m, DEFAULT_ARENA_CAPACITY))),
        other => other,
    };
    [ref_verify, kel_verify, ref_admit, kel_admit]
}

/// Mutants the self-hosted validator is known to accept although the reference rejects them,
/// each with its reason. Every entry is an open soundness residual of the review. The whole
/// corpus fails both on an unsound mutant this list does not name and on a named mutant that is
/// no longer unsound, so the list cannot go stale in either direction. Empty since the repairs
/// below.
const KNOWN_UNSOUND: &[(&str, &str)] = &[];

/// THE MUTANTS THE FIRST WHOLE RUN FOUND UNSOUND, on 2026-11-01 at `7c76d771`. The self-hosted
/// validator accepted all 41 and the reference rejects all 41, in its typed operand-stack pass.
/// Three defects in `verify_typed.kel` caused them. A loop forgot every local, a join forgot
/// every local and every operand entry where the reference keeps a shape that every path agrees
/// on, and a native's error flag was pushed as an unknown rather than as a `Word`.
/// `c5_the_repaired_mutants_are_refused` holds them as the failing-first controls of the repair.
const REPAIRED: &[&str] = &[
    "array_loop_data/c0/op13/sub=newcomposite_small",
    "array_loop_data/c0/op16/operand1=1",
    "array_loop_data/c0/op16/operand1=2",
    "array_loop_data/c0/op16/swap",
    "array_loop_data/c0/op18/swap",
    "array_loop_data/c0/op4/sub=call0",
    "array_loop_data/c0/op4/sub=dup",
    "array_loop_data/c0/op4/sub=popn1",
    "array_loop_data/c0/op4/sub=setdata0",
    "array_loop_data/c0/op5/swap",
    "enum_match/c0/op0/sub=call0",
    "enum_match/c0/op18/operand1=2",
    "enum_match/c0/op19/operand1=255",
    "enum_match/c0/op19/operand1=4096",
    "enum_match/c0/op19/operand1=65535",
    "enum_match/c0/op19/sub=getfield_far",
    "enum_match/c0/op30/operand1=2",
    "enum_match/c0/op31/operand1=255",
    "enum_match/c0/op31/operand1=4096",
    "enum_match/c0/op31/operand1=65535",
    "enum_match/c0/op31/sub=getfield_far",
    "enum_match/c0/op33/operand1=2",
    "enum_match/c0/op34/kind=Text",
    "enum_match/c0/op34/operand1=255",
    "enum_match/c0/op34/operand1=4096",
    "enum_match/c0/op34/operand1=65535",
    "enum_match/c0/op34/sub=getfield_far",
    "enum_match/c0/op6/sub=getfield_far",
    "loop_break/c0/op11/swap",
    "loop_break/c0/op19/sub=newcomposite_small",
    "loop_break/c0/op7/sub=newcomposite_small",
    "loop_break/c0/op9/operand1=1",
    "loop_break/c0/op9/operand1=2",
    "loop_break/c0/op9/swap",
    "loop_break/sig0/param0=shape1",
    "loop_break/sig0/param0=shape3",
    "native_call/c0/op3/sub=getindex_int",
    "native_call/c0/op3/sub=newcomposite_small",
    "stream_yield/c0/op16/sub=getfield0",
    "stream_yield/c0/op16/sub=getfield_far",
    "tuple_call/c1/op11/sub=getindex_int",
];

const FAMILIES: &[&str] = &[
    "operand",
    "access kind",
    "substitution",
    "deletion",
    "duplication",
    "transposition",
    "chunk metadata",
    "signature",
    "enum layout",
    "native shape",
    "data layout",
];

/// The counts one census measured, and the unsound mutants it found.
struct Census {
    total: usize,
    counts: BTreeMap<(&'static str, &'static str), usize>,
    unsound: BTreeMap<String, String>,
}

/// Judge every mutant `select` keeps, given its position within its family, print the census,
/// and fail on a panic or on an unsound mutant that `KNOWN_UNSOUND` does not name.
/// Over-rejections and unavailable analyses are counted and printed, since they narrow the
/// admitted language rather than widen it.
fn census(select: impl Fn(usize, &Mutant) -> bool) -> Census {
    let mut counts: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut unsound: BTreeMap<String, String> = BTreeMap::new();
    let mut over = Vec::new();
    let mut unavailable = Vec::new();
    let mut panics = Vec::new();
    let mut total = 0usize;
    let mut position: BTreeMap<&str, usize> = BTreeMap::new();
    for m in mutants() {
        let index = position.entry(m.family).or_default();
        let keep = select(*index, &m);
        *index += 1;
        if !keep {
            continue;
        }
        total += 1;
        let [rv, kv, ra, ka] = judge(&m.module);
        let mut count = |what| *counts.entry((m.family, what)).or_default() += 1;
        count("mutants");
        if rv == Verdict::Reject {
            count("reference verify rejects");
        }
        if ra == Verdict::Reject {
            count("reference admit rejects");
        }
        if kv == Verdict::Accept && rv != Verdict::Accept {
            count("unsound verify");
            unsound.insert(m.id.clone(), format!("verify: reference {rv:?}"));
        } else if ka == Verdict::Accept && ra != Verdict::Accept {
            count("unsound admission");
            unsound.insert(m.id.clone(), format!("admission: reference {ra:?}"));
        }
        if kv == Verdict::Reject && rv == Verdict::Accept {
            count("over-rejection verify");
            over.push(m.id.clone());
        } else if ka == Verdict::Reject && ra == Verdict::Accept {
            count("over-rejection admission");
            over.push(m.id.clone());
        }
        if kv == Verdict::Unavailable || ka == Verdict::Unavailable {
            count("unavailable");
            unavailable.push(m.id.clone());
        }
        if [rv, kv, ra, ka].contains(&Verdict::Panic) {
            count("panic");
            panics.push(format!("{} {rv:?} {kv:?} {ra:?} {ka:?}", m.id));
        }
    }
    println!("C5 mutation census, {total} mutants");
    for ((family, what), n) in &counts {
        println!("  {family:>16} {what:>26} {n}");
    }
    println!("over-rejections ({}): {over:?}", over.len());
    println!("unavailable ({}): {unavailable:?}", unavailable.len());
    println!("unsound ({}): {unsound:#?}", unsound.len());
    assert!(panics.is_empty(), "panics:\n{}", panics.join("\n"));
    let named: Vec<&str> = KNOWN_UNSOUND.iter().map(|(id, _)| *id).collect();
    let new: Vec<&String> = unsound
        .keys()
        .filter(|id| !named.contains(&id.as_str()))
        .collect();
    assert!(
        new.is_empty(),
        "the self-hosted validator accepted {} mutant(s) the reference rejects: {new:?}",
        new.len()
    );
    Census {
        total,
        counts,
        unsound,
    }
}

fn count(c: &Census, family: &str, what: &str) -> usize {
    c.counts.get(&(family, what)).copied().unwrap_or(0)
}

fn pinned(m: &Mutant) -> bool {
    REPAIRED.contains(&m.id.as_str()) || KNOWN_UNSOUND.iter().any(|(id, _)| *id == m.id)
}

/// THE C5 SOUNDNESS GATE, AS CONTINUOUS INTEGRATION RUNS IT. One mutant in every sixteen of each
/// family, by its position in the family, so that every family is reached, and every mutant
/// `REPAIRED` or `KNOWN_UNSOUND` names. The whole corpus cost 1,974 seconds in a release build on
/// 2026-11-01, which is more than one test can spend in a shard, and this is the share it can.
/// `c5_the_whole_mutation_corpus` runs the rest, and the review record gives its counts.
#[test]
fn c5_the_self_hosted_validator_accepts_no_sampled_mutant_the_reference_rejects() {
    let c = census(|index, m| index % 16 == 0 || pinned(m));
    for family in FAMILIES {
        assert!(
            count(&c, family, "mutants") >= 1,
            "{family}: the sample reached no mutant"
        );
    }
    assert!(
        c.total >= 600,
        "the sample collapsed to {} mutants",
        c.total
    );
}

/// THE C5 SOUNDNESS GATE OVER THE WHOLE CORPUS, which is the review's instrument. It is ignored
/// in the default run for its cost, and the sampled gate above runs in its place. Run it with
/// `cargo test --release -p keleusma --features self-host --test selfhost_validator -- --ignored`.
#[test]
#[ignore = "the whole corpus costs about thirty minutes in a release build"]
fn c5_the_whole_mutation_corpus() {
    let c = census(|_, _| true);
    let gone: Vec<&str> = KNOWN_UNSOUND
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| !c.unsound.contains_key(*id))
        .collect();
    assert!(
        gone.is_empty(),
        "pinned unsound mutants are no longer unsound, so remove them from KNOWN_UNSOUND and \
         record the repair: {gone:?}"
    );
    for family in FAMILIES {
        let n = count(&c, family, "mutants");
        let rejected = count(&c, family, "reference verify rejects");
        assert!(n >= 7, "{family}: reached only {n} mutants");
        assert!(rejected > 0, "{family}: the reference rejected none of {n}");
    }
    assert!(
        c.total >= 10_000,
        "the corpus collapsed to {} mutants",
        c.total
    );
}

/// The failing-first controls of the repair. Each mutant `REPAIRED` names is still generated,
/// the reference rejects it, and the self-hosted validator now rejects it. On `7c76d771` the
/// self-hosted validator accepted all 41.
#[test]
fn c5_the_repaired_mutants_are_refused() {
    let mut seen = 0usize;
    let mut accepted = Vec::new();
    for m in mutants() {
        if !REPAIRED.contains(&m.id.as_str()) {
            continue;
        }
        seen += 1;
        let [rv, kv, _, _] = judge(&m.module);
        assert_eq!(
            rv,
            Verdict::Reject,
            "{}: the reference no longer rejects it",
            m.id
        );
        if kv != Verdict::Reject {
            accepted.push(format!("{} {kv:?}", m.id));
        }
    }
    assert_eq!(
        seen,
        REPAIRED.len(),
        "a repaired mutant is no longer generated"
    );
    assert!(accepted.is_empty(), "not refused: {accepted:?}");
}
