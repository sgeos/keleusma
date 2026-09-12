//! **THE SUPPORT CENSUS HAD NO DENOMINATOR. THIS IS IT.**
//!
//! # The finding
//!
//! `backend_support_census.rs` is titled *"WHICH OPCODES CAN THE BACKEND
//! ACTUALLY LOWER?"*. Measured on this tree, the `Op` enum declares **66**
//! opcodes and that file probes **17**. Nothing in it relates its probe table to
//! the instruction set, so an opcode's absence is **unclassified**: covered by
//! another suite, unprobeable, or forgotten, and the file cannot tell a reader
//! which.
//!
//! That is not a claim that forty-nine opcodes are untested — most are driven by
//! the differential corpus. It is the narrower and worse claim that **the census
//! cannot say which**, so it cannot report progress toward the milestone it
//! exists to measure.
//!
//! # Why this instrument, and not some other
//!
//! Three defects already hid behind its keying: the `Fixed` variants of
//! `CheckedMul` and `CheckedDiv` were refused outright and the `Byte` variants of
//! `CheckedMul` and `CheckedAdd` returned untruncated values, all while it
//! reported **0 refused**. Its header now closes with the honest admission that
//! *"this file cannot know which variants exist, only which it was given."*
//!
//! This line has an idiom for precisely that — pin a population, fail on growth,
//! demand classification rather than a patched number, and always carry a
//! non-vacuity check. It has been applied to test functions, pointer offsets,
//! panic sites, value movement, host buffers and upstream premises. **It had
//! never been applied to the census whose blind spot produced three defects.**
//!
//! # Derived, not transcribed
//!
//! The denominator is parsed from `src/bytecode.rs` at test time. That file
//! belongs to the `v0.2.3` line and is read-only to this one, which is what makes
//! it a source of truth rather than a copy that rots. An opcode added upstream
//! arrives here as a failure on the next run.
//!
//! # The classification is MEASURED
//!
//! Each opcode is placed by observation, not by assertion:
//!
//! - `Lowered` — the corpus emits it and the lowering was observed to reach it.
//! - `EmittedNotLowered` — the corpus emits it and the lowering never reached it.
//! - `NotEmitted` — no corpus program emits it, so **this test has no evidence**
//!   about it either way. That is a statement about the corpus, not about the
//!   backend, and it is spelled that way deliberately.
//!
//! # ⚠ WHAT A `Lowered` VERDICT DOES NOT MEAN
//!
//! That the emitted code is correct. Being visited is a fact about the compiler,
//! not about the program. Correctness is the differential's job. It also does not
//! mean every VARIANT of that opcode lowers — that is the whole finding above,
//! and it is why `NewComposite` is additionally classified per operand variant
//! below.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma_native::LowerOptions;
use std::collections::{BTreeMap, BTreeSet};

mod common;

// ---------------------------------------------------------------------------
// THE DENOMINATOR
// ---------------------------------------------------------------------------

/// Fewer opcodes than this means the parse broke, not that the instruction set
/// shrank.
///
/// **A regex over this very file returned zero rows once already.** Extracting
/// the census's probe names with a one-line matcher yielded **0** because
/// `cargo fmt` had split the tuples across lines; a count of zero was the only
/// thing that gave it away. Every enumeration here carries a floor.
const OPCODE_FLOOR: usize = 60;

/// Opcode names declared by the runtime's instruction set, parsed from its own
/// source.
fn declared_opcodes() -> BTreeSet<String> {
    let src =
        std::fs::read_to_string("../src/bytecode.rs").expect("the instruction set is readable");
    let start = src.find("pub enum Op {").expect("the `Op` enum is present");
    let body = &src[start..];
    // The enum ends at the first line that is exactly a closing brace at column
    // zero. Matching the next `}` would stop inside a doc comment's code block.
    let end = body.find("\n}\n").expect("the `Op` enum terminates");
    let body = &body[..end];
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let t = line.trim();
        // A variant line is an identifier starting with an upper-case letter,
        // at one indent level, followed by `(` or `,`. Doc comments start `///`
        // and attributes `#`, so neither can be mistaken for one.
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

// ---------------------------------------------------------------------------
// THE MEASUREMENT
// ---------------------------------------------------------------------------

/// The corpus, composed the way the shipping host composes it.
///
/// `common::corpus()` compiles each `.kel` standalone and **silently drops**
/// whatever fails, which is how five rtos scripts were once recorded as
/// "rejected by the reference compiler" when they were really missing their
/// prelude. A denominator built on a silent drop understates the evidence, so
/// this reproduces the four-line composition `examples/rtos/src/setup.rs` uses
/// and reports the drops rather than swallowing them.
fn corpus_modules() -> (Vec<(String, Module)>, Vec<String>) {
    let mut built = Vec::new();
    let mut dropped = Vec::new();
    for p in common::corpus_sources() {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            dropped.push(name);
            continue;
        };
        let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
        let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
        let src = if is_rtos && !is_prelude {
            match std::fs::read_to_string("../examples/rtos/scripts/prelude.kel") {
                Ok(prelude) => format!("{prelude}\n{src}"),
                Err(_) => src,
            }
        } else {
            src
        };
        match common::try_build(&src) {
            Some(m) => built.push((name, m)),
            None => dropped.push(name),
        }
    }
    (built, dropped)
}

