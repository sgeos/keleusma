//! **THE OPERAND-TYPE AXIS, WITH A DENOMINATOR — AND THE CELL WHERE THE
//! REFERENCE TRAPS AND THIS BACKEND USED TO ANSWER.**
//!
//! # Two variant axes, and this is the second
//!
//! `opcode_denominator.rs` closed the first: an opcode's own operand FIELD, where
//! `NewComposite` carries eight variants under one name. **This is the other
//! axis: the runtime TYPE of the operands, which has no field in the bytecode at
//! all.** `Op::Add`, `Op::Mod` and `Op::Shl` carry nothing; the virtual machine
//! dispatches on the values it pops. That axis produced the three defects this
//! line has already fixed.
//!
//! `operand_variant_sweep.rs` covers it with 27 hand-written cases and no
//! denominator — the shape the opcode census had before it was given one. This
//! file enumerates the cells from the type list and the operator list instead.
//!
//! # What the denominator found: `Fixed % Fixed`
//!
//! - The reference **compiler accepts it** and emits a plain `Op::Mod`.
//! - The reference **virtual machine traps**:
//!   `TypeError("cannot modulo Fixed by Fixed")`. Its `Op::Mod` arm handles
//!   `Int`/`Int`, `Byte`/`Byte` and `Float`/`Float`; `Fixed` falls to the
//!   catch-all.
//! - This backend **lowered it and returned a value** — `4.0` for `200.0 % 7.0`,
//!   arithmetically right and **not what the reference does**.
//!
//! Now refused in the emitter. `Op::Div` is refused on a `Fixed` operand for the
//! same reason, its virtual-machine arm being equally Fixed-less.
//!
//! # ⚠ WHY THE OLD SWEEP COULD NOT HAVE FOUND THIS
//!
//! `run_both` panics on any virtual-machine outcome that is not `Finished`. **A
//! trapping cell cannot be represented in that matrix at all** — adding one would
//! panic the harness rather than record a result.
//!
//! The previous defect hid behind an instrument's KEYING. This one hid behind an
//! instrument's **outcome type**: the harness could say *agree* and *disagree*,
//! but had no way to say *the reference refuses*. **An instrument can only find
//! defects it has a vocabulary for**, so [`Cell`] below carries the outcomes
//! this backend can actually produce, including the two that are defects.
//!
//! # The shift rows say which type was refused
//!
//! A `Byte` value shifts by a `Word` amount or a literal; a `Byte` AMOUNT is
//! refused. Recording that as "byte shifts are refused" would be the same
//! conflation that once filed an unknown type name as a fact about comparisons,
//! so each shift is driven in all three amount forms and they are recorded
//! separately. `Fixed` is refused by the shift operators in every form.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

/// What a cell's two implementations did.
///
/// **The last three exist because the old sweep could not express them.**
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum Cell {
    /// Both produced the same value.
    Agree,
    /// The reference compiler refused the program. Nothing is claimed about the
    /// backend, which never saw a module.
    RefRejects,
    /// The reference traps at run time and the backend refuses to lower. Sound:
    /// neither produces a value.
    VmTrapsNativeRefuses,
    /// The reference traps at run time and the backend **returns a value**. A
    /// DEFECT: native code answers where the reference errors.
    VmTrapsNativeComputes,
    /// Both produced a value and the values differ. A DEFECT.
    Disagree,
    /// The backend refused a program the reference runs to completion. Not a
    /// correctness defect, but a coverage loss worth seeing.
    NativeRefusesOnly,
}

fn raw(v: &Value) -> i64 {
    match v {
        Value::Byte(x) => i64::from(*x),
        Value::Int(x) | Value::Fixed(x) => *x,
        Value::Bool(x) => i64::from(*x),
        other => panic!("this matrix drives scalars only, got {other:?}"),
    }
}

fn drive(src: &str, args: &[Value]) -> Cell {
    let Some(m) = common::try_build(src) else {
        return Cell::RefRejects;
    };
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    // **A non-`Finished` outcome is recorded, not panicked on.** This is the
    // whole reason the file exists.
    let vm_out: Option<i64> = match vm.call(args) {
        Ok(VmState::Finished(v)) => Some(raw(&v)),
        _ => None,
    };
    if !module_refusals(&m, LowerOptions::default()).is_empty() {
        return if vm_out.is_none() {
            Cell::VmTrapsNativeRefuses
        } else {
            Cell::NativeRefusesOnly
        };
    }
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let sym = format!("kel_chunk_{entry}");
    let nat = match args.len() {
        1 => {
            let f =
                unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }.expect("s");
            unsafe { f.call(raw(&args[0])) }
        }
        2 => {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64) -> i64>(&sym) }
                .expect("s");
            unsafe { f.call(raw(&args[0]), raw(&args[1])) }
        }
        n => panic!("this matrix drives one or two arguments, not {n}"),
    };
    match vm_out {
        None => Cell::VmTrapsNativeComputes,
        Some(v) if v == nat => Cell::Agree,
        Some(_) => Cell::Disagree,
    }
}

