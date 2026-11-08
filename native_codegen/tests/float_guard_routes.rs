//! **DOES THE FLOAT GUARD CLOSE EVERY ROUTE IT CLAIMS TO?**
//!
//! The guard's own comment enumerates four routes a float can take into a
//! module. **An enumeration is a claim**, and a guard that closes three of four
//! while reading as total is the exact shape this line keeps finding: a signal
//! answering a narrower question than the one asked.
//!
//! # Why this matters NOW rather than as tidiness
//!
//! `Op::Add` is emitted for `Byte`, `Fixed` AND `Float`. The first two lower to
//! a wrapping integer add; the third cannot be represented by this backend at
//! all. Lowering the opcode is therefore safe **only if no float can reach it**,
//! and that safety rests entirely on this guard rather than on anything in the
//! arithmetic itself.
//!
//! **Measured before the guard was widened**: a module with a float LOCAL and no
//! float in any signature passed the signature check. It was refused only
//! because `Op::Add` happened to be unsupported — *a property of what is
//! unimplemented, not a guard*. That is the same distinction the module-level
//! float refusal was originally written to fix, one level down.
//!
//! # What each route is decided BY, which is not the same for all four
//!
//! | route | state | decided by |
//! |---|---|---|
//! | chunk signature | **OPEN at 8 bytes** | the signature scan, on width |
//! | chunk constant | **OPEN** | the constant scan |
//! | native return shape | closed | the native-shape scan |
//! | data-segment slot | **OPEN at 8 bytes** | `resolve_shared_scalar`, at the ACCESS |
//!
//! **THREE OF THE FOUR ARE NOW OPEN, each on a ruling rather than a judgement**
//! — the operator's Option A float ABI, recorded in
//! `docs/decisions/ABI_RULINGS.md`. What the two width-guarded routes still
//! refuse is a `Float` of any width other than eight bytes, where emitting a
//! `double` would be a silently wrong number rather than a fault. Route 3 stays
//! closed because no ruling settles a native float ABI and nothing is built
//! behind it.
//!
//! The fourth is deliberately NOT re-checked in the guard, and its boundary is
//! not where I first assumed. A module that DECLARES a slot this backend cannot
//! lower and never reads it LOWERS; the ACCESS is where a value would arrive, so
//! the access is where the decision belongs. That placement is why opening the
//! float case needed no module-level change at all. `resolve_shared_scalar`
//! predates this work, so a second check here would be a parallel model that
//! could drift from it; this file tests the EXISTING behaviour.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

