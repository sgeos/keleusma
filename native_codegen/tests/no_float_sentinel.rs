//! **THE NO-FLOATS SENTINEL, WHICH THIS PACKAGE HAD NEVER EXERCISED.**
//!
//! `float_bits_log2 == 0` is the sentinel for *a target with no floating point*,
//! used by `Target::embedded_8` and `embedded_16` alongside `has_floats: false`.
//! **It is not a one-bit format.** The `v0.2.3` line found that while preparing
//! unrelated work, after a planned refusal of "any width below 32 bits" would
//! have rejected every module built for those targets.
//!
//! Checking this package against it found that **no test here built a module for
//! a no-floats target at all**, so the whole path was uncovered. These tests are
//! that coverage, and they pin what was measured rather than what was expected.
//!
//! # ⚠ THE PREDICTION FAILED ON ITS OWN INTERESTING FALSIFIER
//!
//! Predicted: *"`float_type` is never called with a zero width."* **False.**
//! `lower_module` computes the entry ABI's float type unconditionally, so it is
//! called with zero for every no-floats module. The result is provably unused —
//! it is consumed only where a signature shape is a float, and such a module has
//! none — but the prediction as written is refuted and is recorded as refuted.
//!
//! **The falsifier that fired is the one the brief called the interesting one**,
//! and it fired because it demanded an instrument rather than a reading of the
//! call sites. The reading had said "unreachable".
//!
//! # What was decided, and it is to change nothing in the whitelist
//!
//! `float_type`'s default arm widens any unrecognised width to `f64`. That looks
//! like the silent-wrong-number hazard the same file warns against two functions
//! above it. **Measured, it is not reachable through any operation**: an
//! unlowerable non-zero width is refused before a float type is used, and no IR
//! is emitted at all — now at two stages, since absorption 45 moved the friendly
//! path's refusal into the front end. Threading an `Option` through six call sites and into
//! closures, to remove a hazard nothing can reach, would risk a real defect in a
//! backend measured correct. See `docs/decisions/NO_FLOAT_SENTINEL.md`.

mod common;

use inkwell::context::Context;
use keleusma::target::Target;
use keleusma::{compiler::compile_with_target, lexer::tokenize, parser::parse};
use keleusma_native::{LowerError, LowerOptions, lower_module};

const NO_FLOAT_SRC: &str = "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w * d\n}\n";
const FLOAT_SRC: &str =
    "fn main(w: Word) -> Word {\n  let a = w as Float;\n  let s = a + 1.5;\n  s as Word\n}\n";

fn lower_for(src: &str, t: &Target) -> Result<Result<String, LowerError>, String> {
    let ast = parse(&tokenize(src).expect("lex")).expect("parse");
    let m = compile_with_target(&ast, t).map_err(|e| e.message)?;
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    Ok(match lower_module(&ctx, &lm, &m, LowerOptions::default()) {
        Ok(_) => {
            lm.verify().expect("LLVM module verification");
            Ok(lm.print_to_string().to_string())
        }
        Err(e) => Err(e),
    })
}

fn no_float_64bit() -> Target {
    // A 64-bit machine with no floating-point unit. An ordinary configuration,
    // and the only one that reaches the float question: the two embedded targets
    // are refused for WORD width long before a float type could be built.
    let mut t = Target::host();
    t.has_floats = false;
    t.float_bits_log2 = 0;
    t
}

/// The sentinel reaches the backend and is lowered, rather than being mistaken
/// for a one-bit float format.
#[test]
fn a_module_for_a_target_with_no_floats_lowers() {
    let t = no_float_64bit();
    assert_eq!(
        1u32 << t.float_bits_log2 >> 3,
        0,
        "the sentinel must give a zero byte width, or this test is probing something else"
    );
    let ir = lower_for(NO_FLOAT_SRC, &t)
        .expect("a float-free program must compile for a float-free target")
        .expect("and it must lower");
    assert!(
        ir.contains("kel_chunk_0"),
        "the module lowered to nothing, so the claim above is vacuous:\n{ir}"
    );
}

