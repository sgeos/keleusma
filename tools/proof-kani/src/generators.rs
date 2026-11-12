//! Bounded symbolic generators for verifier inputs.
//!
//! Each generator returns a value chosen nondeterministically by Kani from a
//! stated set. The sets are the trust-relevant part of every harness, because
//! a property checked over a generator holds only for the values that the
//! generator can produce.

use alloc::string::String;
use alloc::vec::Vec;
use keleusma::bytecode::{
    ArrayElem, BlockType, Chunk, EnumField, NewCompositeOperand, Op, StructField, TupleField,
};
use keleusma::value_layout::{CompositeKind, ScalarKind};

/// Measurement narrowing of the instruction selectors.
///
/// The selector of [`with_walk_op`] is confined to the closed range from
/// `KANI_WALK_SEL_LO` to `KANI_WALK_SEL_HI`, and the selector of
/// [`with_close_op`] to the range from `KANI_CLOSE_SEL_LO` to
/// `KANI_CLOSE_SEL_HI`, each read from the environment when this package is
/// compiled. Unset, the ranges are 0 to 255 and the confinement assumes
/// nothing. A narrowed run verifies only the instructions in its range, so it
/// is a measurement of one piece of a harness and never a verification of the
/// whole harness. `check.sh` reports such a run as narrowed and does not pass
/// it. A value that is not a decimal number from 0 to 255, or a range whose
/// low end exceeds its high end, fails the build.
pub const WALK_SEL_LO: u8 = parse_selector(option_env!("KANI_WALK_SEL_LO"), 0);
/// See [`WALK_SEL_LO`].
pub const WALK_SEL_HI: u8 = parse_selector(option_env!("KANI_WALK_SEL_HI"), u8::MAX);
/// See [`WALK_SEL_LO`].
pub const CLOSE_SEL_LO: u8 = parse_selector(option_env!("KANI_CLOSE_SEL_LO"), 0);
/// See [`WALK_SEL_LO`].
pub const CLOSE_SEL_HI: u8 = parse_selector(option_env!("KANI_CLOSE_SEL_HI"), u8::MAX);
const _: () = assert!(WALK_SEL_LO <= WALK_SEL_HI, "KANI_WALK_SEL_LO exceeds KANI_WALK_SEL_HI");
const _: () = assert!(CLOSE_SEL_LO <= CLOSE_SEL_HI, "KANI_CLOSE_SEL_LO exceeds KANI_CLOSE_SEL_HI");

/// Parses a selector bound, a decimal number from 0 to 255, at compile time.
/// Absent, it is `default`. Anything else stops the build.
const fn parse_selector(text: Option<&str>, default: u8) -> u8 {
    let Some(text) = text else {
        return default;
    };
    let bytes = text.as_bytes();
    assert!(
        !bytes.is_empty() && bytes.len() <= 3,
        "a selector bound must be a decimal number from 0 to 255"
    );
    let mut value: u16 = 0;
    let mut i = 0;
    while i < bytes.len() {
        assert!(
            bytes[i].is_ascii_digit(),
            "a selector bound must be a decimal number from 0 to 255"
        );
        value = value * 10 + (bytes[i] - b'0') as u16;
        i += 1;
    }
    assert!(value <= 255, "a selector bound must be a decimal number from 0 to 255");
    value as u8
}

/// Any scalar kind enabled under the default feature set.
pub fn any_scalar_kind() -> ScalarKind {
    match kani::any::<u8>() % 8 {
        0 => ScalarKind::Unit,
        1 => ScalarKind::Bool,
        2 => ScalarKind::Byte,
        3 => ScalarKind::Int,
        4 => ScalarKind::Fixed,
        5 => ScalarKind::Float,
        6 => ScalarKind::Text,
        _ => ScalarKind::Opaque,
    }
}

/// Any composite kind.
pub fn any_composite_kind() -> CompositeKind {
    match kani::any::<u8>() % 4 {
        0 => CompositeKind::Tuple,
        1 => CompositeKind::Array,
        2 => CompositeKind::Struct,
        _ => CompositeKind::Enum,
    }
}