/// The bare opcode name, with any operand stripped.
fn op_name(op: &Op) -> String {
    let s = format!("{op:?}");
    s.split(['(', ' ', '{']).next().unwrap_or(&s).to_string()
}

/// Which opcodes the corpus emits, and which of those the lowering reaches.
fn observe() -> (BTreeSet<String>, BTreeSet<String>, usize) {
    let (mods, _) = corpus_modules();
    let n = mods.len();
    let mut emitted = BTreeSet::new();
    let mut lowered = BTreeSet::new();
    for (_, m) in &mods {
        let (_, visits) = keleusma_native::module_lowered_op_indices(m, LowerOptions::default());
        for (ci, c) in m.chunks.iter().enumerate() {
            let seen = visits.get(ci).and_then(|v| v.as_ref());
            for (i, op) in c.ops.iter().enumerate() {
                let name = op_name(op);
                if seen.is_some_and(|s| s.contains(&i)) {
                    lowered.insert(name.clone());
                }
                emitted.insert(name);
            }
        }
    }
    (emitted, lowered, n)
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum Class {
    Lowered,
    EmittedNotLowered,
    NotEmitted,
}

fn classify(name: &str, emitted: &BTreeSet<String>, lowered: &BTreeSet<String>) -> Class {
    if lowered.contains(name) {
        Class::Lowered
    } else if emitted.contains(name) {
        Class::EmittedNotLowered
    } else {
        Class::NotEmitted
    }
}

// ---------------------------------------------------------------------------
// THE RECORD
// ---------------------------------------------------------------------------

/// Every declared opcode and the class it was observed in.
///
/// **Not a list of what is supported.** It is the partition itself, and its
/// value is that a change to any entry is a failure rather than a silence.
const RECORDED: &[(&str, Class)] = &[
    ("Add", Class::Lowered),
    ("BitAnd", Class::Lowered),
    ("BitOr", Class::Lowered),
    ("BitXor", Class::Lowered),
    ("BoundsCheck", Class::Lowered),
    ("Break", Class::Lowered),
    ("BreakIf", Class::Lowered),
    ("ByteToWord", Class::Lowered),
    ("Call", Class::Lowered),
    ("CallExternalNative", Class::Lowered),
    ("CallVerifiedNative", Class::Lowered),
    ("CheckedAdd", Class::Lowered),
    ("CheckedDiv", Class::Lowered),
    ("CheckedMod", Class::Lowered),
    ("CheckedMul", Class::Lowered),
    ("CheckedNeg", Class::Lowered),
    ("CheckedSub", Class::Lowered),
    ("CmpEq", Class::Lowered),
    ("CmpGe", Class::Lowered),
    ("CmpGt", Class::Lowered),
    ("CmpLe", Class::Lowered),
    ("CmpLt", Class::Lowered),
    ("CmpNe", Class::Lowered),
    ("Const", Class::Lowered),
    ("Div", Class::Lowered),
    ("Dup", Class::Lowered),
    ("Else", Class::Lowered),
    ("EndIf", Class::Lowered),
    ("EndLoop", Class::Lowered),
    ("FixedDiv", Class::Lowered),
    ("FixedMul", Class::Lowered),
    ("FixedToWord", Class::Lowered),
    ("FloatToInt", Class::Lowered),
    ("GetData", Class::Lowered),
    ("GetDataIndexed", Class::Lowered),
    ("GetEnumField", Class::Lowered),
    ("GetField", Class::Lowered),
    ("GetIndex", Class::Lowered),
    ("GetLocal", Class::Lowered),
    ("GetTupleField", Class::Lowered),
    ("If", Class::Lowered),
    ("IntToFloat", Class::Lowered),
    ("IsEnum", Class::Lowered),
    // **See `the_two_unemitted_opcodes_are_refused_rather_than_mislowered`.**
    // The reference compiler is documented as not producing either: B28 P2
    // deleted every `Op::Len` fall-back because a flat array body cannot answer
    // a length query, and `Op::IsStruct` carries an upstream record of a bounded
    // search that found no producer -- recorded as a SEARCH, explicitly not as a
    // claim of unreachability, because an earlier producerless claim there was
    // falsified within the hour. This line copies that discipline: the class
    // below says this corpus emitted none, and nothing stronger.
    ("IsStruct", Class::NotEmitted),
    ("Len", Class::NotEmitted),
    ("Loop", Class::Lowered),
    ("Mod", Class::Lowered),
    ("Mul", Class::Lowered),
    ("Neg", Class::Lowered),
    // Lowered as an OPCODE. Four of its eight operand variants are never
    // emitted; see `NEW_COMPOSITE_RECORDED`, which is the point of this file.
    ("NewComposite", Class::Lowered),
    ("Not", Class::Lowered),
    ("PopN", Class::Lowered),
    ("PushImmediate", Class::Lowered),
    // **Emitted by the corpus, never reached by the lowering.** Recorded
    // independently in `backend_support_census.rs`, which reached the same
    // verdict from the other direction: its probe emits `Reset` in a position
    // the lowering steps over. The native stream rewinds its arena at the host
    // boundary rather than at this instruction, so the instruction has no work
    // to do; a visit appearing here means that changed.
    ("Reset", Class::EmittedNotLowered),
    ("Return", Class::Lowered),
    ("SetData", Class::Lowered),
    ("SetDataIndexed", Class::Lowered),
    ("SetLocal", Class::Lowered),
    ("Shl", Class::Lowered),
    ("Shr", Class::Lowered),
    ("Stream", Class::Lowered),
    ("Sub", Class::Lowered),
    ("Trap", Class::Lowered),
    ("WordToByte", Class::Lowered),
    ("WordToFixed", Class::Lowered),
    ("Yield", Class::Lowered),
];

/// The corpus built fewer modules than this means the walk broke.
const MODULE_FLOOR: usize = 60;

#[test]
fn every_declared_opcode_is_classified_and_the_class_still_holds() {
    let declared = declared_opcodes();
    assert!(
        declared.len() >= OPCODE_FLOOR,
        "parsed only {} opcode names from `src/bytecode.rs`. This is a BROKEN \
         PROBE, not a shrunken instruction set: the parse is keyed on the \
         formatting of a file this line does not own.",
        declared.len()
    );

    let (emitted, lowered, modules) = observe();
    assert!(
        modules >= MODULE_FLOOR,
        "the corpus walk built {modules} modules, below the floor of \
         {MODULE_FLOOR}. Every class below would be understated by a short walk, \
         so this is a broken probe rather than a result."
    );

    let recorded: BTreeMap<&str, Class> = RECORDED.iter().copied().collect();
    assert_eq!(
        recorded.len(),
        RECORDED.len(),
        "the record contains a duplicate opcode"
    );

    let mut unclassified = Vec::new();
    let mut moved = Vec::new();
    for name in &declared {
        match recorded.get(name.as_str()) {
            None => unclassified.push(name.clone()),
            Some(&want) => {
                let got = classify(name, &emitted, &lowered);
                if got != want {
                    moved.push(format!("{name}: recorded {want:?}, observed {got:?}"));
                }
            }
        }
    }

    // **A record outliving its subject is the failure this line has found four
    // times.** An opcode deleted upstream must take its row with it, or the row
    // becomes a standing claim about an instruction that no longer exists.
    let stale: Vec<&str> = RECORDED
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !declared.contains(*n))
        .collect();

    assert!(
        unclassified.is_empty(),
        "{} opcode(s) are declared by the instruction set and classified by \
         nothing here: {unclassified:?}. Classify each one by what this corpus \
         is observed to do with it, and say in the record why it sits where it \
         does. Do NOT add a bare row to make this pass.",
        unclassified.len()
    );
    assert!(
        stale.is_empty(),
        "{} recorded opcode(s) are no longer declared upstream: {stale:?}. Delete \
         the row rather than keeping a claim about an instruction that is gone.",
        stale.len()
    );
    assert!(
        moved.is_empty(),
        "{} opcode(s) changed class: {moved:?}. A move from `NotEmitted` means \
         the corpus grew a producer and the backend's behaviour on it is now \
         worth reading; a move INTO `EmittedNotLowered` means the lowering \
         stopped reaching an instruction it used to. Explain the move in the \
         record; do not edit the class to match.",
        moved.len()
    );
}