/// Binary operators that take a same-typed pair.
const PAIR_OPS: &[&str] = &["+", "-", "*", "/", "%", "band", "bor", "bxor"];
/// Shift operators, driven in three amount forms.
const SHIFT_OPS: &[&str] = &["lsl", "asl", "lsr", "asr"];
/// Comparisons, which return `bool` rather than the operand type.
const CMP_OPS: &[&str] = &["<", ">", "<=", ">=", "==", "!="];

/// Every cell, enumerated from the types and the operators.
fn observe() -> Vec<(String, Cell)> {
    let b = Value::Byte;
    let w = Value::Int;
    let fx = Value::Fixed;
    // ⚠ **`Float` IS MISSING FROM THIS LIST, AND THAT COST A REAL DEFECT.**
    //
    // This function is documented as enumerating *"every cell, from the types and
    // the operators"*. It enumerates three of the four scalar types. `raw` above
    // panics on a `Value::Float` with a message calling it a non-scalar, and this
    // file's header discusses float dispatch — so the omission reads as deliberate
    // and was stated nowhere.
    //
    // **On 2026-09-17 `Op::Neg` on a `Float` was found to emit INVALID IR under
    // `narrow-float-32`** — a bitcast changing the bit width — so float negation
    // was broken outright in a supported configuration, and no cell here could
    // have seen it. It was found by a composite-field probe instead.
    //
    // **Adding `Float` here is not a small change**: the driver passes arguments
    // as raw `i64` bit patterns, and this package has already recorded a probe
    // defect from passing a float as `i64::MIN` rather than as its bits. Until
    // that is done properly, `float_ir_validity.rs` covers the float surface for
    // IR validity in both configurations, which is what the missing cells would
    // have caught. **The gap is narrower now and it is written down.**
    let types: &[(&str, &str, Value, Value)] = &[
        ("byte", "Byte", b(200), b(7)),
        ("word", "Word", w(200), w(7)),
        ("fixed", "Fixed", fx(200 << 16), fx(7 << 16)),
    ];
    let mut out = Vec::new();
    for (tn, ty, a, rhs) in types {
        for op in PAIR_OPS {
            out.push((
                format!("{tn} {op}"),
                drive(
                    &format!("fn main(a: {ty}, b: {ty}) -> {ty} {{ a {op} b }}"),
                    &[a.clone(), rhs.clone()],
                ),
            ));
        }
        for op in SHIFT_OPS {
            // Three amount forms, recorded apart, so a refusal names the type it
            // is really about.
            out.push((
                format!("{tn} {op} [Word amt]"),
                drive(
                    &format!("fn main(a: {ty}, b: Word) -> {ty} {{ a {op} b }}"),
                    &[a.clone(), w(3)],
                ),
            ));
            let same = match *tn {
                "byte" => b(3),
                "word" => w(3),
                _ => fx(3 << 16),
            };
            out.push((
                format!("{tn} {op} [same-type amt]"),
                drive(
                    &format!("fn main(a: {ty}, b: {ty}) -> {ty} {{ a {op} b }}"),
                    &[a.clone(), same],
                ),
            ));
            out.push((
                format!("{tn} {op} [literal amt]"),
                drive(
                    &format!("fn main(a: {ty}) -> {ty} {{ a {op} 3 }}"),
                    std::slice::from_ref(a),
                ),
            ));
        }
        for op in ["-", "bnot"] {
            let src = if op == "-" {
                format!("fn main(a: {ty}) -> {ty} {{ -a }}")
            } else {
                format!("fn main(a: {ty}) -> {ty} {{ bnot a }}")
            };
            out.push((
                format!("{tn} unary{op}"),
                drive(&src, std::slice::from_ref(a)),
            ));
        }
        for op in CMP_OPS {
            out.push((
                format!("{tn} {op}"),
                drive(
                    &format!("fn main(a: {ty}, b: {ty}) -> bool {{ a {op} b }}"),
                    &[a.clone(), rhs.clone()],
                ),
            ));
        }
    }
    for op in ["and", "or", "xor", "andalso", "orelse"] {
        out.push((
            format!("bool {op}"),
            drive(
                &format!("fn main(a: bool, b: bool) -> bool {{ a {op} b }}"),
                &[Value::Bool(true), Value::Bool(false)],
            ),
        ));
    }
    out.push((
        "bool unarynot".to_string(),
        drive("fn main(a: bool) -> bool { not a }", &[Value::Bool(true)]),
    ));
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Fewer cells than this means the enumeration broke, not that the surface
/// shrank.
const CELL_FLOOR: usize = 85;

/// Every cell and the class it was observed in.
///
/// **`fixed %` reads `VmTrapsNativeRefuses` because the emitter was fixed.** It
/// was `VmTrapsNativeComputes` when this file was written, which is the defect
/// the denominator found.
const RECORDED: &[(&str, Cell)] = &[
    ("bool and", Cell::Agree),
    ("bool andalso", Cell::Agree),
    ("bool or", Cell::Agree),
    ("bool orelse", Cell::Agree),
    ("bool unarynot", Cell::Agree),
    ("bool xor", Cell::Agree),
    ("byte -", Cell::Agree),
    ("byte !=", Cell::Agree),
    ("byte *", Cell::Agree),
    ("byte /", Cell::Agree),
    ("byte %", Cell::Agree),
    ("byte +", Cell::Agree),
    ("byte <", Cell::Agree),
    ("byte <=", Cell::Agree),
    ("byte ==", Cell::Agree),
    ("byte >", Cell::Agree),
    ("byte >=", Cell::Agree),
    ("byte asl [literal amt]", Cell::Agree),
    ("byte asl [same-type amt]", Cell::RefRejects),
    ("byte asl [Word amt]", Cell::Agree),
    ("byte asr [literal amt]", Cell::Agree),
    ("byte asr [same-type amt]", Cell::RefRejects),
    ("byte asr [Word amt]", Cell::Agree),
    ("byte band", Cell::Agree),
    ("byte bor", Cell::Agree),
    ("byte bxor", Cell::Agree),
    ("byte lsl [literal amt]", Cell::Agree),
    ("byte lsl [same-type amt]", Cell::RefRejects),
    ("byte lsl [Word amt]", Cell::Agree),
    ("byte lsr [literal amt]", Cell::Agree),
    ("byte lsr [same-type amt]", Cell::RefRejects),
    ("byte lsr [Word amt]", Cell::Agree),
    ("byte unary-", Cell::Agree),
    ("byte unarybnot", Cell::Agree),
    ("fixed -", Cell::Agree),
    ("fixed !=", Cell::Agree),
    ("fixed *", Cell::Agree),
    ("fixed /", Cell::Agree),
    ("fixed %", Cell::VmTrapsNativeRefuses),
    ("fixed +", Cell::Agree),
    ("fixed <", Cell::Agree),
    ("fixed <=", Cell::Agree),
    ("fixed ==", Cell::Agree),
    ("fixed >", Cell::Agree),
    ("fixed >=", Cell::Agree),
    ("fixed asl [literal amt]", Cell::RefRejects),
    ("fixed asl [same-type amt]", Cell::RefRejects),
    ("fixed asl [Word amt]", Cell::RefRejects),
    ("fixed asr [literal amt]", Cell::RefRejects),
    ("fixed asr [same-type amt]", Cell::RefRejects),
    ("fixed asr [Word amt]", Cell::RefRejects),
    ("fixed band", Cell::RefRejects),
    ("fixed bor", Cell::RefRejects),
    ("fixed bxor", Cell::RefRejects),
    ("fixed lsl [literal amt]", Cell::RefRejects),
    ("fixed lsl [same-type amt]", Cell::RefRejects),
    ("fixed lsl [Word amt]", Cell::RefRejects),
    ("fixed lsr [literal amt]", Cell::RefRejects),
    ("fixed lsr [same-type amt]", Cell::RefRejects),
    ("fixed lsr [Word amt]", Cell::RefRejects),
    ("fixed unary-", Cell::Agree),
    ("fixed unarybnot", Cell::RefRejects),
    ("word -", Cell::Agree),
    ("word !=", Cell::Agree),
    ("word *", Cell::Agree),
    ("word /", Cell::Agree),
    ("word %", Cell::Agree),
    ("word +", Cell::Agree),
    ("word <", Cell::Agree),
    ("word <=", Cell::Agree),
    ("word ==", Cell::Agree),
    ("word >", Cell::Agree),
    ("word >=", Cell::Agree),
    ("word asl [literal amt]", Cell::Agree),
    ("word asl [same-type amt]", Cell::Agree),
    ("word asl [Word amt]", Cell::Agree),
    ("word asr [literal amt]", Cell::Agree),
    ("word asr [same-type amt]", Cell::Agree),
    ("word asr [Word amt]", Cell::Agree),
    ("word band", Cell::Agree),
    ("word bor", Cell::Agree),
    ("word bxor", Cell::Agree),
    ("word lsl [literal amt]", Cell::Agree),
    ("word lsl [same-type amt]", Cell::Agree),
    ("word lsl [Word amt]", Cell::Agree),
    ("word lsr [literal amt]", Cell::Agree),
    ("word lsr [same-type amt]", Cell::Agree),
    ("word lsr [Word amt]", Cell::Agree),
    ("word unary-", Cell::Agree),
    ("word unarybnot", Cell::Agree),
];

#[test]
fn every_cell_is_classified_and_the_class_still_holds() {
    let observed = observe();
    assert!(
        observed.len() >= CELL_FLOOR,
        "the enumeration produced {} cells, below the floor of {CELL_FLOOR}. A \
         short enumeration passes every check below while testing nothing, so \
         this is a broken probe rather than a result.",
        observed.len()
    );
    let recorded: std::collections::BTreeMap<&str, Cell> = RECORDED.iter().copied().collect();
    assert_eq!(
        recorded.len(),
        RECORDED.len(),
        "the record contains a duplicate cell label"
    );
    assert_eq!(
        observed.len(),
        RECORDED.len(),
        "the enumeration produced {} cells and {} are recorded. Classify the new \
         cells by what they do; do not resize the record to match.",
        observed.len(),
        RECORDED.len()
    );

    let mut moved = Vec::new();
    for (label, got) in &observed {
        match recorded.get(label.as_str()) {
            None => moved.push(format!("{label}: unclassified, observed {got:?}")),
            Some(&want) if want != *got => {
                moved.push(format!("{label}: recorded {want:?}, observed {got:?}"))
            }
            Some(_) => {}
        }
    }
    assert!(
        moved.is_empty(),
        "{} cell(s) changed class: {moved:?}. Say in the record what changed and \
         why; do not edit the class to match the run.",
        moved.len()
    );
}

/// **The two classes that are defects, asserted apart from the record**, so that
/// re-recording a cell cannot quietly make a defect acceptable.
#[test]
fn no_cell_answers_where_the_reference_errors() {
    let bad: Vec<String> = observe()
        .into_iter()
        .filter(|(_, c)| matches!(c, Cell::VmTrapsNativeComputes | Cell::Disagree))
        .map(|(l, c)| format!("{l}: {c:?}"))
        .collect();
    assert!(
        bad.is_empty(),
        "{} cell(s) diverge from the reference: {bad:?}. `VmTrapsNativeComputes` \
         means native code returns a value where the reference raises an error, \
         which is the `Fixed % Fixed` defect this file was built to find. An \
         arithmetically correct answer is still wrong when the reference does not \
         produce it.",
        bad.len()
    );
}

/// The `Fixed` refusal, asserted on its own rather than only through the matrix.
///
/// The matrix records a CLASS; this records the CAUSE, so a refusal arriving for
/// an unrelated reason is a failure rather than a pass.
#[test]
fn fixed_modulo_is_refused_with_a_reason_naming_the_operand() {
    let m = common::build("fn main(a: Fixed, b: Fixed) -> Fixed { a % b }");
    let text = format!("{:?}", module_refusals(&m, LowerOptions::default()));
    assert!(
        text.contains("Fixed operand"),
        "`Fixed % Fixed` is no longer refused for having a Fixed operand: {text}"
    );
}

/// **The `Fixed` scalar tag is a number agreed with a file this line does not
/// own.** A silent renumbering upstream would make every `Fixed` parameter read
/// as some other kind, the refusal above would stop firing, and the divergence
/// would reopen — failing OPEN, which is the direction that matters.
#[test]
fn the_fixed_scalar_tag_still_matches_upstream() {
    assert_eq!(
        keleusma::value_layout::ScalarKind::Fixed.to_tag(),
        4,
        "`ScalarKind::Fixed` no longer encodes as 4. `SCALAR_FIXED_TAG` in the \
         emitter must follow, or every Fixed operand reads as an unrecognised \
         kind and `Fixed % Fixed` is lowered again."
    );
}
