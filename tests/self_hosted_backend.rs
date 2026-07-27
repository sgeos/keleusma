//! Integration tests for the self-hosted compile backend (`keleusma::selfhost`), the
//! `keleusma-cli --compiler self-hosted` path. Gated on the `self-host` feature.
#![cfg(feature = "self-host")]

use keleusma::Arena;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::selfhost::{SelfHostError, self_hosted_compile};
use keleusma::target::Target;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, required_persistent_capacity_for};

/// For an in-subset program at the host target, the self-hosted pipeline's compiled code
/// (each chunk's ops, constant pool, and local count) is byte-identical to the reference
/// compiler — the differential-oracle guarantee the backend relies on. (The from-scratch
/// module SCAFFOLD — data layout, header bounds — is only proven byte-identical to the
/// reference for the loop-free stage sources, so this checks the code, not `to_bytes`.)
/// The produced module must also load and verify, proving it is a valid, runnable module.
#[test]
fn self_hosted_matches_reference_for_in_subset_program() {
    let src = "fn add(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { add(2, 3) }";
    let sh = self_hosted_compile(src, &Target::host()).expect("in-subset program compiles");
    let reference = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert_eq!(
        sh.chunks.len(),
        reference.chunks.len(),
        "self-hosted chunk count must match the reference",
    );
    for (m, r) in sh.chunks.iter().zip(reference.chunks.iter()) {
        assert_eq!(m.name, r.name, "chunk order");
        assert_eq!(m.ops, r.ops, "self-hosted ops must match reference for `{}`", r.name);
        assert_eq!(m.constants, r.constants, "constant pool for `{}`", r.name);
        assert_eq!(m.local_count, r.local_count, "local_count for `{}`", r.name);
    }
    // The from-scratch module must be valid: it loads and verifies in the VM.
    let need = required_persistent_capacity_for(&sh);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    Vm::new(sh, &arena).expect("self-hosted module must load and verify");
}

/// A non-host target is refused up front (the pipeline is only validated at host width).
#[test]
fn non_host_target_is_refused() {
    let src = "fn add(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { add(2, 3) }";
    match self_hosted_compile(src, &Target::embedded_16()) {
        Err(SelfHostError::NonHostTarget) => {}
        other => panic!("expected NonHostTarget, got {other:?}"),
    }
}

/// An out-of-subset program (float arithmetic) is rejected loudly as `Unsupported`, not
/// silently mis-compiled. The self-hosted pipeline mis-compiles floats to a
/// valid-but-wrong module without panicking; `self_hosted_compile`'s reference
/// cross-check catches the divergence so the CLI can print the retry hint.
#[test]
fn out_of_subset_program_is_rejected_loudly() {
    let src = "fn main() -> Float { 1.0 + 2.0 }";
    match self_hosted_compile(src, &Target::host()) {
        Err(SelfHostError::Unsupported { .. }) => {}
        other => panic!("expected Unsupported for an out-of-subset (float) program, got {other:?}"),
    }
}
