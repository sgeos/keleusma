//! **THE MANIFEST MUST SAY WHAT THIS PACKAGE USES, AND ONE FEATURE USED TO
//! ARRIVE WITHOUT BEING ASKED FOR.**
//!
//! `native_codegen/Cargo.toml` declared `features = ["compile", "self-host"]`
//! and did not disable default features. `self-host` implies `compile` and
//! `verify`, so those two were genuinely declared. **`floats` was neither
//! declared nor implied** — it reached this package only because the parent
//! crate's `default = ["compile", "verify", "floats"]` was still in force.
//!
//! # What is actually wrong with that
//!
//! **The manifest stated an intent, and the intent was incomplete.** A reader
//! concludes the backend needs `compile` and `self-host`. It also needs
//! `floats`: the float differential, the float composite tests, the declared
//! width refusals and the ABI scope tests all rest on it.
//!
//! **The realistic failure is a tidy-up, not sabotage.** Adding
//! `default-features = false` is exactly what someone would do on seeing an
//! explicit feature list, and it reads as a clarification rather than a change.
//!
//! # SCOPED HONESTLY, AND THE FIRST SCOPING WAS ITSELF WRONG
//!
//! **This is not a silent hole.** The first version of this note said the float
//! tests would "fail loudly" without the feature. **Measured, it is louder than
//! that: the package does not COMPILE.** Adding `default-features = false`
//! removes `ScalarKind::Float`, and the build stops before any test runs.
//!
//! **That has a consequence for what the tests below are worth, and it is stated
//! rather than glossed.** `the_float_feature_reaches_this_package` cannot fire by
//! the realistic route, because the realistic route never produces a runnable
//! binary. **The declaration ratchet is the guard that does the real work here**;
//! the behavioural probe is kept because it is nearly free and because its
//! message is the right one if the feature is ever lost by some route that still
//! compiles — but it must not be described as the defence.
//!
//! What the declaration buys is an accurate manifest. What the ratchet buys is
//! that the accuracy stays.
//!
//! **And it is not the runtime finding it resembles.** A float module that
//! verifies, loads, and then TRAPS on a runtime built without floats is the
//! `v0.2.3` line's `INVALID_BYTECODE_CENSUS.md` Group B — a hole in a load-time
//! guarantee, and a larger thing than a manifest that under-describes itself.
//! Conflating the two would be an overclaim, and this line has made enough of
//! those to name the pattern.

mod common;

use common::try_build;

/// **The feature is present, and the message names where to look if it is not.**
///
/// A float literal is the cheapest probe. **Its reach is narrow and is stated
/// here rather than left to be discovered**: measured against a mutation that
/// adds `default-features = false`, the package fails to COMPILE, so this test
/// never runs and cannot be what catches that case. It would fire only if the
/// feature were lost by some route that still builds.
#[test]
fn the_float_feature_reaches_this_package() {
    const FLOAT_SOURCE: &str = "fn main() -> Float { 1.5 }\n";
    assert!(
        try_build(FLOAT_SOURCE).is_some(),
        "A FLOAT PROGRAM NO LONGER COMPILES THROUGH THIS PACKAGE'S `keleusma` \
         DEPENDENCY. Look at `native_codegen/Cargo.toml` before looking \
         anywhere else: the `floats` feature has stopped reaching here, most \
         likely because `default-features = false` was added or `floats` was \
         removed from the explicit list. Roughly a dozen tests in this suite \
         rest on it -- the float differential, float composites, the declared \
         width refusals and the ABI scope tests. This is a MANIFEST question, \
         not a compiler defect."
    );
}

/// **THE MUST-FIRE CONTROL: the probe has to be able to report a negative.**
///
/// Without this, `the_float_feature_reaches_this_package` would pass for a
/// program that never exercised the feature at all, and a green result would
/// mean nothing. A source the compiler rejects for an unrelated reason proves
/// the harness distinguishes success from failure.
#[test]
fn the_probe_can_report_a_program_that_does_not_compile() {
    const NOT_A_PROGRAM: &str = "fn main() -> Word { this is not keleusma }\n";
    assert!(
        try_build(NOT_A_PROGRAM).is_none(),
        "the harness reports SUCCESS for a source that cannot compile, so the \
         float probe's positive result is vacuous and says nothing about the \
         feature"
    );
}

/// **The declaration itself is pinned, so the inheritance cannot quietly return.**
///
/// The test above passes whether `floats` is declared or merely inherited — it
/// measures the effect, not the cause. This one reads the manifest, so the
/// specific defect that was repaired stays repaired.
///
/// **It is textual and therefore weak**, which is why it is a ratchet on a known
/// state rather than a verdict, and why the behavioural test above exists
/// alongside it. A scan alone would pass on a manifest that names the feature in
/// a comment.
#[test]
fn the_float_feature_is_declared_rather_than_inherited() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("read this package's manifest");
    let dep_line = manifest
        .lines()
        .find(|l| l.trim_start().starts_with("keleusma = "))
        .expect("the manifest declares a `keleusma` dependency");
    assert!(
        dep_line.contains("\"floats\""),
        "the `keleusma` dependency no longer names `floats` in its feature list, \
         so float support is once again INHERITED from the parent crate's \
         defaults rather than declared. The behavioural probe in this file may \
         still pass -- defaults would supply it -- which is exactly why this \
         assertion is separate. Line read: {dep_line}"
    );
}
