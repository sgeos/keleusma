//! **THE BACKEND CHECKS THE HOST'S ENDIANNESS; THE CALLER PICKS THE TARGET.**
//!
//! `check_target_endianness` is a `cfg!(target_endian)` on the BUILD HOST. Its
//! own comment says it is not sufficient for cross-compilation to a big-endian
//! target, and the obvious reading is that this is latent, because the library
//! has no `TargetTriple` and no `create_target_machine`.
//!
//! **That reading is wrong.** Every public entry point is lowering-only: they
//! return an LLVM module and the CALLER supplies the target machine. So an
//! embedder can lower here and emit for a big-endian target, and the host check
//! passes while shared slots — stored little-endian — are byte-swapped by an
//! LLVM load.
//!
//! **The library cannot detect that, because it never sees the target.** The
//! obligation is the caller's, and this file is where the tree says so.
//!
//! # What this is
//!
//! A RATCHET, in the shape the `v0.2.3` line used for `Op::Len`. It pins the
//! precondition that makes the present arrangement defensible — that the public
//! surface selects no target — so that when someone adds target selection the
//! test fails and names the obligation to move the check.
//!
//! **It does not repair the hazard.** Pinning a hazard is not discharging it.
//!
//! # What cannot be tested here
//!
//! The refusal branch. It is unreachable on every machine this project builds
//! on, so no test here can exercise it. That is a limit on the evidence and is
//! stated rather than papered over.
use std::path::Path;

fn lib_source() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("lib.rs"),
    )
    .expect("read the backend source")
}

/// Signatures of the public entry points.
fn public_signatures(src: &str) -> Vec<String> {
    src.lines()
        .filter(|l| l.trim_start().starts_with("pub fn "))
        .map(|l| l.trim().to_owned())
        .collect()
}

#[test]
fn the_public_surface_selects_no_target_so_the_obligation_is_the_caller_s() {
    let src = lib_source();
    let sigs = public_signatures(&src);

    // REACH. A scan that found nothing would satisfy the emptiness check below
    // while measuring nothing at all.
    assert!(
        sigs.len() >= 5,
        "only {} public functions were found, so this scan is mis-scoped and would \
         report 'no target selection' for a source it failed to read",
        sigs.len()
    );

    let selects_target: Vec<&String> = sigs
        .iter()
        .filter(|s| {
            s.contains("TargetMachine") || s.contains("TargetTriple") || s.contains("triple")
        })
        .collect();

    assert!(
        selects_target.is_empty(),
        "A public entry point now selects a target: {selects_target:?}\n\n\
         THE ENDIANNESS OBLIGATION HAS MOVED. `check_target_endianness` tests the \
         BUILD HOST. That was defensible only while the caller chose the target and \
         carried the obligation. Now that this crate can be pointed at a target, the \
         check must move onto the TargetMachine, or big-endian emission will pass a \
         host check and byte-swap little-endian shared slots."
    );
}

#[test]
fn the_endianness_check_is_on_the_host_and_the_source_says_so() {
    let src = lib_source();

    // The claim above rests on WHERE the check looks. If it stopped being a
    // host-level cfg, the reasoning in this file would need rewriting rather than
    // its assertions relaxing.
    let idx = src
        .find("pub fn check_target_endianness")
        .expect("the endianness guard exists");
    let body: String = src[idx..].chars().take(400).collect();

    assert!(
        body.contains("cfg!(target_endian"),
        "check_target_endianness no longer tests the build host. This file's \
         reasoning assumes it does; revisit the reasoning, not this assertion.\n{body}"
    );
}
