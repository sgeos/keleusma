//! Compiling the same source twice in one process produces the same module.
//!
//! # Why this is not covered by what already exists
//!
//! The roadmap's cross-cutting list requires the self-hosted toolchain to be
//! byte-reproducible, "so the fixed-point and differential-oracle checks are
//! meaningful". Two things approach that and neither is it:
//!
//! - The **differential oracle** compares the self-hosted output against the
//!   reference. Both sides could be non-deterministic in the same way, or the
//!   check could pass on a first compile and fail on a second, and it would not
//!   notice.
//! - `selfhost_counter_reset.rs` is a **static** guard. It scans the stage
//!   sources for a counter that is never assigned zero, which is one known cause
//!   of carried state — it cost a four-cause diagnosis, and two of the four were
//!   first diagnosed wrongly. A syntactic scan cannot catch a cause nobody has
//!   thought of.
//!
//! This test is behavioural and cause-agnostic: whatever carries between compiles,
//! the second result differs and this fails.
//!
//! # What it does NOT establish
//!
//! **Reproducibility across processes or builds**, which is the stronger property
//! the roadmap ultimately wants. A global initialised once per process would
//! survive this check unchanged. What this catches is state carried from one
//! compile to the next INSIDE a process, which is the shape the counter defect
//! had and the shape a driver accumulating into a reused buffer would have.

#![cfg(all(feature = "self-host", feature = "compile", feature = "verify"))]

use keleusma::bytecode::Module;
use keleusma::target::Target;

/// Compare two modules the way the differential oracle does.
fn same_bytes(a: &Module, b: &Module) -> bool {
    a.chunks.len() == b.chunks.len()
        && a.chunks.iter().zip(b.chunks.iter()).all(|(x, y)| {
            x.name == y.name
                && x.ops == y.ops
                && x.constants == y.constants
                && x.local_count == y.local_count
        })
}

/// **THE SAME SOURCE, COMPILED TWICE, MUST GIVE THE SAME MODULE.**
///
/// The corpus is chosen for the constructs that carry per-function counters,
/// because that is where the one known defect of this class lived: a bare `for`
/// counter that was never reset, so the SECOND and every later function
/// containing one emitted a record pointing past its own parts.
///
/// Several functions per source, and several sources, so a counter that resets
/// per compile but not per function is exercised as well as one that does
/// neither.
#[test]
fn compiling_twice_in_one_process_gives_the_same_module() {
    const CORPUS: &[(&str, &str)] = &[
        (
            "two functions each with a bare for",
            "private data d { q: Word }\n\
             fn a() -> Word { for i in 0..3 limit 3 { d.q = d.q + i; } d.q }\n\
             fn b() -> Word { for j in 0..4 limit 4 { d.q = d.q + j; } d.q }\n\
             fn main() -> Word { a() + b() }",
        ),
        (
            "arithmetic and locals",
            "fn f(x: Word) -> Word { let y = x * 3; y + 1 }\n\
             fn g(x: Word) -> Word { let y = x - 2; y * y }\n\
             fn main() -> Word { f(4) + g(5) }",
        ),
        (
            "a struct field read and a conditional",
            "struct P { x: Word }\n\
             fn f(p: P) -> Word { if p.x > 0 { p.x } else { 0 } }\n\
             fn g(p: P) -> Word { p.x * 2 }\n\
             fn main(p: P) -> Word { f(p) + g(p) }",
        ),
    ];

    let target = Target::host();
    let mut compared = 0usize;

    for (label, src) in CORPUS {
        let first = keleusma::selfhost::self_hosted_compile(src, &target).unwrap_or_else(|e| {
            panic!(
                "{label}: the self-hosted compiler refuses this source ({e}). The corpus must \
                 be IN-SUBSET, or this test measures refusal rather than reproducibility"
            )
        });
        let second = keleusma::selfhost::self_hosted_compile(src, &target)
            .unwrap_or_else(|e| panic!("{label}: the SECOND compile refuses what the first accepted ({e}). That is already a reproducibility failure, before any byte comparison"));

        assert!(
            same_bytes(&first, &second),
            "{label}: compiling the same source twice in one process gave DIFFERENT modules. \
             Something carries between compiles. `selfhost_counter_reset.rs` catches one \
             syntactic shape of this (a counter never assigned zero); a difference here that \
             that guard does not explain is a cause nobody has written down yet"
        );

        // NON-VACUITY: an empty or trivial module would make the comparison hold for
        // reasons that have nothing to do with determinism.
        assert!(
            !first.chunks.is_empty() && first.chunks.iter().any(|c| !c.ops.is_empty()),
            "{label}: the compile produced no operations, so the equality above is vacuous"
        );
        compared += 1;
    }

    assert_eq!(
        compared,
        CORPUS.len(),
        "not every corpus entry was compared, so this test covers less than it names"
    );
}