fn any_new_composite() -> NewCompositeOperand {
    if kani::any() {
        NewCompositeOperand::Flat {
            kind: any_composite_kind(),
            count: kani::any(),
            byte_size: kani::any(),
        }
    } else {
        NewCompositeOperand::Boxed {
            kind: any_composite_kind(),
            count: kani::any(),
            meta: kani::any(),
        }
    }
}

fn any_struct_field() -> StructField {
    if kani::any() {
        StructField::Flat {
            offset: kani::any(),
            kind: any_scalar_kind(),
        }
    } else {
        StructField::FlatNested {
            offset: kani::any(),
            size: kani::any(),
            variant: any_composite_kind(),
        }
    }
}

fn any_tuple_field() -> TupleField {
    if kani::any() {
        TupleField::Flat {
            offset: kani::any(),
            kind: any_scalar_kind(),
        }
    } else {
        TupleField::FlatNested {
            offset: kani::any(),
            size: kani::any(),
            variant: any_composite_kind(),
        }
    }
}

fn any_enum_field() -> EnumField {
    if kani::any() {
        EnumField::Flat {
            offset: kani::any(),
            kind: any_scalar_kind(),
        }
    } else {
        EnumField::FlatNested {
            offset: kani::any(),
            size: kani::any(),
            variant: any_composite_kind(),
        }
    }
}

fn any_array_elem() -> ArrayElem {
    match kani::any::<u8>() % 3 {
        0 => ArrayElem::Flat {
            kind: any_scalar_kind(),
        },
        1 => ArrayElem::FlatNested {
            size: kani::any(),
            variant: any_composite_kind(),
        },
        _ => ArrayElem::Boxed,
    }
}

/// Number of `Op` constructors, which [`op_constructor_index`] enumerates.
pub const OP_CONSTRUCTORS: u8 = 66;

/// Number of selectors [`any_op`] draws from. The last three repeat
/// constructors with fixed fields, as the comment on those arms explains.
const OP_SELECTORS: u8 = 69;

/// Any instruction, over every constructor of `Op`, with every field symbolic.
///
/// [`op_constructor_index`] matches on `Op` without a wildcard arm, so adding
/// a constructor to `Op` breaks this package's build until the generator is
/// extended. That is the guard against the generator silently falling behind
/// the instruction set.
pub fn any_op() -> Op {
    let sel: u8 = kani::any();
    kani::assume(sel < OP_SELECTORS);
    match sel {
        0 => Op::Const(kani::any()),
        1 => Op::GetLocal(kani::any()),
        2 => Op::SetLocal(kani::any()),
        3 => Op::GetData(kani::any()),
        4 => Op::SetData(kani::any()),
        5 => Op::GetDataIndexed(kani::any(), kani::any()),
        6 => Op::SetDataIndexed(kani::any(), kani::any()),
        7 => Op::BoundsCheck(kani::any()),
        8 => Op::Add,
        9 => Op::Sub,
        10 => Op::Mul,
        11 => Op::Div,
        12 => Op::Mod,
        13 => Op::Neg,
        14 => Op::CmpEq,
        15 => Op::CmpNe,
        16 => Op::CmpLt,
        17 => Op::CmpGt,
        18 => Op::CmpLe,
        19 => Op::CmpGe,
        20 => Op::Not,
        21 => Op::If(kani::any()),
        22 => Op::Else(kani::any()),
        23 => Op::EndIf,
        24 => Op::Loop(kani::any()),
        25 => Op::EndLoop(kani::any()),
        26 => Op::Break(kani::any()),
        27 => Op::BreakIf(kani::any()),
        28 => Op::Stream,
        29 => Op::Reset,
        30 => Op::Call(kani::any(), kani::any()),
        31 => Op::Return,
        32 => Op::Yield,
        33 => Op::Dup,
        34 => Op::NewComposite(any_new_composite()),
        35 => Op::GetField(any_struct_field()),
        36 => Op::GetIndex(any_array_elem()),
        37 => Op::GetTupleField(any_tuple_field()),
        38 => Op::GetEnumField(any_enum_field()),
        39 => Op::Len,
        40 => Op::IsEnum(kani::any(), kani::any(), kani::any()),
        41 => Op::IsStruct(kani::any()),
        42 => Op::IntToFloat,
        43 => Op::FloatToInt,
        44 => Op::WordToByte,
        45 => Op::ByteToWord,
        46 => Op::WordToFixed(kani::any()),
        47 => Op::FixedToWord(kani::any()),
        48 => Op::FixedMul(kani::any()),
        49 => Op::FixedDiv(kani::any()),
        50 => Op::Trap(kani::any()),
        51 => Op::CheckedAdd,
        52 => Op::CheckedSub,
        53 => Op::CheckedMul(kani::any()),
        54 => Op::CheckedNeg,
        55 => Op::CheckedDiv(kani::any()),
        56 => Op::CheckedMod,
        57 => Op::PushImmediate(kani::any()),
        58 => Op::PopN(kani::any()),
        59 => Op::BitAnd,
        60 => Op::BitOr,
        61 => Op::BitXor,
        62 => Op::Shl,
        63 => Op::Shr,
        64 => Op::CallVerifiedNative(kani::any(), kani::any()),
        65 => Op::CallExternalNative(kani::any(), kani::any()),
        // Three further selectors repeat the cases whose operand-stack effect
        // depends on a field value, the two trap kinds with their own effect
        // and an indexed read with a bounds outcome. They add no new behaviour
        // and only make those cases explicit.
        66 => Op::Trap(keleusma::bytecode::TrapKind::IndexOutOfBounds.code()),
        67 => Op::Trap(keleusma::bytecode::TrapKind::HostError.code()),
        _ => Op::GetIndex(ArrayElem::Flat {
            kind: ScalarKind::Int,
        }),
    }
}

