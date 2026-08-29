//! **A CONSERVATIVE REJECTION, NOT `verify()`, IS WHAT HOLDS A RUNTIME TRAP SHUT.**
//!
//! `Op::Len` on a flat array returns `VmError::InvalidBytecode`. `src/vm.rs`
//! justifies that class with *"array length is a fixed-size, compile-time
//! constant the compiler folds to a literal (it never emits `Op::Len` on an
//! array), so a flat body here is a mis-compilation rather than a script
//! error."*
//!
//! **The reference compiler emits exactly that**, from an ordinary program:
//! `for x in if c { a } else { b }`. `Op::Len` fires when the for-in source has
//! no statically known length, and `static_for_in_length` has no `Expr::If` arm.
//!
//! # Why this is written down rather than shrugged at
//!
//! `InvalidBytecode` is the class `verify()` exists to exclude at load time, and
//! this project has already had one instance of that hole: the `Op::IsStruct`
//! witness verified, took a bound, LOADED, and then trapped. It was repaired at
//! both root causes.
//!
//! This is the same class one guard away — but **the guard holding it shut is
//! not `verify()`, which accepts the module. It is the resource-bound check**,
//! and the project's own taxonomy places that refusal in the SECOND category:
//! provable in principle, analysis not implemented. That is the category defined
//! as liftable. So an unambiguous improvement to the bound extractor, made by
//! someone with no reason to look at `Op::Len`, would convert a rejected program
//! into one that loads and traps.
//!
//! **The finding is that an improvement is silently gated on an unrelated
//! repair.** These four tests are what makes it un-silent.
//!
//! # Scope
//!
//! `src/vm.rs` and `src/verify.rs` are owned by the `v0.2.3` line and are
//! read-only here. This file **reports**; the repair is not this line's to make.
//! See `docs/decisions/LEN_FLAT_ARRAY_HAZARD.md`.

use keleusma::bytecode::Op;
use keleusma::vm::Vm;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// The construct, found by the `v0.2.3` line by reading `static_for_in_length`'s
/// arms for what they OMIT rather than by guessing a fifteenth construct.
const IF_SOURCE: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [3, 4];
  for x in if c { a } else { b } { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";

/// The ordinary for-in. Every refusal below must NOT fire for this, or the
/// refusals say nothing about the `if` source in particular.
const PLAIN_SOURCE: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  for x in a { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";

fn build(src: &str) -> keleusma::bytecode::Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn emits_len(m: &keleusma::bytecode::Module) -> bool {
    m.chunks
        .iter()
        .any(|c| c.ops.iter().any(|o| matches!(o, Op::Len)))
}

/// **LEG 1 — the structural verifier ACCEPTS it.**
///
/// If this ever starts failing, the hazard is closed at the right place and the
/// rest of this file becomes historical.
#[test]
fn leg_1_verify_accepts_a_module_that_emits_len_on_an_array() {
    let m = build(IF_SOURCE);
    assert!(
        emits_len(&m),
        "the subject no longer emits `Op::Len`, so every assertion in this file \
         is about a program that does not exist. The construct was repaired out \
         of existence twice before for `Op::IsStruct`; this is that failure mode"
    );
    assert!(
        keleusma::verify::verify(&m).is_ok(),
        "`verify()` now REJECTS the Len witness. That is the repair this file \
         reports as missing, and this file should be retired rather than fixed: \
         {:?}",
        keleusma::verify::verify(&m).err()
    );
}

/// **LEG 2 — executing it traps, measured rather than reasoned about.**
///
/// **`new_unchecked` is the documented trust-skip for precompiled bytecode, and
/// using it here is NOT a claim that this program is admissible.** It is the only
/// way to ask what the runtime arm does, which is the question. Leg 3 is what
/// establishes admissibility, and it establishes the opposite.
#[test]
fn leg_2_executing_it_yields_invalid_bytecode() {
    let m = build(IF_SOURCE);
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = unsafe { Vm::new_unchecked(m, &arena) }.expect("new_unchecked");
    let err = vm.call(&[]).expect_err(
        "the Len witness RAN TO COMPLETION. Either the flat-array arm no longer \
         faults, or the operand is no longer a flat array. Both change what this \
         file reports",
    );
    let text = format!("{err:?}");
    assert!(
        text.contains("InvalidBytecode"),
        "the trap is no longer `InvalidBytecode`, which is the class `verify()` \
         exists to exclude at load time and the whole reason this is written \
         down: {text}"
    );
    assert!(
        text.contains("flat array"),
        "the module faults for some OTHER reason, so this test would report the \
         hazard while measuring something else: {text}"
    );
}

/// **LEG 3 — it is NOT reachable through the supported path today.**
///
/// The check runs in `Vm::new` itself, not only in `auto_arena_capacity_for`, so
/// a host that sizes its own arena cannot load it either. **That distinction was
/// measured after an earlier reading of this hazard assumed otherwise**, and it
/// is the difference between a hazard and a false alarm.
#[test]
fn leg_3_the_supported_path_refuses_it_at_every_arena_size() {
    let m = build(IF_SOURCE);
    for cap in [4096usize, 65_536, 1 << 20] {
        let arena = keleusma_arena::Arena::with_capacity(cap);
        assert!(
            Vm::new(m.clone(), &arena).is_err(),
            "`Vm::new` ADMITTED the Len witness at arena capacity {cap}. The \
             trap in leg 2 is then reachable through the supported path, which \
             makes this a live defect rather than a gated one"
        );
    }

    // CONTROL. Without this, "Vm::new refuses" could mean it refuses everything
    // of this shape, and the finding would be about for-in rather than about the
    // `if` source.
    let plain = build(PLAIN_SOURCE);
    let arena = keleusma_arena::Arena::with_capacity(65_536);
    assert!(
        Vm::new(plain, &arena).is_ok(),
        "`Vm::new` refuses the ORDINARY for-in too, so leg 3 says nothing about \
         the `if` source specifically"
    );
}

/// **LEG 4 — the refusal is SECOND category, which is why leg 3 is not comfort.**
///
/// A first-category refusal (the bound cannot exist) would hold forever. This
/// one is "provable in principle, analysis not implemented": giving both arms the
/// same length makes the trip count two on every path and provable by
/// inspection, and it is refused anyway, because neither the length guard nor
/// the bound extractor looks THROUGH an `Expr::If`.
///
/// **So the guard holding the trap shut is one someone may lift as an
/// improvement**, with no reason to look at `Op::Len` while doing it.
#[test]
fn leg_4_the_refusal_is_liftable_rather_than_structural() {
    const EQUAL_LENGTHS: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [9, 9];
  for x in if c { a } else { b } { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";
    let m = build(EQUAL_LENGTHS);
    assert!(
        emits_len(&m),
        "the equal-length form no longer emits `Op::Len`, so it cannot show that \
         the refusal survives a trip count provable by inspection"
    );
    let arena = keleusma_arena::Arena::with_capacity(65_536);
    assert!(
        Vm::new(m, &arena).is_err(),
        "the equal-length form is now ADMITTED. The bound extractor has learned \
         to see through an `Expr::If`, which is the improvement this file exists \
         to gate: leg 2's trap is now reachable through the supported path"
    );
}
