//! **`Op::Len` IS REACHABLE IN BYTECODE AND UNREACHABLE IN A BOUNDED PROGRAM.**
//!
//! The `v0.2.3` line answered the long-open question of whether a construct
//! reaching `Op::Len` EXISTS: it does, an `if` EXPRESSION as the for-in source,
//! found by reading `static_for_in_length`'s match arms for what they OMIT
//! rather than by guessing a fifteenth construct. That answer stands and this
//! file does not dispute it.
//!
//! **What this file adds is the qualification that matters for THIS project.**
//! The program that emits the opcode passes `verify()` and is then REFUSED by
//! the resource-bound analysis: `auto_arena_capacity_for` reports that the loop
//! "has no statically extractable iteration bound". So on a language whose
//! stated value proposition is definitive worst-case execution time and memory
//! use, `Op::Len` is reachable in an artefact that **cannot be admitted**.
//!
//! # The two facts are ONE fact, which is why this is structural rather than a
//! # missing case
//!
//! `Op::Len` fires exactly when the for-in source has no statically known
//! length. A loop whose trip count is not statically known is exactly what the
//! bound extractor refuses. **The property that makes the opcode reachable is
//! the property that makes the loop unbounded.** They are not two independent
//! limitations that might be lifted separately.
//!
//! # The measurement that rules out the obvious objection
//!
//! "The arms have different lengths, so of course the bound is unknown." **No.**
//! `both_arms_same_length_is_still_refused` gives both arms length two, so the
//! trip count is two on every path and the bound is provable by inspection. It
//! is refused anyway, because neither the length guard nor the bound extractor
//! looks THROUGH an `Expr::If`. That places this squarely in the project's
//! SECOND category of conservative rejection — provable in principle, analysis
//! not implemented — and not in the first.
//!
//! # This is NOT a defect report, and must not be read as one
//!
//! Refusing a program whose bound it cannot prove is the verifier working as
//! designed and as `LANGUAGE_DESIGN.md` documents. What is recorded here is the
//! CONSEQUENCE for opcode reachability, which is a rad-hard question on a
//! project treating opcode count as a first-order constraint.
//!
//! # The latent harness hazard, named so it is not discovered by a crash
//!
//! `examples/scripts/opcode_witness.kel` now carries this construct, so that
//! module can never be given an arena. The corpus differential exempts it
//! BEFORE it would try, because `module_refusals` is non-empty and the
//! backend-refusal check runs first. **That ordering is what keeps the suite
//! green, and it is contingent.** If `Add`, `FixedDiv`, `IntToFloat` and `Len`
//! all became lowerable, the file would leave the exempt set and the harness
//! would panic in `arena_for`'s `expect("arena capacity")` rather than exempt
//! cleanly. `the_witness_module_cannot_be_given_an_arena` pins that, so the day
//! it matters someone meets an explanation instead of a stack trace.
use keleusma::bytecode::Module;
use keleusma::vm::{auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// The construct, from the `v0.2.3` line. An `if` EXPRESSION as the for-in
/// source; `static_for_in_length` handles `ArrayLiteral`, `Call`, `FieldAccess`,
/// `Ident`, `ArrayIndex` and `Match`, then falls through to `_ => None`.
const IF_SOURCE: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [3, 4];
  for x in if c { a } else { b } { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";

/// The same shape with BOTH ARMS THE SAME LENGTH, so the trip count is two on
/// every path and the bound is provable by inspection.
const IF_SOURCE_EQUAL_LENGTHS: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [9, 9];
  for x in if c { a } else { b } { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";

/// The `Op::IsStruct` witness, from the `v0.2.3` line after nine failed attempts
/// of their own. The parameter pattern is UNANNOTATED, so the pattern's type is
/// unknown at the test; annotating it folds the test out.
const IS_STRUCT_SOURCE: &str = "\
struct P { a: Word, b: Word }
fn g(P { a, b }) -> Word { a + b }
fn main() -> Word { g(P { a: 1, b: 2 }) }
";

/// The ordinary form, which emits no `Len` and IS bounded. The control that
/// stops every assertion below from being about for-in in general.
const PLAIN_SOURCE: &str = "\
fn f() -> Word {
  let a = [1, 2];
  for x in a { let _d = x; }
  0
}
fn main() -> Word { f() }
";

fn build(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

fn emits_len(m: &Module) -> bool {
    m.chunks
        .iter()
        .any(|c| c.ops.iter().any(|o| format!("{o:?}").starts_with("Len")))
}

/// **THE CONTROL, and it carries two claims at once.** The plain form must emit
/// NO `Len` and must be BOUNDED. Without it, every refusal below could be a
/// property of for-in, of arrays, or of this harness, rather than of the
/// construct under test.
#[test]
fn the_ordinary_for_in_emits_no_len_and_is_bounded() {
    let m = build(PLAIN_SOURCE);
    assert!(
        !emits_len(&m),
        "the plain for-in emits Len, so it is not the contrast this file needs \
         and the `if` source is not what makes the difference"
    );
    assert!(
        auto_arena_capacity_for(&m, &[]).is_ok(),
        "the plain for-in is REFUSED a bound, so the refusals below say nothing \
         about the `if` source: {:?}",
        auto_arena_capacity_for(&m, &[]).err()
    );
}

/// The construct reaches the opcode, and the module is structurally valid.
///
/// **`verify()` accepting is half the claim and the less interesting half.**
/// Stated separately from the bound so the two cannot be confused: this module
/// is well-formed bytecode AND inadmissible under the resource analysis, and a
/// reader who saw only one of those would draw the wrong conclusion.
#[test]
fn the_if_source_reaches_op_len_and_verifies() {
    let m = build(IF_SOURCE);
    assert!(
        emits_len(&m),
        "the construct no longer emits Op::Len. If `static_for_in_length` gained \
         an `Expr::If` arm, that is NEWS: the only known witness for this opcode \
         is gone and the reachability question is reopened"
    );
    assert!(
        keleusma::verify::verify(&m).is_ok(),
        "the structural verifier rejects it, which would make this a different \
         finding from the one recorded here: {:?}",
        keleusma::verify::verify(&m).err()
    );
}

/// **TWO INDEPENDENT BOUND ENTRY POINTS REFUSE IT, and that is not redundancy.**
///
/// The finding below rests on `auto_arena_capacity_for`. If that were the only
/// evidence, "the Len witness is unbounded" and "one arena-sizing helper happens
/// to refuse it" would be indistinguishable. `module_wcmu` is a different public
/// entry into the resource analysis, and it refuses too.
///
/// **Reached independently by the `v0.2.3` line through `module_wcmu` while this
/// line used `auto_arena_capacity_for`**, before either knew which the other had
/// called. Re-run here rather than taken on report.
#[test]
fn a_second_bound_entry_point_refuses_it_too() {
    let m = build(IF_SOURCE);
    assert!(
        keleusma::verify::module_wcmu(&m, &[]).is_err(),
        "`module_wcmu` accepts the Len witness while `auto_arena_capacity_for` \
         refuses it. The two disagreeing is a finding in itself and means the \
         refusal recorded in this file is a property of ONE helper rather than \
         of the resource analysis"
    );
    // The control: the bounded form must pass BOTH, or "refuses" above is just
    // "this entry point refuses everything".
    let plain = build(PLAIN_SOURCE);
    assert!(
        keleusma::verify::module_wcmu(&plain, &[]).is_ok(),
        "`module_wcmu` refuses the ORDINARY for-in as well, so its refusal above \
         says nothing about the `if` source: {:?}",
        keleusma::verify::module_wcmu(&plain, &[]).err()
    );
}

/// **THE FINDING.** Verified bytecode, refused a bound.
#[test]
fn the_only_known_len_witness_cannot_be_given_a_resource_bound() {
    let m = build(IF_SOURCE);
    let err = auto_arena_capacity_for(&m, &[])
        .err()
        .map(|e| format!("{e:?}"));
    assert!(
        err.is_some(),
        "the Len construct WAS given a resource bound. That is NEWS rather than \
         a defect -- it would mean the bound extractor learned to see through an \
         `Expr::If`, and `Op::Len` would become reachable in an ADMISSIBLE \
         program for the first time. Update the handoff row; do not delete this."
    );
    let err = err.unwrap();
    assert!(
        err.contains("iteration bound"),
        "the bound was refused for a DIFFERENT reason than the unextractable \
         iteration count, so this test is no longer measuring what it claims: \
         {err}"
    );
}

/// **The objection ruled out.** Equal-length arms make the trip count provable
/// by inspection, and it is refused anyway.
#[test]
fn both_arms_same_length_is_still_refused() {
    let m = build(IF_SOURCE_EQUAL_LENGTHS);
    assert!(
        emits_len(&m),
        "equal-length arms folded the length out, so this case is not exercising \
         the construct and proves nothing about why the bound is refused"
    );
    assert!(
        auto_arena_capacity_for(&m, &[]).is_err(),
        "equal-length arms ARE given a bound, so the refusal is about the arms \
         DISAGREEING and not about `Expr::If` being opaque. That is a materially \
         weaker finding than the one recorded in this file, and the handoff must \
         be corrected rather than this assertion inverted"
    );
}

/// **The latent harness hazard, pinned.** See the module docs.
///
/// This asserts a PROPERTY OF THE CORPUS FILE, not of a synthetic string, so it
/// tracks the file rather than a copy of it.
#[test]
fn the_witness_module_cannot_be_given_an_arena() {
    let src = std::fs::read_to_string("../examples/scripts/opcode_witness.kel")
        .expect("read the witness module");
    let m = build(&src);
    assert!(
        emits_len(&m),
        "opcode_witness.kel no longer emits Op::Len, so the corpus lost its only \
         witness for that opcode and the ISA coverage census will drop to 64"
    );
    assert!(
        auto_arena_capacity_for(&m, &[]).is_err(),
        "opcode_witness.kel CAN now be given an arena. Good news, and it means \
         the file may become runnable -- but check `arena_for` in \
         corpus_differential before assuming the harness handles it"
    );
    // The consequence, spelled out where someone debugging a panic will find it.
    assert!(
        !keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default()).is_empty(),
        "THE BACKEND NOW LOWERS opcode_witness.kel COMPLETELY, and that removes \
         the exemption keeping the corpus differential away from it. The module \
         still CANNOT be given an arena (asserted above), so `arena_for`'s \
         `expect(\"arena capacity\")` will PANIC rather than exempt cleanly. \
         This is NEWS -- four refused opcodes were lowered -- but the harness \
         needs a bound-refusal exemption class before the file can be driven."
    );
}

/// **`Op::IsStruct` IS THE OTHER CASE, AND IT IS THE SHARPER ONE.**
///
/// Measured on this tree rather than taken from the `v0.2.3` line's report:
///
/// | witness | `verify()` | `module_wcmu` | arena | load | run |
/// |---|---|---|---|---|---|
/// | `Op::Len` | accepts | REFUSES | REFUSED | n/a | never runs |
/// | `Op::IsStruct` | accepts | accepts | OK | **LOADS** | **TRAPS** |
///
/// `Op::Len`'s witness cannot be ADMITTED — refused before it can load, which is
/// the conservative-verification stance working as designed and therefore not a
/// hole. **`Op::IsStruct`'s witness satisfies EVERY load-time check**, receives a
/// memory bound, loads, and then dies at call time with `InvalidBytecode`.
///
/// **That is not the same kind of fact.** `InvalidBytecode` is the class
/// `verify()` exists to exclude AT LOAD TIME, so a legal program reaching it at
/// RUN time is a load-time hole rather than a bad program. **`src/verify.rs` is
/// read-only to BOTH lines** — the `v0.2.3` line believed it was this line's and
/// this line's handoff records it as theirs, so NEITHER has repaired it. It is
/// recorded for the operator, with the ownership question named as its own item.
///
/// **This test fires in the FAILING direction on a repair.** If the trap stops
/// happening, that is the hole being closed and the census row wants rewriting,
/// not this assertion deleting.
#[test]
fn the_is_struct_witness_loads_and_then_traps() {
    let m = build(IS_STRUCT_SOURCE);
    assert!(
        m.chunks.iter().any(|c| c
            .ops
            .iter()
            .any(|o| format!("{o:?}").starts_with("IsStruct"))),
        "the witness no longer emits Op::IsStruct. If the compiler now folds the \
         test out for an unannotated parameter, that is a REPAIR: say so and \
         re-measure the coverage census, which counts this opcode witnessed"
    );
    assert!(
        keleusma::verify::verify(&m).is_ok(),
        "the structural verifier rejects it, which would make this a DIFFERENT \
         finding -- the hole recorded here is that it does NOT"
    );

    let cap = auto_arena_capacity_for(&m, &[]).expect(
        "the IsStruct witness was refused a bound. That makes it the same case as \
         Op::Len rather than the contrasting one, and the table in this file's \
         docs is then wrong rather than merely stale",
    );
    let need = required_persistent_capacity_for(&m);
    let mut arena = keleusma_arena::Arena::with_capacity(cap + need + (4 << 20));
    arena.resize_persistent(need).expect("persistent fits");
    let mut vm = keleusma::vm::Vm::new(m, &arena)
        .expect("the witness must LOAD -- that it does is half the finding");

    let mut shared: Vec<u8> = Vec::new();
    let err = vm
        .call_with_shared(&mut shared, &[])
        .expect_err("THE WITNESS RAN CLEANLY. The load-time hole is closed: a program that reaches Op::IsStruct no longer traps. Rewrite the row, do not delete this.");
    let text = format!("{err:?}");
    assert!(
        text.contains("InvalidBytecode"),
        "the witness failed for a DIFFERENT reason than InvalidBytecode, so this \
         test no longer measures the load-time hole it names: {text}"
    );
}