/// Exhaustive constructor index, written without a wildcard arm.
///
/// Its only purpose is the build-time exhaustiveness guard described on
/// [`any_op`]. The returned value is always below [`OP_CONSTRUCTORS`].
pub fn op_constructor_index(op: &Op) -> u8 {
    match op {
        Op::Const(_) => 0,
        Op::GetLocal(_) => 1,
        Op::SetLocal(_) => 2,
        Op::GetData(_) => 3,
        Op::SetData(_) => 4,
        Op::GetDataIndexed(_, _) => 5,
        Op::SetDataIndexed(_, _) => 6,
        Op::BoundsCheck(_) => 7,
        Op::Add => 8,
        Op::Sub => 9,
        Op::Mul => 10,
        Op::Div => 11,
        Op::Mod => 12,
        Op::Neg => 13,
        Op::CmpEq => 14,
        Op::CmpNe => 15,
        Op::CmpLt => 16,
        Op::CmpGt => 17,
        Op::CmpLe => 18,
        Op::CmpGe => 19,
        Op::Not => 20,
        Op::If(_) => 21,
        Op::Else(_) => 22,
        Op::EndIf => 23,
        Op::Loop(_) => 24,
        Op::EndLoop(_) => 25,
        Op::Break(_) => 26,
        Op::BreakIf(_) => 27,
        Op::Stream => 28,
        Op::Reset => 29,
        Op::Call(_, _) => 30,
        Op::Return => 31,
        Op::Yield => 32,
        Op::Dup => 33,
        Op::NewComposite(_) => 34,
        Op::GetField(_) => 35,
        Op::GetIndex(_) => 36,
        Op::GetTupleField(_) => 37,
        Op::GetEnumField(_) => 38,
        Op::Len => 39,
        Op::IsEnum(_, _, _) => 40,
        Op::IsStruct(_) => 41,
        Op::IntToFloat => 42,
        Op::FloatToInt => 43,
        Op::WordToByte => 44,
        Op::ByteToWord => 45,
        Op::WordToFixed(_) => 46,
        Op::FixedToWord(_) => 47,
        Op::FixedMul(_) => 48,
        Op::FixedDiv(_) => 49,
        Op::Trap(_) => 50,
        Op::CheckedAdd => 51,
        Op::CheckedSub => 52,
        Op::CheckedMul(_) => 53,
        Op::CheckedNeg => 54,
        Op::CheckedDiv(_) => 55,
        Op::CheckedMod => 56,
        Op::PushImmediate(_) => 57,
        Op::PopN(_) => 58,
        Op::BitAnd => 59,
        Op::BitOr => 60,
        Op::BitXor => 61,
        Op::Shl => 62,
        Op::Shr => 63,
        Op::CallVerifiedNative(_, _) => 64,
        Op::CallExternalNative(_, _) => 65,
    }
}

