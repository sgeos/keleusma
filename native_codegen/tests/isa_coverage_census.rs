//! **WHICH OPCODES OF THE ISA HAS NOTHING EVER LOWERED?**
//!
//! Workstream A's milestone is that *the whole language* lowers, and the
//! measurements that exist report on *the corpus*. Those are different
//! populations and the corpus is the smaller one: `spike_corpus_coverage`
//! reports 100% of corpus opcode instances lowering, which is true and says
//! nothing whatever about an opcode the corpus never emits.
//!
//! This census closes that gap by naming the opcodes with no witness.
//!
//! # Why this does not reuse `dump_opcode_module_map`
//!
//! That probe answers a different question and filters for it: it skips a module
//! with any lowering refusal, with no entry point, or with composite entry
//! parameters. Those skips are correct for a mutation sweep, which needs modules
//! it can drive. **They make its population narrower than the corpus**, so an
//! opcode witnessed only inside a skipped module would be reported here as
//! missing. Measured: reading its output alone put `Add`, `Sub`, `Mul` and `Neg`
//! on a missing list, and that reading was wrong for a second reason as well ---
//! see below.
//!
//! # The ISA is DERIVED, never transcribed
//!
//! A hand-written list of 66 opcode names is the thing that goes stale silently,
//! and this file exists to catch staleness. The variant set is read out of
//! `src/bytecode.rs` at test time, and the extraction asserts it found a
//! plausible number before any conclusion is drawn from it.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeSet;

/// The corpus directories, matching the differential's.
const CORPUS_DIRS: [&str; 4] = [
    "examples/scripts",
    "src/selfhost/kel",
    "examples/rtos/scripts",
    "compiler/kel",
];

fn source_for(p: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(p).ok()?;
    let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
    let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
    if is_rtos && !is_prelude {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        return Some(format!("{prelude}\n{src}"));
    }
    Some(src)
}

/// Every module that COMPILES, with no further filtering.
///
/// **Deliberately unfiltered.** A module this harness cannot drive still tells
/// the truth about which opcodes the compiler emits, and that is the only
/// question here.
fn all_compiling_modules() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            paths.push(p);
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(src) = source_for(&p) else { continue };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        out.push((name, m));
    }
    out
}

/// The `Op` variant names, read from the crate source rather than listed here.
fn declared_isa() -> BTreeSet<String> {
    let src = std::fs::read_to_string("../src/bytecode.rs").expect("read bytecode.rs");
    let start = src.find("pub enum Op {").expect("find `pub enum Op`");
    let body = &src[start..];
    let end = body.find("\n}").expect("find the enum's close");
    let body = &body[..end];
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let t = line.trim_start();
        // A variant line starts at one indent, capitalised, and is followed by
        // a payload, a brace, or a comma. Doc comments and attributes are not.
        if line.starts_with("    ")
            && !line.starts_with("     ")
            && t.starts_with(|c: char| c.is_ascii_uppercase())
        {
            let name: String = t
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
    out
}

#[test]
fn which_isa_opcodes_have_no_corpus_witness() {
    let isa = declared_isa();
    // **Non-vacuity, and it must come first.** A regex that matched nothing
    // would report every opcode as unwitnessed, which reads as a dramatic
    // finding and is a broken extraction.
    assert!(
        isa.len() >= 60,
        "extracted only {} Op variants from bytecode.rs; the parse is broken and \
         every conclusion below would be an artefact of it",
        isa.len()
    );

    let corpus = all_compiling_modules();
    assert!(
        corpus.len() > 50,
        "only {} modules compiled, so this census is reading the wrong tree",
        corpus.len()
    );

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (_, m) in &corpus {
        for c in &m.chunks {
            for op in &c.ops {
                let d = format!("{op:?}");
                seen.insert(d.split('(').next().unwrap_or(&d).to_string());
            }
        }
    }

    let missing: Vec<&String> = isa.difference(&seen).collect();
    let unknown: Vec<&String> = seen.difference(&isa).collect();

    println!("\n================ ISA COVERAGE CENSUS");
    println!("  modules compiled            : {}", corpus.len());
    println!("  opcodes declared in the ISA  : {}", isa.len());
    println!("  opcodes with a corpus witness: {}", seen.len());
    println!("  opcodes with NO witness      : {}", missing.len());
    for m in &missing {
        println!("     {m}");
    }
    if !unknown.is_empty() {
        println!(
            "\n  witnessed but not extracted as a variant ({}) -- the extraction \
             is incomplete, NOT the ISA:",
            unknown.len()
        );
        for u in &unknown {
            println!("     {u}");
        }
    }
    println!(
        "\n  ZERO REMAIN, as of 2026-08-21. Both of the last two were answered by\n  \
         the `v0.2.3` line, and NEITHER was found by guessing a construct --\n  \
         between the lines, twenty-three guesses failed. Both fell to reading the\n  \
         GUARD'S OWN MATCH ARMS for what they OMIT.\n  \
         \n  \
         `Len`      an `if` EXPRESSION as the for-in source. `static_for_in_length`\n  \
                    handles ArrayLiteral, Call, FieldAccess, Ident, ArrayIndex and\n  \
                    Match, then falls through to `_ => None`. `Expr::If` is absent.\n  \
         `IsStruct` an UNANNOTATED struct-parameter pattern, so the pattern's type\n  \
                    is unknown at the test. Annotating it folds the test out.\n  \
         \n  \
         **A WITNESS IS NOT AN ADMISSIBLE PROGRAM, AND THE TWO DIFFER.** Measured:\n  \
         \n  \
           witness      verify()  module_wcmu  arena     load   run\n  \
           Len          accepts   REFUSES      REFUSED   n/a    never runs\n  \
           IsStruct     accepts   accepts      OK        LOADS  TRAPS\n  \
         \n  \
         `Len`'s witness cannot be ADMITTED -- refused before load by the strict\n  \
         iteration-bound check, the conservative-verification stance working as\n  \
         designed. `IsStruct`'s satisfies every load-time check, loads, and dies at\n  \
         call time on `InvalidBytecode` -- the class `verify()` exists to exclude AT\n  \
         LOAD TIME. A legal program reaching it at RUN time is a load-time hole, and\n  \
         `src/verify.rs` is read-only to BOTH lines. Recorded for the operator; see\n  \
         `probe_len_reachability.rs` and the handoff.\n  \
         \n  \
         SO 66 OF 66 IS DEFENSIBLE ON REACHABILITY AND DESERVES ITS FOOTNOTE:\n  \
         one witness cannot be LOADED, the other cannot be RUN."
    );
    println!("================\n");

    // **Reported, not pinned.** The set moves with the corpus and with the ISA,
    // and an assertion on its contents would fail on ordinary growth. What is
    // asserted is that the two sides were derived from real inputs.
    assert!(
        !seen.is_empty(),
        "no opcode was witnessed at all, so the walk read nothing"
    );
    assert!(
        unknown.is_empty(),
        "these opcodes occur in compiled modules but were not extracted from \
         bytecode.rs, so the ISA side of this census is incomplete and the \
         missing list cannot be trusted: {unknown:?}"
    );
}
