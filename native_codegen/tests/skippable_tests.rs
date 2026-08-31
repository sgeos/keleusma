//! Which tests can pass without testing, and how many are doing so.
//!
//! # The shape, arriving from a different direction
//!
//! The name audit asked whether a test proves the claim in its name. This asks
//! whether it **ran at all**. A test that returns early when a toolchain is
//! absent reports as **passed** and contributes to the total this line quotes as
//! evidence — so **a suite's pass count means only as much as the fraction of it
//! that executed.**
//!
//! # What is pinned, and what is deliberately not asserted
//!
//! The **population** is pinned: tests whose body can return before asserting
//! anything. A new one announces itself here rather than joining the total
//! unnoticed.
//!
//! **Whether a skip actually occurs is NOT asserted**, and that is deliberate.
//! These tests need a C compiler and a linker; a machine without one should not
//! see a failure, because **the defect is invisibility, not the skip**. Measured
//! on the machine of record: **none of them skipped** — `retcon_m2` really
//! builds, links and runs, printing its subprocess output — so the risk is
//! latent rather than active.

use std::collections::BTreeSet;

/// Tests whose body can reach a `return` before any assertion.
///
/// Pinned so an eleventh announces itself. **Not a defect list**: every one of
/// these executed on the machine of record, and each prints a loud banner when
/// it does skip.
const SKIPPABLE: &[&str] = &[
    "a_deeper_call_chain_raises_the_bound",
    // **A RENAME, NOT AN ADDITION.** This entry replaced route 4's earlier
    // refusal test when the float shared slot opened that route. The test kept
    // its early return for a reference compiler that cannot build a `Float`
    // slot, and changed only what it asserts once it gets past it. The
    // population is unchanged at ten, which is the fact this pin exists to
    // state.
    //
    // **The old name is deliberately not written here.** The citation guard
    // caught this comment naming it: a citation that resolves to nothing reads
    // as coverage while being none, and a dead test name in a comment is
    // exactly that shape whether or not it was ever meant as a citation.
    "a_float_data_slot_lowers_at_eight_bytes_and_refuses_at_any_other_width",
    "a_linked_native_object_agrees_with_the_vm",
    "a_linked_object_with_natives_and_a_data_segment_agrees_with_the_vm",
    "a_native_stack_bound_is_computable_end_to_end",
    "m2_a_buffer_large_enough_never_reaches_the_arena_at_run_time",
    "m2_a_retcon_coroutine_spawns_resumes_and_releases_when_linked",
    "m3_timing_a_resumption_against_an_equivalent_direct_call",
    "spike_report_real_native_frame",
    "trap_child_runs_one_module_natively",
];

/// # ⚠ A KNOWN FALSE POSITIVE: A `return` INSIDE A CLOSURE
///
/// **This pin caught its own author within one increment.** A test added the
/// next day used `else { return false; }` inside a closure, and the scanner —
/// which cannot tell a closure's value from a test's early exit — flagged it as
/// able to pass without testing. **The pin fired correctly on its own terms and
/// was wrong about the cause.**
///
/// The repair was to write that closure without a `return`, **not** to add the
/// test to the pinned list: recording a test as skippable when it cannot skip
/// would corrupt the very figure this file exists to keep honest. If a future
/// test genuinely needs a closure-local `return`, add it here with that reason
/// stated rather than silently.
///
/// Is this line a `return` STATEMENT, rather than the word in prose?
///
/// **The first version asked whether the line contained "return" and counted
/// 33 tests where a second instrument counted 10.** The difference was entirely
/// comments — `// while still returning the right low word.` matched. Two
/// instruments disagreeing is what surfaced it; either alone looked reasonable.
fn is_early_return(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with("//") || t.starts_with("///") {
        return false;
    }
    t.starts_with("return ")
        || t.starts_with("return;")
        || t.ends_with("return;")
        || t.contains("{ return")
        || t.contains("=> return")
}

/// Scan the package's own test sources for the pattern.
///
/// **Reads what is there rather than trusting the constant**, which is the whole
/// point: a pinned list that pinned itself would be the guard whose precondition
/// guarantees its conclusion.
fn skippable_in_sources() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(rd) = std::fs::read_dir("tests") else {
        return out;
    };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if !p.extension().is_some_and(|x| x == "rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let lines: Vec<&str> = src.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if l.trim() != "#[test]" {
                continue;
            }
            // The function line may follow attributes such as `#[cfg(...)]`.
            let Some(j) = (i + 1..lines.len().min(i + 6)).find(|k| lines[*k].starts_with("fn "))
            else {
                continue;
            };
            let name = lines[j]
                .trim_start_matches("fn ")
                .split('(')
                .next()
                .unwrap_or_default()
                .to_string();
            let mut asserted = false;
            for l in &lines[j + 1..] {
                if l.starts_with('}') {
                    break;
                }
                if l.contains("assert") || l.contains("panic!") {
                    asserted = true;
                }
                if !asserted && is_early_return(l) {
                    out.insert(name.clone());
                    break;
                }
            }
        }
    }
    out
}

/// The population is what it was measured to be.
#[test]
fn the_set_of_tests_that_can_pass_without_testing_is_pinned() {
    let found = skippable_in_sources();
    let pinned: BTreeSet<String> = SKIPPABLE.iter().map(|s| (*s).to_string()).collect();

    println!("\n================ TESTS THAT CAN RETURN BEFORE ASSERTING");
    println!("  found in sources : {}", found.len());
    for n in &found {
        println!("    {n}");
    }
    let added: Vec<&String> = found.difference(&pinned).collect();
    let gone: Vec<&String> = pinned.difference(&found).collect();
    if !added.is_empty() || !gone.is_empty() {
        println!("  newly skippable : {added:?}");
        println!("  no longer       : {gone:?}");
    }
    println!(
        "\n  NONE OF THESE SKIPPED ON THE MACHINE OF RECORD. They need a C compiler\n  \
         and a linker, and each prints a loud banner when it does skip. **The\n  \
         population is pinned so an eleventh announces itself; whether a skip\n  \
         OCCURS is deliberately not asserted, because a machine without a\n  \
         toolchain should not see a failure.**\n================\n"
    );

    // **NON-VACUITY.** A scanner that found nothing would satisfy any claim about
    // the population, and the pinned list would then be pinning itself.
    assert!(
        !found.is_empty(),
        "the scanner found no skippable test at all, so it is not reading the \
         sources and this pin means nothing"
    );
    assert_eq!(
        found, pinned,
        "the set of tests that can return before asserting has changed. That is \
         not necessarily wrong -- a new toolchain-dependent test is legitimate -- \
         but it must be a decision rather than a silent addition to the pass count."
    );
}