/// Largest operand count, argument count or composite field count that the
/// chunk generators produce.
///
/// The analyses loop once per counted operand, so an unbounded count would
/// need an unwinding bound of 256 or more. Kani keeps its unwinding
/// assertions on, so a count above this limit could not pass silently. It
/// would fail the harness instead.
pub const MAX_COUNT: u8 = 3;

/// A count from zero to [`MAX_COUNT`].
pub fn small_count() -> u8 {
    let n: u8 = kani::any();
    kani::assume(n <= MAX_COUNT);
    n
}

/// A native call argument byte, a small count with the error-reify high bit
/// either set or clear.
fn native_args() -> u8 {
    if kani::any() {
        small_count() | 0x80
    } else {
        small_count()
    }
}

fn small_new_composite() -> NewCompositeOperand {
    let count = u16::from(small_count());
    if kani::any() {
        NewCompositeOperand::Flat {
            kind: any_composite_kind(),
            count,
            byte_size: kani::any(),
        }
    } else {
        NewCompositeOperand::Boxed {
            kind: any_composite_kind(),
            count,
            meta: kani::any(),
        }
    }
}

// Why the chunk generators take a continuation.
//
// A chunk whose instruction DISCRIMINANTS are symbolic defeats the constant
// propagation of the model checker's symbolic execution. Every match on an
// instruction then explores every arm, and the recursive region walks compound
// that into a blow-up that did not finish within 300 seconds even for a
// three-instruction chunk with one two-way choice. Measured on 2026-11-08.
//
// The generators below instead choose the discriminant and every jump target
// by branching, and call the continuation `f` separately in each branch with a
// concrete instruction. Symbolic execution then sees a concrete chunk shape on
// each path, while operands that the walks do not follow, such as slots,
// constant indices, trap codes and callee indices, stay symbolic. The cost is
// that the number of paths is the product of the choices, which is what
// bounds the chunk lengths in the harnesses. Each concrete shape costs about
// five seconds of checking on the development machine, the cost grows faster
// than linearly when many shapes share one harness, and a harness can hold
// about eight gigabytes of memory, so the enumerations are kept small and
// split across several harnesses that run one at a time.
//
// For the same reason no harness may choose a whole instruction, chunk or
// block type with an `if kani::any()` expression and then pass the merged
// value on. Each branch must call its continuation itself.
//
// A branch in which every `kani::any()` guard below is false calls `f` not at
// all. Such a branch checks nothing and is harmless to soundness. The cover
// statements in the harnesses show that the interesting branches are reached.

/// Calls `f` with one jump target from `0` to `n` inclusive, the target being
/// concrete on each path.
pub fn with_target<F: FnMut(u16)>(n: u16, f: &mut F) {
    let mut v: u16 = 0;
    while v <= n {
        if kani::any() {
            f(v);
            return;
        }
        v += 1;
    }
}