// ---------------------------------------------------------------------------
// THE VARIANT AXIS, WHICH IS THE WHOLE FINDING
// ---------------------------------------------------------------------------

/// Every `NewCompositeOperand` variant: two forms times four composite kinds.
///
/// **The rad-hard minimal-ISA constraint manufactures this blind spot.** The P4
/// consolidation took the instruction set from 69 opcodes to 66 by moving a
/// discriminant OUT of the opcode name and INTO an operand field, and every
/// future consolidation does the same. A name-keyed census cannot see across
/// that move, so the axis has to be enumerated separately.
///
/// The `Boxed` form is upstream-documented as transitional -- *"removed at P3
/// when reference fields become handles"* -- and the emitter matches only
/// `Flat`. Four `NotEmitted` rows are therefore the expected reading, not a gap,
/// and they will fall away with the form itself.
const NEW_COMPOSITE_RECORDED: &[(&str, Class)] = &[
    ("Boxed/Array", Class::NotEmitted),
    ("Boxed/Enum", Class::NotEmitted),
    ("Boxed/Struct", Class::NotEmitted),
    ("Boxed/Tuple", Class::NotEmitted),
    ("Flat/Array", Class::Lowered),
    ("Flat/Enum", Class::Lowered),
    ("Flat/Struct", Class::Lowered),
    ("Flat/Tuple", Class::Lowered),
];

