//! **DOES THE REFERENCE COMPILER EMIT `Op::Len` FROM ANY SOURCE? SEARCHED, AND
//! NOT FOUND — WHICH IS NOT THE SAME AS UNREACHABLE.**
//!
//! Absorption 51 brought the `v0.2.3` line's `Op::Len` root repair and the only
//! witness this line had went away, reopening a question that had been treated
//! as settled. This file is the instrument that answers it, and it is deliberately
//! built out of FOUR INDEPENDENT LEGS rather than one.
//!
//! # Why four legs and not a grep
//!
//! This line ran three textual censuses of the backend's refusal surface and had
//! all three falsified by their own controls. A grep counts MENTIONS. It cannot
//! tell a construction from a match arm from a sentence in a comment, and every
//! `Op::Len` occurrence in the compiler today is one of the latter two. A textual
//! scan is admissible here only as a supporting leg, and only carrying a control
//! that proves the scanner can see a real occurrence.
//!
//! # What each leg can and cannot establish
//!
//! | leg | establishes | cannot establish |
//! |---|---|---|
//! | detector control | the detector reports TRUE on a module that really carries the opcode | anything about the compiler |
//! | construct battery | these specific source shapes do not emit it | that no other shape does |
//! | corpus sweep | no program anyone has actually written emits it | anything about programs not written |
//! | source scan | the compiler holds no construction expression today | that construction cannot arrive indirectly |
//!
//! **Together they are a search, not a proof.** The verdict recorded in
//! `docs/decisions/OP_LEN_PRODUCER_CENSUS.md` is *no producer found, by these
//! four methods, with these limits*.
//!
//! # THE WORD THAT IS NOT USED HERE
//!
//! **Unreachable.** This tree carries a retraction on exactly that word:
//! `Op::IsStruct` was declared producerless and four producers were found within
//! the hour. Nothing in this file says `Op::Len` cannot be emitted. It says what
//! was searched and what was found.
//!
//! # THE MECHANISM, CORRECTED
//!
//! The handoff and the reverse prompt both recorded that *"`static_for_in_length`
//! gained an `Expr::If` arm"*. **It did not, and the claim propagated through
//! three documents without being read against the source.**
//! `structural_for_in_length` still matches exactly `ArrayLiteral`, `Call`,
//! `FieldAccess`, `Ident`, `ArrayIndex` and `Match`, then `_ => None`. What folds
//! the `if` form is `static_for_in_length`'s FALLBACK to `infer_expr_type` plus
//! `array_length_of_type`, which consults the authoritative per-span type table
//! and therefore answers for expression forms whose structural arms are absent.
//!
//! And the fold is only half of it: **both `Op::Len` emission sites in
//! `src/compiler.rs` are gone**, each replaced by a fold or by a compile error
//! that names the unfoldable length.

mod common;

use common::{IF_SOURCE, IF_SOURCE_EQUAL_LENGTHS, PLAIN_SOURCE, build, corpus, emits, try_build};
use keleusma::bytecode::Op;

/// **LEG 1 — THE DETECTOR MUST BE ABLE TO SEE THE OPCODE.**
///
/// Every negative below is worthless if the detector cannot report a positive.
/// A module is compiled, checked to be clean, then has `Op::Len` INJECTED as
/// bytecode, and the detector must flip. This is the only leg here that does not
/// depend on the compiler at all, which is exactly why it is the control.
#[test]
fn the_detector_reports_true_for_a_module_that_really_carries_the_opcode() {
    let mut m = build(PLAIN_SOURCE);
    assert!(
        !emits(&m, "Len"),
        "the control program already carries Op::Len, so injecting it proves \
         nothing about the detector"
    );
    let entry = m
        .entry_point
        .expect("the control program has an entry point");
    m.chunks[entry].ops.insert(0, Op::Len);
    assert!(
        emits(&m, "Len"),
        "THE DETECTOR CANNOT SEE AN INJECTED `Op::Len`. Every negative verdict \
         in this file is then vacuous, and the census document must be withdrawn \
         rather than merely re-measured."
    );
}

