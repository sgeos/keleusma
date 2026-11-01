// The self-hosted driver's multihead decision, probed against the reference.
#![cfg(all(
    feature = "self-host",
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! Whether a group of same-named heads compiles as a multiheaded dispatch is decided
//! by the driver, not by the `.kel` stages: `reconstruct.kel` builds a dispatch when
//! the host sets `head_count`, and the host sets it only on the branch it selects.
//!
//! The driver selected that branch from the DECLARATION KEYWORD (`yield`) rather than
//! from the number of heads, so a multiheaded `fn` took the single-body branch, which
//! reads `group[0].body` and silently discards every later head and every `when`
//! guard. The reference compiler admits and lowers a multiheaded `fn`, so the two
//! disagreed with no diagnostic on either side.
//!
//! Each case here asserts against the reference rather than against a recorded op
//! sequence, so the probe cannot drift away from what the compiler actually emits.

use keleusma::bytecode::Module;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::selfhost::{compile_src, self_host_compile};

/// A source the reference REJECTS is not a self-host gap, and bad probe syntax reads
/// exactly like one. Every case asserts reference acceptance before comparing.
fn reference_accepts(src: &str) -> Module {
    compile_src(src)
}

/// Compare one named chunk between the self-hosted pipeline and the reference.
fn chunk_ops(module: &Module, name: &str) -> Vec<keleusma::bytecode::Op> {
    module
        .chunks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no chunk named `{name}`"))
        .ops
        .clone()
}

/// The defect, pinned. Two heads and a `when` guard on a plain `fn`; the reference
/// emits the guard-dispatch chain and the driver used to emit only the first head's
/// body. A regression here is a SILENT miscompile, not a rejection, which is why it
/// is asserted rather than left to the whole-stage self-compiles to notice.
#[test]
fn a_multiheaded_fn_compiles_byte_identically() {
    let src = "fn pick(r: Word) -> Word when r == 0 { 1 } \
        fn pick(r: Word) -> Word { 2 } \
        loop main(r: Word) -> Word { yield pick(r) }";
    let reference = reference_accepts(src);
    let module = self_host_compile(src);
    assert_eq!(
        chunk_ops(&module, "pick"),
        chunk_ops(&reference, "pick"),
        "multiheaded `fn` dispatch diverged from the reference"
    );
}

/// Three heads, two of them guarded, so the fall-through arm is reached only after
/// two failed guards. Two heads alone cannot distinguish "emits a dispatch" from
/// "emits the first guard and then the last body".
#[test]
fn a_three_headed_fn_compiles_byte_identically() {
    let src = "fn pick(r: Word) -> Word when r == 0 { 1 } \
        fn pick(r: Word) -> Word when r > 5 { 2 } \
        fn pick(r: Word) -> Word { 3 } \
        loop main(r: Word) -> Word { yield pick(r) }";
    let reference = reference_accepts(src);
    let module = self_host_compile(src);
    assert_eq!(
        chunk_ops(&module, "pick"),
        chunk_ops(&reference, "pick"),
        "three-headed `fn` dispatch diverged from the reference"
    );
}

/// MUST-NOT-FIRE CONTROL. A single-headed `fn` has one head and no guard, so it must
/// keep taking the single-body path. Without this, a fix that routed everything
/// through the multihead branch would pass the two cases above while breaking every
/// ordinary function in the corpus.
#[test]
fn a_single_headed_fn_is_unaffected() {
    let src = "fn pick(r: Word) -> Word { r + 1 } \
        loop main(r: Word) -> Word { yield pick(r) }";
    let reference = reference_accepts(src);
    let module = self_host_compile(src);
    assert_eq!(
        chunk_ops(&module, "pick"),
        chunk_ops(&reference, "pick"),
        "single-headed `fn` diverged from the reference"
    );
}

/// MUST-NOT-FIRE CONTROL. The `yield` multihead is the path that already worked, and
/// the fix must not disturb it. This is also the probe's evidence that the comparison
/// can report identity on a dispatch chain at all, so a green result above is not
/// merely the comparison being blind to that shape.
#[test]
fn a_multiheaded_yield_is_unaffected() {
    let src = "yield g(r: Word) -> Word when r == 0 { yield r } \
        yield g(r: Word) -> Word when r > 5 { yield r } \
        yield g(r: Word) -> Word { yield 0 } \
        loop main(r: Word) -> Word { g(r) }";
    let reference = reference_accepts(src);
    let module = self_host_compile(src);
    assert_eq!(
        chunk_ops(&module, "g"),
        chunk_ops(&reference, "g"),
        "multiheaded `yield` dispatch diverged from the reference"
    );
}

/// A SINGLE GUARDED HEAD is the case a bare head-count predicate gets wrong. The
/// guard can fail, so the reference still emits a dispatch with the no-matching-head
/// trap; routing it to the single-body path would drop the guard exactly as the
/// multiheaded `fn` case did. Probed rather than assumed, because the predicate this
/// suite settles on depends on the answer.
#[test]
fn a_single_guarded_fn_head_compiles_byte_identically() {
    let src = "fn pick(r: Word) -> Word when r == 0 { 1 } \
        loop main(r: Word) -> Word { yield pick(r) }";
    let reference = reference_accepts(src);
    let module = self_host_compile(src);
    assert_eq!(
        chunk_ops(&module, "pick"),
        chunk_ops(&reference, "pick"),
        "single guarded `fn` head diverged from the reference"
    );
}

/// The same question on the `yield` side is INADMISSIBLE, and that is a language fact
/// rather than a self-host gap. A lone guarded head is not always-yielding, because
/// the guard can fail into the no-matching-head trap, so a `loop` delegating its
/// productivity obligation to it is rejected by structural verification.
///
/// Recorded as an assertion because it constrains the fix: the driver never has to
/// route a single guarded `yield` head anywhere, since no such program compiles.
#[test]
fn a_single_guarded_yield_head_is_rejected_by_the_reference() {
    let src = "yield g(r: Word) -> Word when r == 0 { yield r } \
        loop main(r: Word) -> Word { g(r) }";
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let err = compile(&program).expect_err("a lone guarded yield head is not always-yielding");
    assert!(
        err.message.contains("at least one Yield"),
        "rejected for the wrong reason: {}",
        err.message
    );
}

/// A single-headed `yield` took the multihead branch, which appended a parameter copy
/// and a `Trap(NoMatchingHead)` the reference never emits. The corpus could not reach
/// it — every `yield` in the ten stage sources is multiheaded — so no whole-stage
/// self-compile would ever have reported it.
#[test]
fn a_single_headed_yield_is_unaffected() {
    let src = "yield g(r: Word) -> Word { yield r } \
        loop main(r: Word) -> Word { g(r) }";
    let reference = reference_accepts(src);
    let module = self_host_compile(src);
    assert_eq!(
        chunk_ops(&module, "g"),
        chunk_ops(&reference, "g"),
        "single-headed `yield` diverged from the reference"
    );
}
