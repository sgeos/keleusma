//! **WHICH OPCODES DOES THIS LINE ACTUALLY DRIVE, END TO END?**
//!
//! # The axis this adds
//!
//! `opcode_denominator.rs` answers *does the backend lower it* and says in its
//! own header that a `Lowered` verdict **does not mean the emitted code is
//! correct**. `differential_coverage.rs` answers *does the corpus execute a
//! module containing it*, and explicitly disclaims the stronger reading: **an
//! executed module does not exercise every opcode it contains.**
//!
//! This file answers the strongest of the three: **is there a program that emits
//! this opcode, runs on both implementations, and agrees?**
//!
//! # ⚠ THE DRIVER RETURNS `Word` ONLY, AND THAT SHAPES THE TABLE
//!
//! `common::vm_and_native_two_arg` panics unless the virtual machine finishes
//! with a `Value::Int`. That is a real constraint, not a defect: a `Byte`,
//! `Fixed` or `bool` return needs a different entry signature, and a float return
//! needs an `f64` one. Witnesses here are therefore written to return `Word`,
//! wrapping the conversion under test where necessary — `(a as Byte) as Word`
//! witnesses `WordToByte` without asking the driver for a `Byte` return.
//!
//! **Opcodes whose witness cannot be shaped that way are recorded, not omitted.**
//!
//! # Why the driver is delegated to rather than reimplemented
//!
//! A first attempt named the lowered function's signature by hand and took a
//! **SIGBUS** on the first composite witness. The canonical driver reads
//! `count_params`, sizes every buffer from the published contract, installs the
//! private-init image and plants canaries. `common/mod.rs` warns about exactly
//! this: *"a harness that names a signature cannot see it change … ten tests kept
//! passing on the calling convention's good manners."*
//!
//! # ⚠ THREE APPARENT DIVERGENCES WHILE BUILDING THIS WERE THE HARNESS
//!
//! A float argument passed as `i64::MIN` rather than its bit pattern; a float
//! return read as an integer; and the hand-named signature above. **None was a
//! backend defect.** A probe that produces a divergence has implicated itself
//! first, and this one implicated itself three times in one sitting.

use std::collections::BTreeSet;

mod common;