/// **LEG 2 — THE CONSTRUCT BATTERY, AND THE SEARCH IS RECORDED BESIDE THE RESULT.**
///
/// A negative without its search reads as *"I looked"* when it means *"I looked
/// at these"*. Each probe is reported with the STAGE that disposed of it, because
/// "refused" without naming the stage is not a result: lexing, parsing, type
/// checking and compilation are different answers with different implications.
///
/// **A probe that fails to compile is not a miss and is not counted as one.** It
/// is a shape the language does not admit, reported as such.
#[test]
fn no_probed_construct_emits_the_opcode() {
    // The omitted-arm space. `structural_for_in_length` handles ArrayLiteral,
    // Call, FieldAccess, Ident, ArrayIndex and Match; everything below either
    // sits outside that list or tries to defeat the type-table fallback that
    // now answers for the rest.
    let probes: &[(&str, &str)] = &[
        ("for-in over an if-expression", IF_SOURCE),
        (
            "for-in over an if-expression, equal arm lengths",
            IF_SOURCE_EQUAL_LENGTHS,
        ),
        (
            "for-in over a NESTED if-expression",
            "fn f(c: bool, d: bool) -> Word { let a = [1, 2]; let b = [3, 4]; let e = [5, 6]; \
             for x in if c { if d { a } else { b } } else { e } { let _q = x; } 0 }\n\
             fn main() -> Word { f(true, true) }",
        ),
        (
            "for-in over an if-expression whose arms are CALLS",
            "fn a() -> [Word; 2] { [1, 2] }\nfn b() -> [Word; 2] { [3, 4] }\n\
             fn f(c: bool) -> Word { for x in if c { a() } else { b() } { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over an if-expression of ARRAY LITERALS",
            "fn f(c: bool) -> Word { for x in if c { [1, 2] } else { [3, 4] } { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over a match expression",
            "fn f(c: Word) -> Word { let a = [1, 2]; let b = [3, 4]; \
             for x in match c { 0 => a, _ => b } { let _q = x; } 0 }\n\
             fn main() -> Word { f(0) }",
        ),
        (
            "for-in over an INDEX of an if-expression",
            "fn f(c: bool) -> Word { let a = [[1, 2], [3, 4]]; let b = [[5, 6], [7, 8]]; \
             for x in if c { a } else { b }[0] { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over a FIELD of an if-expression",
            "struct S { v: [Word; 2] }\n\
             fn f(c: bool) -> Word { let a = S { v: [1, 2] }; let b = S { v: [3, 4] }; \
             for x in if c { a } else { b }.v { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over a PARENTHESISED if-expression",
            "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
             for x in (if c { a } else { b }) { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over an if-expression with a limit clause",
            "fn f(c: bool) -> Word { let a = [1, 2]; let b = [3, 4]; \
             for x in if c { a } else { b } limit 2 { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over an if-expression of TUPLES",
            "fn f(c: bool) -> Word { let a = (1, 2); let b = (3, 4); \
             for x in if c { a } else { b } { let _q = x; } 0 }\n\
             fn main() -> Word { f(true) }",
        ),
        (
            "for-in over an if-expression of DIFFERENT lengths, generic feeder",
            "fn pick<const N: Word>(a: [Word; N]) -> [Word; N] { a }\n\
             fn f(c: bool) -> Word { for x in if c { pick([1, 2]) } else { pick([3, 4]) } \
             { let _q = x; } 0 }\nfn main() -> Word { f(true) }",
        ),
        (
            "checked index over a Multiword body",
            "fn f(i: Word) -> Word { let m = 5 as Multiword<2, 0>; let _q = m[i]; 0 }\n\
             fn main() -> Word { f(0) }",
        ),
        (
            "checked index over an array with a runtime index",
            "fn f(i: Word) -> Word { let a = [1, 2, 3]; a[i] }\n\
             fn main() -> Word { f(1) }",
        ),
    ];

    println!("\n================ THE `Op::Len` CONSTRUCT BATTERY");
    let mut compiled = 0usize;
    let mut emitting: Vec<&str> = Vec::new();
    for (name, src) in probes {
        match try_build(src) {
            None => println!("  {name:58}  REFUSED before codegen"),
            Some(m) => {
                compiled += 1;
                let e = emits(&m, "Len");
                println!("  {name:58}  compiles, emits Len: {e}");
                if e {
                    emitting.push(name);
                }
            }
        }
    }
    println!("  ------------------------------------------------------------");
    println!("  probes: {}, reaching codegen: {compiled}", probes.len());
    println!("================\n");

    // **NON-VACUITY: the battery must actually reach the compiler.** A battery in
    // which every probe failed to parse would report "none emits Len" while
    // having tested nothing. This floor is deliberately well below the count
    // observed, so a probe becoming a type error does not fail the wrong test.
    assert!(
        compiled >= 6,
        "only {compiled} of {} probes reached code generation, so this battery is \
         mostly measuring the grammar rather than the emitter. Read the printed \
         table before trusting any negative here.",
        probes.len()
    );

    assert!(
        emitting.is_empty(),
        "A PROBED CONSTRUCT EMITS `Op::Len`: {emitting:?}. That is NEWS and it is \
         good news -- a source-level witness exists again. Re-point the witness \
         registry in `tests/common/mod.rs` at it, restore the corpus claim in \
         `refused_witness.kel`, and rewrite the verdict in \
         `docs/decisions/OP_LEN_PRODUCER_CENSUS.md`. Do NOT weaken this assertion."
    );
}

/// **LEG 3 — NO PROGRAM ANYONE HAS ACTUALLY WRITTEN EMITS IT.**
///
/// The battery is shapes this line thought of. The corpus is shapes the project
/// actually contains, which is a different and independent population.
#[test]
fn no_corpus_module_emits_the_opcode() {
    let mods = corpus();
    assert!(
        mods.len() >= 20,
        "the corpus loader found only {} modules. A sweep over a corpus that \
         failed to load reports no emissions for the wrong reason.",
        mods.len()
    );

    let carrying: Vec<&str> = mods
        .iter()
        .filter(|(_, m)| emits(m, "Len"))
        .map(|(n, _)| n.as_str())
        .collect();

    println!("\n================ THE CORPUS SWEEP FOR `Op::Len`");
    println!("  modules compiled : {}", mods.len());
    println!("  carrying Op::Len : {}", carrying.len());
    for n in &carrying {
        println!("    {n}");
    }
    println!("================\n");

    // **THE SWEEP'S OWN CONTROL.** The loader and the detector are exercised
    // together against an injected occurrence, so "none carry it" cannot mean
    // "the sweep looked at nothing".
    let mut probe = mods
        .iter()
        .find_map(|(_, m)| m.entry_point.map(|e| (m.clone(), e)))
        .expect("at least one corpus module has an entry point");
    probe.0.chunks[probe.1].ops.insert(0, Op::Len);
    assert!(
        emits(&probe.0, "Len"),
        "the sweep cannot detect an injected occurrence in a corpus module, so \
         its negative verdict is vacuous"
    );

    assert!(
        carrying.is_empty(),
        "A CORPUS MODULE EMITS `Op::Len`: {carrying:?}. The corpus has a witness \
         again -- restore the `WITNESSES:` claim in that file and re-measure the \
         coverage census rather than editing this assertion."
    );
}

/// **LEG 4 — THE COMPILER HOLDS NO CONSTRUCTION EXPRESSION, PINNED AS A RATCHET.**
///
/// **This leg is textual and therefore the weakest of the four**, which is why it
/// is stated as a ratchet on a known-inert set rather than as a verdict. It
/// asserts that every `Op::Len` occurrence in `src/compiler.rs` is one the reader
/// has already classified — a comment, or one of the file's own assertions that
/// the opcode is NOT emitted. A new occurrence fails here and asks for a reading.
///
/// **`src/` belongs to the `v0.2.3` line and is read-only here.** This leg
/// observes that file; it does not modify it.
///
/// # The control that makes the scan non-vacuous
///
/// The same scanner is run over `src/wire_format.rs`, which really does construct
/// `Op::Len` in its decoder. A scanner that reported zero there would be
/// reporting zero everywhere.
#[test]
fn the_compiler_holds_no_op_len_construction() {
    fn occurrences(path: &str) -> Vec<(usize, String)> {
        let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        src.lines()
            .enumerate()
            .filter(|(_, l)| l.contains("Op::Len"))
            .map(|(i, l)| (i + 1, l.trim().to_string()))
            .collect()
    }

    // THE CONTROL FIRST. If this is empty the scanner is broken and nothing
    // below means anything.
    let decoder = occurrences("../src/wire_format.rs");
    assert!(
        !decoder.is_empty(),
        "the scanner finds no `Op::Len` in the wire-format decoder, which really \
         does construct it. The scanner is broken and leg 4 is vacuous."
    );

    let found = occurrences("../src/compiler.rs");
    println!("\n================ `Op::Len` IN THE COMPILER");
    println!(
        "  control -- occurrences in wire_format.rs : {}",
        decoder.len()
    );
    println!(
        "  occurrences in compiler.rs               : {}",
        found.len()
    );
    for (n, l) in &found {
        println!("    {n:>6}  {l}");
    }
    println!("================\n");

    // Every occurrence must be inert: a comment, or an assertion that the opcode
    // is absent. Anything else is a candidate construction and wants a reading.
    let live: Vec<&(usize, String)> = found
        .iter()
        .filter(|(_, l)| !(l.starts_with("//") || l.starts_with("///") || l.contains("assert!")))
        .collect();
    assert!(
        live.is_empty(),
        "`src/compiler.rs` carries an `Op::Len` occurrence that is neither a \
         comment nor an absence assertion: {live:?}. It may be a construction, \
         which would mean a source-level producer exists again. READ IT before \
         changing anything here; do not extend the inert classification to make \
         this pass."
    );
}
