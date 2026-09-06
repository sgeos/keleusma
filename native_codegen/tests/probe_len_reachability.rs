//! **THE REACHABILITY VERDICT MOVED OUT OF THIS FILE, AND THAT IS THE REPAIR.**
//!
//! This file used to hold five assertions about a single `Op::Len` witness
//! source, and four other files held their own copies of the same source.
//! Absorption 51 folded that construct away and **twelve tests went red at
//! once**. Every copy had to be found and disposed of separately.
//!
//! **That coupling is this line's own design fault and it has now rotted three
//! times** — `Op::Call`, `Op::IsStruct`, and `Op::Len`. The witness text now has
//! a single definition in `tests/common/mod.rs`, and the reachability VERDICT has
//! a single owner in `tests/len_producer_census.rs`, which answers it across four
//! independent legs each carrying its own must-fire control.
//!
//! # What remains here, and why each survived rather than being kept out of habit
//!
//! Three things, none of which the census answers:
//!
//! 1. **The control** that the ordinary for-in emits no `Len` and is bounded.
//!    Without it, nothing distinguishes a claim about the `if` source from a
//!    claim about for-in in general.
//! 2. **The `Op::IsStruct` load-time hole**, closed and asserted BY VALUE — a
//!    repair that changed a program's meaning would be worse than the trap it
//!    replaced.
//! 3. **The `limit`-clause blocker check**, the one of five that survived a
//!    specific attack on the merits.
//!
//! # And one thing was added, because a prediction resolved
//!
//! `the_corpus_witness_module_now_runs` records that `refused_witness.kel` left
//! the differential's exempt set. This file predicted a harness PANIC for that
//! moment. **It did not happen, and the reason is the useful part.**
//!
//! # WHAT IS NOT CLAIMED
//!
//! Nothing here says `Op::Len` is unreachable. This tree carries a retraction on
//! exactly that word. See `docs/decisions/OP_LEN_PRODUCER_CENSUS.md` for what was
//! searched, by what method, and with what limits.

mod common;