/// `(opcode, witness returning `Word`)`.
const WITNESSED: &[(&str, &str)] = &[
    ("Const", "fn main(a: Word, b: Word) -> Word { 7 }"),
    ("GetLocal", "fn main(a: Word, b: Word) -> Word { a }"),
    (
        "SetLocal",
        "fn main(a: Word, b: Word) -> Word { let c: Word = a; c }",
    ),
    (
        "If",
        "fn main(a: Word, b: Word) -> Word { if a > b { 1 } else { 2 } }",
    ),
    (
        "Call",
        "fn f(x: Word) -> Word { x + 1 }\nfn main(a: Word, b: Word) -> Word { f(a) }",
    ),
    ("PopN", "fn main(a: Word, b: Word) -> Word { a + b }"),
    (
        "Not",
        "fn main(a: Word, b: Word) -> Word { if not (a > b) { 1 } else { 0 } }",
    ),
    (
        "IntToFloat",
        "fn main(a: Word, b: Word) -> Word { (a as Float) as Word }",
    ),
    (
        "FloatToInt",
        "fn main(a: Word, b: Word) -> Word { (a as Float) as Word }",
    ),
    (
        "ByteToWord",
        "fn main(a: Word, b: Word) -> Word { (a as Byte) as Word }",
    ),
    (
        "WordToByte",
        "fn main(a: Word, b: Word) -> Word { (a as Byte) as Word }",
    ),
    (
        "FixedToWord",
        "fn main(a: Word, b: Word) -> Word { (a as Fixed) as Word }",
    ),
    (
        "WordToFixed",
        "fn main(a: Word, b: Word) -> Word { (a as Fixed) as Word }",
    ),
    (
        "GetField",
        "struct P { x: Word }\nfn main(a: Word, b: Word) -> Word { let p: P = P { x: a }; p.x }",
    ),
    (
        "GetIndex",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 2] = [a, b]; xs[0] }",
    ),
    (
        "GetTupleField",
        "fn main(a: Word, b: Word) -> Word { let t: (Word, Word) = (a, b); t.0 }",
    ),
    (
        "NewComposite",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 2] = [a, b]; xs[1] }",
    ),
    ("CheckedAdd", "fn main(a: Word, b: Word) -> Word { a + b }"),
    ("CheckedNeg", "fn main(a: Word, b: Word) -> Word { -a }"),
    ("CheckedSub", "fn main(a: Word, b: Word) -> Word { a - b }"),
    ("CheckedMul", "fn main(a: Word, b: Word) -> Word { a * b }"),
    ("Div", "fn main(a: Word, b: Word) -> Word { a / b }"),
    ("Mod", "fn main(a: Word, b: Word) -> Word { a % b }"),
    ("BitAnd", "fn main(a: Word, b: Word) -> Word { a band b }"),
    ("BitOr", "fn main(a: Word, b: Word) -> Word { a bor b }"),
    ("BitXor", "fn main(a: Word, b: Word) -> Word { a bxor b }"),
    ("Shl", "fn main(a: Word, b: Word) -> Word { a lsl b }"),
    ("Shr", "fn main(a: Word, b: Word) -> Word { a asr b }"),
    (
        "IsEnum",
        "enum E { A(Word), B }\nfn main(a: Word, b: Word) -> Word { let e: E = E::A(a); match e { E::A(v) => v, E::B => 0 } }",
    ),
    (
        "GetEnumField",
        "enum E { A(Word), B }\nfn main(a: Word, b: Word) -> Word { let e: E = E::A(a); match e { E::A(v) => v, E::B => 0 } }",
    ),
    ("Return", "fn main(a: Word, b: Word) -> Word { a }"),
    (
        "Else",
        "fn main(a: Word, b: Word) -> Word { if a > b { 1 } else { 2 } }",
    ),
    (
        "EndIf",
        "fn main(a: Word, b: Word) -> Word { if a > b { 1 } else { 2 } }",
    ),
    (
        "Trap",
        "enum E { A(Word), B }\nfn main(a: Word, b: Word) -> Word { let e: E = E::A(a); match e { E::A(v) => v, E::B => 0 } }",
    ),
    (
        "Add",
        "fn main(a: Word, b: Word) -> Word { ((a as Byte) + (b as Byte)) as Word }",
    ),
    (
        "Sub",
        "fn main(a: Word, b: Word) -> Word { ((a as Byte) - (b as Byte)) as Word }",
    ),
    (
        "Mul",
        "fn main(a: Word, b: Word) -> Word { ((a as Byte) * (b as Byte)) as Word }",
    ),
    (
        "Neg",
        "fn main(a: Word, b: Word) -> Word { (-(a as Fixed)) as Word }",
    ),
    (
        "FixedMul",
        "fn main(a: Word, b: Word) -> Word { ((a as Fixed) * (b as Fixed)) as Word }",
    ),
    (
        "FixedDiv",
        "fn main(a: Word, b: Word) -> Word { ((a as Fixed) / (b as Fixed)) as Word }",
    ),
    (
        "CmpEq",
        "fn main(a: Word, b: Word) -> Word { if a == b { 1 } else { 0 } }",
    ),
    (
        "CmpNe",
        "fn main(a: Word, b: Word) -> Word { if a != b { 1 } else { 0 } }",
    ),
    (
        "CmpLt",
        "fn main(a: Word, b: Word) -> Word { if a < b { 1 } else { 0 } }",
    ),
    (
        "CmpGt",
        "fn main(a: Word, b: Word) -> Word { if a > b { 1 } else { 0 } }",
    ),
    (
        "CmpLe",
        "fn main(a: Word, b: Word) -> Word { if a <= b { 1 } else { 0 } }",
    ),
    (
        "CmpGe",
        "fn main(a: Word, b: Word) -> Word { if a >= b { 1 } else { 0 } }",
    ),
    (
        "Loop",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 4] = [a, b, 1, 2]; for i in 0..4 { let _q = i * a; break; } xs[3] }",
    ),
    (
        "EndLoop",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 4] = [a, b, 1, 2]; for i in 0..4 { let _q = i * a; break; } xs[3] }",
    ),
    (
        "Break",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 4] = [a, b, 1, 2]; for i in 0..4 { let _q = i * a; break; } xs[3] }",
    ),
    (
        "BreakIf",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 4] = [a, b, 1, 2]; for i in 0..4 { let _q = i * a; break; } xs[3] }",
    ),
    (
        "PushImmediate",
        "fn main(a: Word, b: Word) -> Word { let xs: [Word; 4] = [a, b, 1, 2]; for i in 0..4 { let _q = i * a; break; } xs[3] }",
    ),
    (
        "GetData",
        "private data d { v: Word }\nfn main(a: Word, b: Word) -> Word { d.v = a; d.v }",
    ),
    (
        "SetData",
        "private data d { v: Word }\nfn main(a: Word, b: Word) -> Word { d.v = a; d.v }",
    ),
];