/// **WHY the zero width is harmless, measured rather than asserted.** No float
/// operation can exist in such a module, because the front end refuses one.
#[test]
fn a_float_operation_cannot_exist_under_a_no_floats_target() {
    let err = lower_for(FLOAT_SRC, &no_float_64bit())
        .expect_err("a float program must not compile for a float-free target");
    assert!(
        err.contains("floating-point"),
        "the refusal is not about floating point, so it may be incidental: {err}"
    );
}

/// **A DISTINCT FINDING, kept separate on purpose.** The two shipped embedded
/// targets never reach the float question at all: they are refused for word
/// width. Folding this into the float result would have credited the float
/// handling with a refusal it did not make.
#[test]
fn the_embedded_targets_are_refused_for_word_width_not_float_width() {
    for (label, t) in [
        ("embedded_16", Target::embedded_16()),
        ("embedded_8", Target::embedded_8()),
    ] {
        let outcome = lower_for(NO_FLOAT_SRC, &t).expect("compiles for the target");
        match outcome {
            Err(LowerError::UnsupportedWordWidth(_)) => {}
            other => {
                panic!("{label} was expected to be refused for WORD width; instead: {other:?}")
            }
        }
    }
}

/// **THE HAZARD THAT WOULD MATTER, AND THE REFUSAL MOVED UPSTREAM OF IT.**
///
/// A width this backend does not lower, non-zero and with floats enabled. If a
/// module lowered at such a width, `float_type`'s default arm would widen it to
/// `f64` and the program would compute in the wrong precision — a plausible
/// wrong number rather than a fault.
///
/// # ⚠ THIS TEST MOVED IN ABSORPTION 45, AND THE MOVE WAS PREDICTED
///
/// It used to construct a two-byte float target, watch the LOWERING refuse, and
/// assert on that message. **The `v0.2.3` line now refuses such a target at
/// COMPILE**, so the module never reaches the backend and the old assertion
/// failed. That was named as this absorption's falsifier before it was measured,
/// and it is **the refusal correctly moving upstream** rather than a defect on
/// either side.
///
/// # The consequence, which is the part worth keeping
///
/// **The backend's own float-width guard is no longer reachable through any
/// `Target`.** Every sub-32-bit width is now rejected by the front end.
///
/// **It is not dead code.** This backend consumes BYTECODE, and bytecode need
/// not come from this front end — which is the whole reason a native code
/// generator has its own guards. So the guard is still tested, by the route that
/// models the actual threat: a module compiled at a lowerable width whose
/// declared width is then altered, exactly as a module arriving from elsewhere
/// might present.
#[test]
fn an_unlowerable_float_width_is_refused_at_compile_and_again_by_the_backend() {
    // 1. THE FRIENDLY PATH. A target claiming floats at a width that is not a
    //    format no longer compiles at all.
    let mut t = Target::host();
    t.has_floats = true;
    t.float_bits_log2 = 4; // two bytes
    let err = lower_for(FLOAT_SRC, &t)
        .expect_err("a target claiming floats at a non-format width must not compile");
    assert!(
        err.contains("not a floating-point format"),
        "the compile refusal is not about the float width, so it may be \
         incidental: {err}"
    );

    // 2. THE THREAT MODEL. Bytecode need not come from this front end. Compile at
    //    a width the backend lowers, then alter the declared width the way a
    //    module from elsewhere could present, and require the backend to refuse
    //    on its own account.
    let ast = parse(&tokenize(FLOAT_SRC).expect("lex")).expect("parse");
    let mut m = compile_with_target(&ast, &Target::host()).expect("compiles at the host width");
    m.float_bits_log2 = 4;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let e = lower_module(&ctx, &lm, &m, LowerOptions::default())
        .expect_err("the backend must refuse a declared width it does not lower");
    let msg = format!("{e:?}");
    assert!(
        msg.contains("2 bytes"),
        "the backend refusal does not name the offending width: {msg}"
    );
    assert!(
        msg.contains("4 and 8"),
        "the backend refusal names the wrong lowered set: {msg}"
    );
    let ir = lm.print_to_string().to_string();
    assert!(
        !ir.contains("double") && !ir.contains("half"),
        "the backend emitted float code before refusing:\n{ir}"
    );
}