use common::{IF_SOURCE, IS_STRUCT_SOURCE, PLAIN_SOURCE, build, emits};
use keleusma::vm::{auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// **THE CONTROL, and it carries two claims at once.** The plain form must emit
/// NO `Len` and must be BOUNDED.
#[test]
fn the_ordinary_for_in_emits_no_len_and_is_bounded() {
    let m = build(PLAIN_SOURCE);
    assert!(
        !emits(&m, "Len"),
        "the plain for-in emits Len, so it is not the contrast this file needs"
    );
    assert!(
        auto_arena_capacity_for(&m, &[]).is_ok(),
        "the plain for-in is REFUSED a bound, so every comparison against it is \
         meaningless: {:?}",
        auto_arena_capacity_for(&m, &[]).err()
    );
}

/// **THE FORMER WITNESS IS NOW AN ORDINARY PROGRAM, ASSERTED THROUGH THE WHOLE
/// CHAIN.**
///
/// This replaces four separate assertions that each measured one link of it:
/// emission, `verify()`, `module_wcmu`, and `auto_arena_capacity_for`. They are
/// one fact and are now stated as one, so a future fold invalidates one test.
///
/// **The two bound entry points are still checked separately**, and that is not
/// redundancy: if they disagreed, a claim about "the resource analysis" would
/// really be a claim about one helper. They were reached independently by the
/// two lines before either knew which the other had called.
#[test]
fn the_former_witness_is_now_an_ordinary_bounded_program() {
    let m = build(IF_SOURCE);
    assert!(
        !emits(&m, "Len"),
        "the `if` source EMITS `Op::Len` again. The fold regressed; re-read the \
         pre-2026-09-05 analysis rather than writing a new one."
    );
    assert!(
        keleusma::verify::verify(&m).is_ok(),
        "the structural verifier rejects a program that used to pass it: {:?}",
        keleusma::verify::verify(&m).err()
    );
    assert!(
        keleusma::verify::module_wcmu(&m, &[]).is_ok(),
        "`module_wcmu` refuses the folded form: {:?}",
        keleusma::verify::module_wcmu(&m, &[]).err()
    );
    assert!(
        auto_arena_capacity_for(&m, &[]).is_ok(),
        "`auto_arena_capacity_for` refuses the folded form while `module_wcmu` \
         accepts it. THE TWO DISAGREEING IS A FINDING IN ITSELF and means any \
         claim here about `the resource analysis` is really about one helper: \
         {:?}",
        auto_arena_capacity_for(&m, &[]).err()
    );
}

/// **A PREDICTION RESOLVED, AND ITS PREMISE IS WHAT FAILED.**
///
/// This file predicted that if every refused opcode in `refused_witness.kel`
/// became lowerable, the corpus differential would PANIC in `arena_for`'s
/// `expect("arena capacity")` rather than exempt cleanly, because the module
/// still could not be given an arena.
///
/// **Both halves moved at once and no panic occurred.** `module_refusals` now
/// returns empty, the module takes an arena, and it runs.
///
/// **The prediction was sound and its premise was not.** It assumed the two
/// properties could move apart. They could not: the property that emitted
/// `Op::Len` was the property that denied the bound, which is the structural
/// claim this file has argued from the start. The argument held; the contingency
/// planned around it never arose.
#[test]
fn the_corpus_witness_module_now_runs() {
    let src = std::fs::read_to_string("../examples/scripts/refused_witness.kel")
        .expect("read the former refusing-witness module");
    let m = build(&src);
    assert!(
        !emits(&m, "Len"),
        "`refused_witness.kel` emits `Op::Len` again, so the corpus has a witness \
         once more: restore the `WITNESSES:` claim in that file and re-measure \
         the coverage census."
    );
    let refusals = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the backend refuses `refused_witness.kel` again: {refusals:?}. If that \
         refusal is `Op::Len` the producer is back; if it is anything else the \
         module has left the exempt set for a new reason. Name which before \
         editing `remaining_refusals.rs`."
    );
    let cap = auto_arena_capacity_for(&m, &[]).expect(
        "the module lowers completely but CANNOT be given an arena. That is \
         exactly the state this file predicted would panic the corpus \
         differential, and it now needs the bound-refusal exemption class that \
         was never required.",
    );
    let need = required_persistent_capacity_for(&m);
    let mut arena = keleusma_arena::Arena::with_capacity(cap + need + (1 << 20));
    arena.resize_persistent(need).expect("persistent fits");
    let mut vm = keleusma::vm::Vm::new(m, &arena).expect("the module must LOAD");
    let mut shared: Vec<u8> = Vec::new();
    let got = vm
        .call_with_shared(&mut shared, &[keleusma::bytecode::Value::Int(1)])
        .expect("the module must RUN");
    assert_eq!(
        format!("{got:?}"),
        "Finished(Int(4))",
        "the module runs but no longer means what it meant. Its three parts \
         contribute 1, 3 and 0; a changed total says which fold changed a value."
    );
}

/// **SUPERSEDED VERDICT — THE `Op::IsStruct` LOAD-TIME HOLE IS CLOSED. The
/// assertion is kept and INVERTED, never deleted.**
///
/// `Op::IsStruct`'s witness satisfied every load-time check, received a memory
/// bound, loaded, and then died at call time with `InvalidBytecode` — the class
/// `verify()` exists to exclude AT LOAD TIME. `6d217f0a` closed it in the
/// compiler at both root causes; **`src/verify.rs` was never touched.**
///
/// That retired an item this line had escalated as blocked on an ownership
/// question about `src/verify.rs`. **The premise was wrong**: the defect was
/// upstream, and closing it there removed the emission rather than teaching the
/// verifier to reject it. **Test the premise of an escalation before escalating
/// it** — this line has now hit that twice.
///
/// The value is asserted, not merely the absence of a trap: a repair that
/// changed the program's meaning would be worse than the trap it replaced.
#[test]
fn the_is_struct_witness_runs_and_the_load_time_hole_is_closed() {
    let m = build(IS_STRUCT_SOURCE);
    assert!(
        !emits(&m, "IsStruct"),
        "`Op::IsStruct` is emitted again for an un-annotated parameter, so the \
         fold regressed. Re-measure the whole chain before rewriting anything"
    );
    assert!(
        keleusma::verify::verify(&m).is_ok(),
        "the structural verifier now rejects a program that RUNS, which is a \
         different and worse finding than the one this test used to record"
    );

    let cap = auto_arena_capacity_for(&m, &[])
        .expect("the witness must still be given a bound; it was before the fold");
    let need = required_persistent_capacity_for(&m);
    let mut arena = keleusma_arena::Arena::with_capacity(cap + need + (4 << 20));
    arena.resize_persistent(need).expect("persistent fits");
    let mut vm = keleusma::vm::Vm::new(m, &arena).expect("the witness must LOAD");

    let mut shared: Vec<u8> = Vec::new();
    let got = vm.call_with_shared(&mut shared, &[]).expect(
        "THE WITNESS TRAPS AGAIN. The load-time hole is open: a legal program \
         reaches `InvalidBytecode` at run time. Rewrite the verdict, do not \
         delete this, and report it to the line that owns the compiler.",
    );
    assert_eq!(
        format!("{got:?}"),
        "Finished(Int(3))",
        "the witness runs but no longer means what it meant. A repair that \
         changes a program's VALUE is worse than the trap it replaced -- that is \
         why this is asserted by value and not by absence of a fault"
    );
}