#[test]
fn every_composite_construction_variant_is_classified_by_itself() {
    let (mods, _) = corpus_modules();
    assert!(mods.len() >= MODULE_FLOOR, "broken corpus walk");
    let mut emitted = BTreeSet::new();
    let mut lowered = BTreeSet::new();
    for (_, m) in &mods {
        let (_, visits) = keleusma_native::module_lowered_op_indices(m, LowerOptions::default());
        for (ci, c) in m.chunks.iter().enumerate() {
            let seen = visits.get(ci).and_then(|v| v.as_ref());
            for (i, op) in c.ops.iter().enumerate() {
                let Op::NewComposite(o) = op else { continue };
                let form = match o {
                    NewCompositeOperand::Flat { .. } => "Flat",
                    NewCompositeOperand::Boxed { .. } => "Boxed",
                };
                let key = format!("{form}/{:?}", o.kind());
                if seen.is_some_and(|s| s.contains(&i)) {
                    lowered.insert(key.clone());
                }
                emitted.insert(key);
            }
        }
    }

    // **Enumerated from the types, not from the record.** A fifth composite kind
    // added upstream appears here as an unclassified key rather than as silence.
    let mut observed = Vec::new();
    for form in ["Flat", "Boxed"] {
        for kind in ["Tuple", "Array", "Struct", "Enum"] {
            let key = format!("{form}/{kind}");
            observed.push((key.clone(), classify(&key, &emitted, &lowered)));
        }
    }
    assert_eq!(
        observed.len(),
        NEW_COMPOSITE_RECORDED.len(),
        "the operand variant space changed size; re-enumerate it from \
         `NewCompositeOperand` and `CompositeKind` rather than padding the record"
    );

    let recorded: BTreeMap<&str, Class> = NEW_COMPOSITE_RECORDED.iter().copied().collect();
    let mut moved = Vec::new();
    for (key, got) in &observed {
        match recorded.get(key.as_str()) {
            None => moved.push(format!("{key}: unclassified, observed {got:?}")),
            Some(&want) if want != *got => {
                moved.push(format!("{key}: recorded {want:?}, observed {got:?}"))
            }
            Some(_) => {}
        }
    }
    assert!(
        moved.is_empty(),
        "{} composite-construction variant(s) changed class: {moved:?}. A `Boxed` \
         variant becoming emitted matters most: the emitter matches only the \
         `Flat` form, so the corpus would now contain a construction the backend \
         refuses.",
        moved.len()
    );
}

// ---------------------------------------------------------------------------
// WHAT `NotEmitted` IS WORTH
// ---------------------------------------------------------------------------

/// **`NotEmitted` on its own proves nothing about the backend**, so the two
/// opcodes in that class are driven into it directly.
///
/// Both are absent from the emitter entirely -- there is no `Op::Len` or
/// `Op::IsStruct` arm -- so both reach the default refusal. That is the correct
/// behaviour for an instruction the reference does not produce, and asserting it
/// is what distinguishes *"refuses loudly"* from *"lowers to something"*. If
/// either ever grows an arm, this fails and the class above needs re-reading.
///
/// The module is MUTATED rather than compiled, because the reference will not
/// emit either opcode -- which is the very fact being relied on.
#[test]
fn the_two_unemitted_opcodes_are_refused_rather_than_mislowered() {
    for (tag, op) in [("Len", Op::Len), ("IsStruct", Op::IsStruct(0))] {
        let mut m = common::build("fn main(a: Word) -> Word { a + 1 }");
        let chunk = &mut m.chunks[0];
        let at = chunk
            .ops
            .iter()
            .position(|o| matches!(o, Op::Add | Op::CheckedAdd))
            .expect("the probe program still contains an addition to overwrite");
        chunk.ops[at] = op;
        let refusals = keleusma_native::module_refusals(&m, LowerOptions::default());
        let text = format!("{refusals:?}");
        assert!(
            text.contains("UnsupportedOp") && text.contains(tag),
            "`Op::{tag}` did not produce an `UnsupportedOp` refusal; got \
             {text}. If the backend grew an arm for it, the `NotEmitted` class \
             in `RECORDED` is now understating what is known."
        );
    }
}