/// **THE CLASS GUARD, because five instances of one mistake is a class.**
///
/// The `f32` increment taught the backend to lower a four-byte float and left
/// every refusal message saying that only an eight-byte one is lowered. Five
/// separate messages — the shared-slot ABI, the entry ABI, `GetIndex`,
/// `GetField`, and a doc comment — told a reader that `f32` is unsupported while
/// the code beside them supported it.
///
/// **A refusal that names the wrong supported set is worse than a bare
/// refusal.** It does not merely fail to help; it tells someone to stop using
/// something that works.
///
/// Pinning the individual strings would be brittle and would miss the sixth. This
/// pins the CLASS: no source line may claim eight is the only lowered float
/// width, for as long as `float_width_lowered` admits four.
#[test]
fn no_refusal_claims_eight_is_the_only_lowered_float_width() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .expect("the backend source");

    // Guard the guard: if four ever stops being lowered, this test must stop
    // demanding that the messages say so, and it should fail loudly rather than
    // quietly enforcing a stale rule of its own.
    assert!(
        src.contains("matches!(float_bytes, 4 | 8)"),
        "the lowered float set is no longer `4 | 8`. This test enforces that \
         refusal messages name four AND eight; re-derive it against the new set \
         rather than deleting it."
    );

    // **NON-VACUITY.** The assertion below is that a phrase is ABSENT, which
    // holds for an empty file, a wrong path, or a reader that silently returns
    // nothing. Require the CORRECTED phrasing to be present, so the test is
    // reading the file it thinks it is and the absence means something.
    let corrected = src.matches("4- and 8-byte Floats").count();
    assert!(
        corrected >= 4,
        "found only {corrected} corrected width phrases in the backend source;          the reader is probably not seeing the file it means to check, which          would make the absence below worthless"
    );

    let offenders: Vec<(usize, String)> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            l.contains("only an 8-byte Float")
                || l.contains("only 8 is lowered")
                || l.contains("Only 8 is lowered")
        })
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect();

    assert!(
        offenders.is_empty(),
        "{} line(s) claim eight is the only lowered float width while four is \
         lowered too:\n  {}",
        offenders.len(),
        offenders
            .iter()
            .map(|(n, l)| format!("{n}: {l}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// **THE PREDICATE IS A CLAIM ABOUT A SET, SO THE SET IS ENUMERATED.**
///
/// `float_width_lowered` is an allowlist — `matches!(float_bytes, 4 | 8)` — so
/// it defaults to refusing, which is this codebase's stated posture. The tests
/// above sample it at two points: the no-floats sentinel, and one unlowerable
/// width.
///
/// **Sampling a predicate is reasoning about the members you had in mind.** The
/// `v0.2.3` line wrote the mirror of this guard on their side and it failed on
/// its first run, not for the reason it was written: their predicate was a
/// DENYLIST, `!matches!(bits_log2, 3 | 4)`, correct for the two rungs under
/// discussion and wrong about everything else — it claimed two-bit and four-bit
/// floats were implemented. **A denylist for a safety question defaults to
/// admitting the unknown.**
///
/// So this walks every encodable `float_bits_log2` and checks the outcome
/// against the predicate, rather than trusting that an allowlist cannot be
/// wrong about its own domain.
/// # ⚠ THREE DISTINCT `float_bits_log2` VALUES COLLAPSE TO ONE WIDTH HERE
///
/// Measured by this sweep: `float_bytes` is `1 << log2 >> 3`, so **log2 0, 1 and
/// 2 all give ZERO bytes**. This backend therefore cannot distinguish the
/// no-floats sentinel from a declared two-bit or four-bit float.
///
/// **Not a defect here, because all three refuse**, and a module with no floats
/// carries no float operation to refuse. It is recorded because the `v0.2.3`
/// line's predicate operates on `bits_log2` directly and *can* tell them apart,
/// so the two sides do not agree on how many distinct inputs exist — and a
/// conflation that is harmless under one refusal policy is not automatically
/// harmless under another.
#[test]
fn every_encodable_float_width_is_lowered_or_refused_as_the_allowlist_says() {
    const FLOAT_SRC: &str =
        "fn main(w: Word) -> Word {\n  let a = w as Float;\n  let s = a + 1.5;\n  s as Word\n}\n";

    let mut lowered: Vec<u8> = Vec::new();
    let mut refused: Vec<(u8, &'static str)> = Vec::new();

    for log2 in 0u8..=7 {
        let mut t = Target::host();
        t.has_floats = true;
        t.float_bits_log2 = log2;
        let bytes = 1u32 << log2 >> 3;

        match lower_for(FLOAT_SRC, &t) {
            // Refused before lowering: the front end or the target check. A
            // distinct outcome, kept distinct — folding it into the lowering's
            // refusal would credit this backend with a rejection it did not make.
            Err(_) => refused.push((log2, "compile")),
            Ok(Err(_)) => refused.push((log2, "lowering")),
            Ok(Ok(_)) => lowered.push(log2),
        }
        println!(
            "  float_bits_log2={log2} ({bytes} bytes) -> {}",
            match lower_for(FLOAT_SRC, &t) {
                Err(_) => "refused at compile".to_string(),
                Ok(Err(e)) => format!("refused at lowering: {e:?}"),
                Ok(Ok(_)) => "LOWERED".to_string(),
            }
        );
    }

    // **THE SUM IS CHECKED AGAINST THE DOMAIN, at the `v0.2.3` line's addition to
    // the counted-bucket rule.** An accumulator that increments but is never
    // compared against the domain size is a bucket nobody empties: it counts
    // correctly and proves nothing.
    //
    // Structurally guaranteed today, because the match over the nested `Result`
    // is exhaustive and every arm pushes. **Asserted anyway**, because the
    // guarantee is a property of the current control flow rather than of the
    // test, and a fourth arm or a `continue` would break it silently. This is
    // the line expected to rot first, since it is the only one that must change
    // when the domain does.
    assert_eq!(
        lowered.len() + refused.len(),
        8,
        "the sweep classified {} widths out of a domain of 8. A width matching \
         no bucket passes unseen, which is how an enumeration stops enumerating.",
        lowered.len() + refused.len()
    );
    assert!(
        !lowered.is_empty(),
        "no width lowered at all, so this sweep is measuring a broken harness \
         rather than the predicate"
    );

    // Every width that lowered must be one the allowlist admits. A width that
    // lowers while the predicate refuses it would mean some path bypasses the
    // guard entirely.
    for log2 in &lowered {
        let bytes = 1u32 << log2 >> 3;
        assert!(
            matches!(bytes, 4 | 8),
            "float_bits_log2={log2} ({bytes} bytes) LOWERED, but the allowlist \
             admits only 4 and 8. Some path reaches the float lowering without \
             consulting it, which is how a wrong-width float becomes a plausible \
             wrong number rather than a fault."
        );
    }

    // And nothing the allowlist admits may be refused BY THE LOWERING — a
    // refusal there would mean the predicate claims support the backend lacks,
    // which is the direction that admits wrong numbers on the other line.
    for (log2, stage) in &refused {
        let bytes = 1u32 << log2 >> 3;
        if matches!(bytes, 4 | 8) {
            assert_ne!(
                *stage, "lowering",
                "float_bits_log2={log2} ({bytes} bytes) is admitted by the \
                 allowlist and refused by the lowering. The predicate claims a \
                 width the backend does not implement."
            );
        }
    }
}