/// Opcodes with no driven witness HERE, and why. **Recorded, not omitted.**
const NO_WITNESS_HERE: &[(&str, &str)] = &[
    (
        "CheckedDiv",
        "a Word `/` emits Div, not this -- no witness constructed here; the corpus's \
         `checked_ratio` is the producer",
    ),
    (
        "CheckedMod",
        "a Word `%` emits Mod, not this -- same producer as CheckedDiv",
    ),
    (
        "BoundsCheck",
        "emitted by indexing a DATA-SLOT array, not a local one -- `opcode_witness.kel` \
         is the corpus's only producer. A local `xs[a]` emits NONE, which is the very \
         premise that once caused a defect here: the emitter assumed the compiler \
         emitted it before an index, and the compiler does not",
    ),
    ("Dup", "emitted incidentally; no witness isolates it"),
    (
        "Stream",
        "a stream entry; driven by the general-stream suites",
    ),
    (
        "Yield",
        "a stream entry; driven by the general-stream suites",
    ),
    ("Reset", "emitted but never visited; see opcode_denominator"),
    ("Len", "the reference emits none; see opcode_denominator"),
    (
        "IsStruct",
        "the reference emits none; see opcode_denominator",
    ),
    (
        "CallVerifiedNative",
        "needs a registered native; driven by the corpus differential",
    ),
    (
        "CallExternalNative",
        "needs a registered native; driven by the corpus differential",
    ),
    (
        "GetDataIndexed",
        "needs an indexed slot; driven by the indexed-composite suite",
    ),
    (
        "SetDataIndexed",
        "needs an indexed slot; driven by the indexed-composite suite",
    ),
];

fn declared_opcodes() -> BTreeSet<String> {
    let src =
        std::fs::read_to_string("../src/bytecode.rs").expect("the instruction set is readable");
    let start = src.find("pub enum Op {").expect("the `Op` enum is present");
    let body = &src[start..];
    let end = body.find("\n}\n").expect("the `Op` enum terminates");
    let body = &body[..end];
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let t = line.trim();
        if !line.starts_with("    ") || line.starts_with("     ") {
            continue;
        }
        let Some(name) = t.split(['(', ',', ' ']).next() else {
            continue;
        };
        if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        if !(t.contains('(') || t.ends_with(',')) {
            continue;
        }
        out.insert(name.to_string());
    }
    out
}

fn ops_of(m: &keleusma::bytecode::Module) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in &m.chunks {
        for o in &c.ops {
            let n = format!("{o:?}");
            s.insert(n.split(['(', ' ', '{']).next().unwrap().to_string());
        }
    }
    s
}

/// **Every opcode is in exactly one column.**
#[test]
fn the_two_columns_partition_the_instruction_set() {
    let declared = declared_opcodes();
    assert!(
        declared.len() >= 60,
        "parsed only {} opcodes; a broken probe rather than a shrunken ISA",
        declared.len()
    );
    let driven: BTreeSet<&str> = WITNESSED.iter().map(|(o, _)| *o).collect();
    let recorded: BTreeSet<&str> = NO_WITNESS_HERE.iter().map(|(o, _)| *o).collect();

    let both: Vec<&&str> = driven.intersection(&recorded).collect();
    assert!(
        both.is_empty(),
        "{both:?} are listed as BOTH driven here and not driven here"
    );

    let covered: BTreeSet<String> = driven.union(&recorded).map(|s| (*s).to_string()).collect();
    let unclassified: Vec<&String> = declared.difference(&covered).collect();
    let stale: Vec<&String> = covered.difference(&declared).collect();
    assert!(
        unclassified.is_empty(),
        "{} opcode(s) are in neither column: {unclassified:?}. Give each one a \
         driven witness, or record why it has none here. Do not leave it out.",
        unclassified.len()
    );
    assert!(
        stale.is_empty(),
        "{} listed opcode(s) are no longer declared upstream: {stale:?}. Delete \
         the rows.",
        stale.len()
    );
}

/// **Every witness compiles, emits its opcode, and agrees.**
#[test]
fn every_witness_emits_its_opcode_and_agrees_with_the_reference() {
    assert!(
        WITNESSED.len() >= 30,
        "only {} witnesses; a shortened table passes while proving less",
        WITNESSED.len()
    );
    for (opcode, src) in WITNESSED {
        let m = common::try_build(src)
            .unwrap_or_else(|| panic!("the witness for `{opcode}` no longer compiles: {src}"));
        // **A witness that does not emit its opcode proves nothing about it.**
        assert!(
            ops_of(&m).contains(*opcode),
            "the witness for `{opcode}` compiles but does not emit it: {src}. It \
             would then agree while saying nothing about this opcode."
        );
        let (vm, native) = common::vm_and_native_two_arg(src, 3, 4);
        assert_eq!(
            vm, native,
            "`{opcode}` DIVERGES: reference {vm}, native {native}, for {src}"
        );
    }
}