/// **THE FIFTH BLOCKER CHECK, KEPT BECAUSE IT SURVIVED A SPECIFIC ATTACK.**
///
/// The structural claim was that `Op::Len` fires exactly when the for-in source
/// has no statically known length, and that such a loop is exactly what the bound
/// extractor refuses — *"not two independent limitations that might be lifted
/// separately."* `for .. limit <const>` is precisely a mechanism for lifting them
/// separately, and this argument predated that form.
///
/// # ANSWER, MEASURED 2026-08-27 AND RE-MEASURED 2026-09-05: THE BLOCKER HOLDS
///
/// The program is refused at COMPILATION, as a type error: *"a `limit` clause
/// requires a range `for` loop."* **The `limit` form reaches RANGES only; the
/// opcode fired on SOURCES.** Four of five recorded blockers expired in one
/// session; this is the one that survived on the merits.
///
/// **It is retained now that the opcode has no producer** because it is the
/// argument, not the witness, that carried the verdict — and the argument is what
/// the census leans on when it says the two properties moved together.
#[test]
fn does_a_limit_clause_admit_a_source_with_no_static_length() {
    const WITH_LIMIT: &str = "\
fn f(c: bool) -> Word {
  let a = [1, 2];
  let b = [3, 4];
  for x in if c { a } else { b } limit 2 { let _d = x; }
  0
}
fn main() -> Word { f(true) }
";
    println!("\n================ DOES `limit` ADMIT A NO-STATIC-LENGTH SOURCE?");

    // **NAME THE STAGE.** "Refused" without saying which stage refused is not a
    // result: parse, typecheck, verify and the bound extractor are different
    // answers with different implications.
    let toks = match tokenize(WITH_LIMIT) {
        Ok(t) => t,
        Err(e) => {
            println!("  REFUSED AT: lexing -- {e:?}");
            println!("================\n");
            return;
        }
    };
    let ast = match parse(&toks) {
        Ok(a) => a,
        Err(e) => {
            println!("  REFUSED AT: parsing -- {e:?}");
            println!("  VERDICT: the grammar does not accept `limit` on a non-range source,");
            println!("  so the structural argument HOLDS.");
            println!("================\n");
            return;
        }
    };
    let m = match compile(&ast) {
        Ok(m) => m,
        Err(e) => {
            println!("  REFUSED AT: compilation, as a TYPE ERROR -- {e:?}");
            println!("  VERDICT: **THE BLOCKER HOLDS.** The `limit` clause requires a RANGE");
            println!("  `for` loop, and the opcode fired on a for-in over a SOURCE. The one");
            println!("  mechanism that lifts the static-bound requirement does not reach the");
            println!("  case where the opcode was emitted, so the two limitations really were");
            println!("  not liftable separately -- as this argument held before the form even");
            println!("  existed, and as the eventual repair confirmed by closing BOTH at once.");
            println!("================\n");
            return;
        }
    };

    // **NON-VACUITY.** Reading an admission verdict from a program that never
    // reaches the opcode settles nothing.
    let emitted = emits(&m, "Len");
    println!("  compiles: yes.  emits Op::Len: {emitted}");
    if !emitted {
        println!("  VERDICT: INCONCLUSIVE -- the limit form compiles but the opcode is not");
        println!("  emitted, so this program cannot answer the question.");
        println!("================\n");
        return;
    }
    match auto_arena_capacity_for(&m, &[]) {
        Ok(cap) => println!("  ADMITTED by the bound extractor, arena capacity {cap}."),
        Err(e) => println!("  REFUSED AT: the resource-bound analysis -- {e:?}"),
    }
    println!("================\n");
}