/// Calls `f` with one instruction from the set the region walks treat
/// specially, plus one representative of each ordinary operand-stack shape.
///
/// The set is every structured control-flow instruction, the stream envelope,
/// `Yield`, `Return`, `Trap`, the three call forms and the local, constant,
/// arithmetic, pop and composite construction instructions. `If`, `Else` and
/// `Loop` take every target from `0` to `n`, which for `n` equal to the chunk
/// length is every in-range target and every backward or self target. Slots,
/// constant indices, callee indices, trap codes, and `Break`, `BreakIf` and
/// `EndLoop` operands are unconstrained. Every count is at most
/// [`MAX_COUNT`].
pub fn with_walk_op<F: FnMut(Op)>(n: u16, f: &mut F) {
    let sel: u8 = kani::any();
    kani::assume(WALK_SEL_LO <= sel && sel <= WALK_SEL_HI);
    match sel {
        0 => f(Op::Stream),
        1 => f(Op::Reset),
        2 => f(Op::Yield),
        3 => f(Op::EndIf),
        4 => f(Op::Return),
        5 => f(Op::Trap(kani::any())),
        6 => f(Op::Break(kani::any())),
        7 => f(Op::BreakIf(kani::any())),
        8 => f(Op::EndLoop(kani::any())),
        9 => f(Op::Add),
        10 => f(Op::Const(kani::any())),
        11 => f(Op::GetLocal(kani::any())),
        12 => f(Op::SetLocal(kani::any())),
        13 => f(Op::PopN(small_count())),
        14 => f(Op::Call(kani::any(), small_count())),
        15 => f(Op::CallVerifiedNative(kani::any(), native_args())),
        16 => f(Op::CallExternalNative(kani::any(), native_args())),
        17 => f(Op::NewComposite(small_new_composite())),
        18 => with_target(n, &mut |t| f(Op::If(t))),
        19 => with_target(n, &mut |t| f(Op::Else(t))),
        _ => with_target(n, &mut |t| f(Op::Loop(t))),
    }
}

/// Calls `f` with one instruction that can close or end a region opened
/// earlier in the chunk, `Yield`, `EndIf`, `Return`, `Trap`, `EndLoop`,
/// `Break` or `BreakIf` with an unconstrained operand, or `Else` with every
/// target from `lo` to `n`. Only the `Else` target is followed by the region
/// walks, which is why only it is enumerated.
pub fn with_close_op<F: FnMut(Op)>(lo: u16, n: u16, f: &mut F) {
    let sel: u8 = kani::any();
    kani::assume(CLOSE_SEL_LO <= sel && sel <= CLOSE_SEL_HI);
    match sel {
        0 => f(Op::Yield),
        1 => f(Op::EndIf),
        2 => f(Op::Return),
        3 => f(Op::Trap(kani::any())),
        4 => f(Op::EndLoop(kani::any())),
        5 => f(Op::Break(kani::any())),
        6 => f(Op::BreakIf(kani::any())),
        _ => with_target(n - lo, &mut |k| f(Op::Else(k + lo))),
    }
}

/// Calls `f` with one instruction that the region walks cost as straight-line
/// code and that the text-length analysis does not charge. Used where a
/// harness states an exact cost rather than an upper bound.
pub fn with_plain_op<F: FnMut(Op)>(f: &mut F) {
    match kani::any::<u8>() {
        0 => f(Op::PushImmediate(kani::any())),
        1 => f(Op::Dup),
        2 => f(Op::Yield),
        3 => f(Op::Mul),
        4 => f(Op::Sub),
        5 => f(Op::Not),
        6 => f(Op::Neg),
        7 => f(Op::PopN(small_count())),
        _ => f(Op::BitXor),
    }
}

/// A chunk with the given instructions and block type and no constants,
/// templates or debug metadata.
pub fn chunk_of(ops: Vec<Op>, block_type: BlockType, local_count: u16) -> Chunk {
    Chunk {
        name: String::from("k"),
        ops,
        constants: Vec::new(),
        struct_templates: Vec::new(),
        local_count,
        param_count: 0,
        block_type,
        param_types: Vec::new(),
        debug_pool: None,
    }
}

/// A module holding the given chunks and otherwise empty, at the runtime's
/// default word, address and float widths.
pub fn module_of(chunks: Vec<Chunk>) -> keleusma::bytecode::Module {
    keleusma::bytecode::Module {
        schema_hash: 0,
        enum_layouts: Vec::new(),
        signatures: Vec::new(),
        native_return_shapes: Vec::new(),
        chunks,
        native_names: Vec::new(),
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: keleusma::bytecode::RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: keleusma::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: keleusma::bytecode::RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        aux_arena_bytes: 0,
        persistent_composite_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
    }
}