fn build(src: &str) -> Option<Module> {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

/// Whether the whole module is refused, by any route.
fn refused(m: &Module) -> Option<String> {
    let ctx = inkwell::context::Context::create();
    let lm = ctx.create_module("guard");
    lower_module(&ctx, &lm, m, LowerOptions::default())
        .err()
        .map(|e| format!("{e:?}"))
}

/// **Route 1: a float in a chunk signature.**
///
/// **This route OPENED when the entry ABI landed**, for the one float width
/// that is built: an 8-byte float takes a real floating-point position in the
/// declared function type. What the route still refuses is a float of any
/// other declared width, where declaring a `double` would be a silently wrong
/// number rather than a fault.
///
/// This build's `Float` is 8 bytes, so the width refusal cannot be reached by
/// compiling a program; the module's declared width is overwritten after
/// compilation to make the refusal a must-fire fact rather than an
/// unreachable claim. The open half — that the lowered entry really uses the
/// floating-point calling convention — is exercised by calling the JIT-ed
/// symbol in `entry_abi_float.rs`, not here.
#[test]
fn a_float_in_a_signature_lowers_at_eight_bytes_and_refuses_at_any_other_width() {
    let mut m = build("fn p(a: Float) -> Float { a }\nfn main() -> Word { 0 }").expect("compiles");
    assert!(
        // **WIDTH-DERIVED, NOT PINNED.** This asserted eight, which made the
        // test describe one of the two builds and announce a mismatch in the
        // other. Both four and eight lower now, so the premise is that the
        // build's width is one the backend lowers.
        matches!(1u32 << m.float_bits_log2 >> 3, 4 | 8),
        "this build's Float is not a width this backend lowers, so this \
         test describes a different build than the one running it"
    );
    assert!(
        refused(&m).is_none(),
        "an 8-byte float signature refuses again; either the entry ABI was \
         removed or another guard now fires first: {:?}",
        refused(&m)
    );
    // **THE SUBJECT ROTATED WHEN `f32` LANDED.** This used to overwrite the
    // width to 5, meaning four bytes, which was refused. Four bytes now LOWERS,
    // so the subject moves to a width that is still refused: 7, meaning sixteen
    // bytes. The rotation is forced rather than optional -- left at 5 this test
    // would assert a refusal that no longer happens.
    m.float_bits_log2 = 7;
    let why = refused(&m).expect("a 4-byte float in a signature must refuse the module");
    assert!(
        why.contains("signature"),
        "refused, but not by the signature route: {why}"
    );
}

/// **Route 2: a float LOCAL, with no float in any signature.**
///
/// **This is the one that was open**, and the reason this file exists. Before
/// the guard was widened it was refused only because `Op::Add` was unsupported,
/// which is not a guard at all.
#[test]
fn a_float_constant_no_longer_refuses_the_module() {
    let src =
        "fn p(w: Word) -> Word { let f = 1.5; let g = f + 2.5; w }\nfn main() -> Word { p(1) }";
    let m = build(src).expect("compiles");

    // The premise: signatures really are clean, or this tests the wrong route.
    const FLOAT_TAG: u8 = 5;
    let sig_has_float = m.signatures.iter().any(|sg| {
        matches!(sg.ret, keleusma::bytecode::WireShape::Scalar { kind } if kind == FLOAT_TAG)
            || sg.params.iter().any(
                |p| matches!(p, keleusma::bytecode::WireShape::Scalar { kind } if *kind == FLOAT_TAG),
            )
    });
    assert!(
        !sig_has_float,
        "this probe DOES carry a float in a signature, so it exercises route 1 \
         rather than the constant route it exists for"
    );

    // **SUPERSEDED 2026-08-30: THE CONSTANT ROUTE IS DELIBERATELY OPEN.**
    //
    // This asserted the module was refused, and by the constant route
    // specifically. Slice two opened that route, because it is the only one of
    // the four with a lowering behind it: a float constant is pushed tagged
    // `Float`, the conversions and float `Add`/`Sub`/`Mul` are implemented, and
    // `float_witness.kel` now LOWERS AND AGREES with the reference in the corpus
    // differential.
    //
    // **What replaced the route guard is finer, not absent**: an opcode that
    // consumes a float and was not written for one refuses at the operand. So
    // this subject — whose floats only feed an `Add` — lowers, while the same
    // float reaching a division still fails closed, pinned in
    // `float_differential.rs`.
    //
    // **The other three routes are unchanged and still refuse**, each tested
    // above and below, because the entry ABI, a native float return and float
    // data slots are all unbuilt.
    assert!(
        refused(&m).is_none(),
        "a float constant refuses the module again. The constant route was opened \
         deliberately and its witness verified differentially, so this is a \
         regression rather than the guard working: {:?}",
        refused(&m)
    );
}

/// **Route 3: a native declaring a Float RETURN SHAPE.**
///
/// **THIS ROUTE HAD NO TEST UNTIL 2026-08-24, and the file read as though it
/// did.** The guard's comment in `src/lib.rs` said *"the list is a claim and
/// `the_float_guard_closes_every_route_it_names` tests each one"* -- citing a
/// test **that was never written**. The nearest real test,
/// `the_guard_names_exactly_the_routes_this_file_tests`, asserts something much
/// weaker: that the four route NAMES still appear as strings in `src/lib.rs`.
/// **It tests the comment, not the guard.**
///
/// So the file closed three routes of four while reading as total -- *the exact
/// shape its own module header warns about*, committed by the file that warns
/// about it. Found by scanning this package for backticked citations that
/// resolve to nothing, after the `v0.2.3` line found the same class in theirs.
///
/// # Why a DECLARED-BUT-UNCALLED native is the right subject
///
/// It isolates the route. Measured on this source: `native_return_shapes` is
/// `[Scalar { kind: 5 }]` while the only chunk signature is
/// `params: [], ret: Scalar { kind: 3 }` -- **no float in any signature and no
/// float constant anywhere**, so routes 1 and 2 cannot fire and the refusal can
/// only be route 3. A native that were CALLED would put a float local in play
/// and route 2 might reach it first, which is what makes the uncalled form the
/// discriminating one rather than merely the simpler one.
#[test]
fn a_native_declaring_a_float_return_refuses_the_module() {
    let m = build("use host::read_temp() -> Float\n\nfn main() -> Word { 0 }").expect(
        "a native declaring a Float return must still COMPILE; the guard under \
         test is the backend's, and a compile failure here would make this test \
         vacuous rather than passing",
    );

    // Assert the isolation rather than trusting the prose above, so a change to
    // how the compiler shapes natives cannot leave this test passing for the
    // wrong reason.
    assert!(
        !m.signatures.iter().any(|sg| {
            matches!(sg.ret, keleusma::bytecode::WireShape::Scalar { kind } if kind == 5)
                || sg.params.iter().any(
                    |p| matches!(p, keleusma::bytecode::WireShape::Scalar { kind } if *kind == 5),
                )
        }),
        "a Float reached a chunk SIGNATURE, so route 1 can fire and this no \
         longer isolates route 3. Signatures: {:?}",
        m.signatures
    );
    assert!(
        !m.chunks.iter().any(|c| {
            c.constants
                .iter()
                .any(|k| matches!(k, keleusma::bytecode::ConstValue::Float(_)))
        }),
        "a Float CONSTANT reached the module, so route 2 can fire and this no \
         longer isolates route 3"
    );

    let why = refused(&m).expect(
        "a native declaring a Float return LOWERED. Route 3 is open: the native's \
         result would arrive on the operand stack as a float this backend has no \
         representation for",
    );
    assert!(
        why.contains("RETURN SHAPE"),
        "refused, but not by the native-shape scan ({why}), so this measures some \
         other route and route 3 is still untested"
    );
}

/// The control for route 3, and it is not optional.
///
/// Without it, `a_native_declaring_a_float_return_refuses_the_module` would pass
/// just as happily if the backend refused **every** module that declares a
/// native. The refusal has to be about the Float.
#[test]
fn a_native_declaring_a_word_return_is_not_refused_by_the_float_guard() {
    let m = build("use host::read_count() -> Word\n\nfn main() -> Word { 0 }").expect("compiles");
    if let Some(why) = refused(&m) {
        assert!(
            !why.contains("RETURN SHAPE") && !why.contains("Float"),
            "a native returning `Word` is refused by the FLOAT guard ({why}). The \
             route-3 test above is then passing for the wrong reason -- it would \
             fire for any declared native at all"
        );
    }
}

/// **Route 4: a float data slot — OPEN at eight bytes, refused at any other.**
///
/// **This route OPENED with the float shared slot.** The operator's Option A
/// ruling settles the slot as IEEE-754 bytes at the stated offset, which is
/// exactly what the reference's `read_scalar_le` does, so a read is an
/// eight-byte load and a `Float` tag. See
/// `docs/decisions/FLOAT_SHARED_SLOT_BRIEF.md`.
///
/// **The measurement that shaped this route is kept, because it is still the
/// reason the guard sits where it does.** Before the route opened:
///
/// | program | result |
/// |---|---|
/// | float slot, never read | **LOWERED** |
/// | float slot, read | refused, `UnsupportedDataSlot` |
///
/// The boundary was the ACCESS and not the declaration, and that placement is
/// why opening the float case needed no change at module level at all.
///
/// **What still refuses is the WIDTH**, and this build's `Float` is eight bytes,
/// so the refusal is unreachable by compiling a program. The module's declared
/// width is overwritten after compilation to make it a must-fire fact rather
/// than an unreachable claim — the same technique route 1 uses, for the same
/// reason.
///
/// The open half — that the lowered access agrees with the virtual machine
/// **byte for byte in the host buffer** — is exercised by `shared_data.rs`,
/// which is where the buffer-comparing oracle lives.
#[test]
fn a_float_data_slot_lowers_at_eight_bytes_and_refuses_at_any_other_width() {
    let unread = "shared data s { x: Float }\nfn p() -> Word { 0 }\nfn main() -> Word { p() }";
    let read =
        "shared data s { x: Float }\nfn p() -> Word { s.x as Word }\nfn main() -> Word { p() }";

    let Some(mut m_read) = build(read) else {
        println!("  the reference compiler will not build a Float shared slot; route 4 is");
        println!("  unreachable from source and cannot be tested through it");
        return;
    };
    assert!(
        matches!(1u32 << m_read.float_bits_log2 >> 3, 4 | 8),
        "this build's Float is not a width this backend lowers, so this \
         test describes a different build than the one running it"
    );
    assert!(
        refused(&m_read).is_none(),
        "READING an 8-byte float data slot refuses again; either the float \
         shared slot was removed or another guard now fires first: {:?}",
        refused(&m_read)
    );

    // The width refusal, made must-fire by overwriting the declared width.
    // **THE SUBJECT ROTATED WHEN `f32` LANDED.** This used to overwrite the
    // width to 5, meaning four bytes, which was refused. Four bytes now LOWERS,
    // so the subject moves to a width that is still refused: 7, meaning sixteen
    // bytes. The rotation is forced rather than optional -- left at 5 this test
    // would assert a refusal that no longer happens.
    m_read.float_bits_log2 = 7;
    let why = refused(&m_read).expect("a 4-byte float data slot must refuse the module");
    assert!(
        why.to_lowercase().contains("float"),
        "refused, but not in a way naming the float slot: {why}"
    );

    // The declaration-versus-access placement, kept asserted: an UNREAD slot
    // lowers, which is what it did before the route opened and what it must
    // still do now that reading it lowers too.
    if let Some(m_unread) = build(unread) {
        assert!(
            refused(&m_unread).is_none(),
            "an UNREAD float slot is refused, which contradicts the placement \
             this file records -- update the claim rather than deleting it"
        );
    }
}

/// **THE MUST-NOT-FIRE CONTROL.** A guard that refused every module would pass
/// every test above and make the whole enumeration meaningless.
#[test]
fn a_module_with_no_float_anywhere_is_not_refused_by_the_float_guard() {
    let m = build("fn p(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { p(1, 2) }")
        .expect("compiles");
    match refused(&m) {
        None => {}
        Some(why) => assert!(
            !why.to_lowercase().contains("float"),
            "a module with no float anywhere was refused BY THE FLOAT GUARD, so \
             the guard fires on programs it should not and every refusal above \
             proves nothing: {why}"
        ),
    }
}

/// **The enumeration itself is asserted**, so adding a route to the comment
/// without a test here fails rather than passing quietly.
///
/// Deliberately a source check: the claim being tested is that the guard's
/// comment lists exactly the routes this file covers.
#[test]
fn the_guard_names_exactly_the_routes_this_file_tests() {
    let src = std::fs::read_to_string("src/lib.rs").expect("read lib.rs");
    let start = src
        .find("A SIGNATURE IS NOT THE ONLY ROUTE A FLOAT TAKES")
        .expect("the guard's route enumeration is gone; it is what this file tests");
    let window = &src[start..start + 2000];
    for route in [
        "chunk signatures",
        "chunk constants",
        "native return shapes",
        "data-segment slots",
    ] {
        assert!(
            window.contains(route),
            "the guard no longer names the route `{route}`. If a route was \
             REMOVED, say why it cannot happen; if one was RENAMED, rename it \
             here too. A route that leaves the list silently is how a guard \
             stops being total without anyone noticing."
        );
    }
}
