//! **EVERY PANIC-CAPABLE SITE IN THE EMITTER THAT ENCODES A MODULE ASSUMPTION.**
//!
//! # Why, and the evidence that the class is live
//!
//! One increment ago an `assert_eq!` in `lower_chunk` was found reachable from a
//! module `lower_module` accepts — retarget a branch to a valid-but-wrong index
//! and two edges reach one block at different operand depths. **`lower_module` is
//! a public entry point that does not require a verified module**, and this
//! package had already converted 58 panics on it into refusals.
//!
//! **That one was found by hand**, because `lowering_robustness.rs` mutates
//! opcodes and the defect needed a jump target moved. This is the deliberate
//! version of the same question.
//!
//! # Scope, and what is deliberately excluded
//!
//! `src/lib.rs` carries **208 `.unwrap()` calls**, overwhelmingly on inkwell
//! builder results that cannot fail, and 46 `.expect(`s of which most name the
//! same kind of infallible API. **A table nobody maintains is worse than none** —
//! the premise census measured a proposed widening at 29 rows to 132 and declined
//! it — so the matcher keeps only sites whose MESSAGE names module vocabulary.
//!
//! **The message decides, not the construct.** `.expect("a stream declares its
//! resume parameter")` is a claim about the module;
//! `.expect("the builder is positioned")` is not.
//!
//! # The families, and what a false assumption would cost
//!
//! | family | sites | disposition |
//! |---|---|---|
//! | the three trailing pointers are present | 11 | **agreement by construction** — the signature and `DataCtx` are built from one predicate, `has_data \|\| needs_region`. Two routes were tested by construction: a data op in a module with NO layout, and a `NewComposite` inserted after compilation. **Both refuse before reaching the site.** |
//! | a lowered chunk or native returns a value, never void | 3 | claims about LLVM function types **this emitter itself declared**; a module cannot contradict them |
//! | a general stream has a loop top and a resume parameter | 3 | reached only when `general_stream` holds, and `param_count != 1` is REFUSED earlier, so the arity claim is established before the site |
//! | the slot was just found in this table; mid-chunk | 2 | internal — the preceding statement establishes it |
//! | `PRIVATE_SLOT_BYTES` is a power of two | 2 | a `const`, checked where it is declared |
//! | the visit vector is parallel to the chunk table | 1 | **`debug_assert_eq!`, so it panics in the configuration this suite runs.** The early return above it handles the module-refusal case, and the `resize` below it makes the postcondition true regardless — so the assertion is a statement about the loop having run, not a guard the caller depends on |
//!
//! **The family counts sum to 22 and the recorded total is 22.** Stated because a
//! first draft's families summed to 23 against a count of 21 and neither figure
//! was right: the probe that produced them used a regex whose word boundary
//! silently skipped `debug_assert_eq!`, and the table double-counted the frame
//! sites. **An arithmetic that does not close is the cheapest available signal
//! that a population was not actually read.**
//!
//! # What this census can and cannot do
//!
//! It enumerates and forces a disposition. **It does not prove any site
//! unreachable** — two were tested by construction and the rest are argued. An
//! argument recorded is worth more than an argument re-derived, and less than a
//! test; the table says which is which.

mod common;

/// Message vocabulary that marks an assumption about the MODULE rather than
/// about the compiler's own infallible API.
const MODULE_VOCAB: &[&str] = &[
    "chunk",
    "stream",
    "module",
    "slot",
    "table",
    "parameter",
    "declares",
    "data",
    "operand",
    "entry",
    "signature",
    "composite",
    "native",
];

/// Panic-capable constructs. `.unwrap()` is excluded by design — see the header.
const PANIC_FORMS: &[&str] = &[
    "assert!",
    "assert_eq!",
    "panic!(",
    "unreachable!(",
    ".expect(\"",
];

/// Sites at the stamp.
const RECORDED_PANIC_SITES: usize = 22;
// 22 at first derivation, 2026-09-12, across six families. The count excludes
// everything below the `#[cfg(test)]` module: a unit test's assertion is the
// point of the unit test.

fn sites() -> Vec<(usize, String)> {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    let mut out = Vec::new();
    for (i, l) in src.lines().enumerate() {
        // The emitter's own unit tests assert deliberately; stop there.
        if l.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        if PANIC_FORMS.iter().any(|f| l.contains(f)) {
            let lower = l.to_lowercase();
            if MODULE_VOCAB.iter().any(|v| lower.contains(v)) {
                out.push((i + 1, l.trim().chars().take(100).collect()));
            }
        }
    }
    out
}

#[test]
fn every_panic_site_encoding_a_module_assumption_is_dispositioned() {
    let found = sites();
    println!("\n================ PANIC-CAPABLE SITES WITH A MODULE ASSUMPTION");
    for (n, l) in &found {
        println!("  src/lib.rs:{n}  {l}");
    }
    println!("  ------------------------------------------------");
    println!("  sites: {}", found.len());
    println!(
        "\n  A PANIC on `lower_module` is unrecoverable for a caller, and that\n  \
         entry point does not require a verified module. Each site is placed in\n  \
         a family in this file's header with what a false assumption costs.\n================\n"
    );

    // **NON-VACUITY, and specifically that the filter keeps the known sites.**
    assert!(
        found
            .iter()
            .any(|(_, l)| l.contains("declares the private pointer")),
        "the matcher no longer finds the trailing-pointer family, so either the \
         emitter was rewritten or the vocabulary filter has gone stale"
    );

    // **AND THAT IT DOES NOT SWALLOW THE INFALLIBLE ONES.** The header's whole
    // argument for a maintainable table is that this filter excludes them; if it
    // stopped excluding, the count would balloon and the table would rot.
    let all_forms = {
        let src = std::fs::read_to_string("src/lib.rs").expect("readable");
        src.lines()
            .take_while(|l| !l.trim_start().starts_with("#[cfg(test)]"))
            .filter(|l| PANIC_FORMS.iter().any(|f| l.contains(f)))
            .count()
    };
    assert!(
        all_forms > found.len() * 2,
        "the vocabulary filter is keeping {} of {all_forms} panic-capable lines. It \
         is supposed to keep a small minority — the module assumptions — and \
         exclude the infallible-API calls that dominate the file. A filter that \
         keeps most of them produces a table nobody maintains.",
        found.len()
    );

    assert_eq!(
        found.len(),
        RECORDED_PANIC_SITES,
        "the number of panic-capable sites encoding a module assumption has \
         changed. Do not patch the number: place the new site in a family in this \
         file's header, and say what a caller would see if it fired. `lower_module` \
         is a public entry point that does not require a verified module, and the \
         last unclassified assertion of this kind was reachable."
    );
}
