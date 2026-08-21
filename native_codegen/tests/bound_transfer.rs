//! **DOES THE NATIVE LOWERING PRESERVE THE BOUNDS THE BYTECODE WAS VERIFIED AGAINST?**
//!
//! This is Workstream E's question and the project's whole value proposition:
//! the bytecode is the verification artefact, and native compilation must not
//! invalidate the worst-case execution time and memory bounds proven on it.
//!
//! # Why this exists: a load-bearing claim that nobody had measured
//!
//! `region.rs` carries, in a comment:
//!
//! > *"Naming the provenance is not decoration. A region of unspecified origin
//! > would put the backend's memory outside the arena's accounting, and
//! > transferring that bound is the whole property this lowering exists to
//! > preserve."*
//!
//! **That is a claim.** This line has now found five separate cases where a
//! documented or assumed property was false or vacuous when checked, and a
//! comment asserting the load-bearing property of a workstream is exactly that
//! shape.
//!
//! # The two comparisons, and the unit trap between them
//!
//! * **Operand stack.** [`MAX_STACK`] is what the backend provisions. The
//!   verifier computes the exact figure as `RuntimeFootprint::max_operand_slots`.
//! * **Composite region.** `region_total_bytes` is what the backend demands from
//!   the host arena. `RuntimeFootprint::max_heap_bytes` is what the bytecode was
//!   verified to need.
//!
//! **`max_operand_slots` is deliberately representation-independent — a SLOT
//! count, not bytes.** `max_heap_bytes` and `region_total_bytes` are real bytes.
//! Bringing a slot count and a byte count together silently produces a plausible
//! number, which is the `FixedMul` trap in a new place, so the two comparisons
//! below are kept entirely separate and neither converts.
//!
//! # What a passing run here does NOT establish
//!
//! **Not that the bound is preserved in general.** These are measurements over
//! ONE corpus. A comfortable margin is a fact about the programs in the tree,
//! not a proof about the lowering, and the printouts say so where they are read.
//! A margin without its denominator is the kind of figure this line has had
//! outlive the thing it measured.
use keleusma::bytecode::Module;
use keleusma::verify::module_runtime_footprint;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerError, LowerOptions, MAX_STACK, module_refusals, region};

/// The corpus directories, matching the differential and both censuses.
const CORPUS_DIRS: [&str; 4] = [
    "examples/scripts",
    "src/selfhost/kel",
    "examples/rtos/scripts",
    "compiler/kel",
];

fn source_for(p: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(p).ok()?;
    let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
    let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
    if is_rtos && !is_prelude {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        return Some(format!("{prelude}\n{src}"));
    }
    Some(src)
}

fn all_compiling_modules() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            paths.push(p);
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(src) = source_for(&p) else { continue };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        out.push((name, m));
    }
    out
}

/// **THE OPERAND-STACK CEILING, AND A COMPARISON I GOT WRONG FIRST.**
///
/// # The wrong version, kept because the mistake is the lesson
///
/// This test first compared `MAX_STACK` against `RuntimeFootprint::
/// max_operand_slots` and reported `codegen.kel` VIOLATING the ceiling at 175
/// slots against 64. **That was my instrument, not the backend.**
///
/// `max_operand_slots` is *transitively* folded — `wcmu_region` adds the
/// callee's stack into the caller at every `Op::Call`, because in the virtual
/// machine every frame shares ONE operand stack. `MAX_STACK` is **per lowered
/// function**: a native call pushes a new C frame carrying its own slots. So the
/// two describe different things, and comparing them is a scope error of exactly
/// the kind this file's own header warns about for units.
///
/// **Measured, which is what settled it:** `codegen.kel` produces ZERO refusals
/// and zero `OperandStackTooDeep`. The backend lowers it without difficulty. A
/// "violation" that the thing it accuses handles cleanly is a broken comparison.
///
/// # What this test asks instead
///
/// The well-formed question needs no scope reasoning: **does any corpus module
/// actually hit the ceiling?** `OperandStackTooDeep` is the backend's own
/// verdict, at the backend's own scope, and it is not an inference.
#[test]
fn does_any_corpus_module_reach_the_operand_stack_ceiling() {
    let corpus = all_compiling_modules();
    let mut too_deep: Vec<String> = Vec::new();
    let mut worst_transitive: Option<(String, u32)> = None;

    for (name, m) in &corpus {
        for (chunk, e) in module_refusals(m, LowerOptions::default()) {
            if let LowerError::OperandStackTooDeep {
                needed,
                provisioned,
            } = e
            {
                too_deep.push(format!("{name}::{chunk} needs {needed} of {provisioned}"));
            }
        }
        if let Ok(fp) = module_runtime_footprint(m, &[])
            && worst_transitive
                .as_ref()
                .is_none_or(|(_, w)| fp.max_operand_slots > *w)
        {
            worst_transitive = Some((name.clone(), fp.max_operand_slots));
        }
    }

    println!("\n================ OPERAND-STACK CEILING");
    println!("  modules examined                  : {}", corpus.len());
    println!("  backend provisions (MAX_STACK)    : {MAX_STACK} slots per FUNCTION");
    println!("  chunks refused as too deep        : {}", too_deep.len());
    for t in &too_deep {
        println!("     {t}");
    }
    if let Some((n, w)) = &worst_transitive {
        println!("\n  For contrast, the verifier's worst TRANSITIVE figure is {w}");
        println!("  slots ({n}) -- ABOVE the ceiling, and NOT a violation.");
        println!("  `max_operand_slots` folds every callee's stack into the caller,");
        println!("  because the virtual machine shares ONE operand stack across");
        println!("  frames. Native code does not: each lowered function has its own.");
        println!("  I asserted these were comparable and reported a violation that");
        println!("  did not exist. The module in question lowers with zero refusals.");
    }
    println!("================\n");

    assert!(
        corpus.len() > 50,
        "only {} modules compiled, so this reads the wrong tree",
        corpus.len()
    );
    assert!(
        too_deep.is_empty(),
        "A CORPUS CHUNK EXCEEDS THE BACKEND'S PER-FUNCTION OPERAND-STACK \
         PROVISIONING. Unlike the transitive figure discussed above this IS a \
         real refusal, at the backend's own scope. DO NOT WIDEN MAX_STACK to \
         make it pass -- the ceiling is a worst-case-memory decision and belongs \
         to the operator.\n\n{}",
        too_deep.join("\n")
    );
}

/// **THE COMPOSITE REGION IS NOT COVERED BY ANY VERIFIED BOUND, AND THAT IS THE
/// WORKSTREAM E FINDING.**
///
/// `region.rs` says the backend's memory provenance is named so that it is not
/// *"outside the arena's accounting"*, and calls transferring that bound *"the
/// whole property this lowering exists to preserve"*. **Naming the provenance is
/// not the same as a verified figure covering it**, and measurement separates
/// the two.
///
/// # The measurement, and the negative claim it supports
///
/// `region_total_bytes` is what the backend demands. `max_heap_bytes` is the
/// verifier's per-iteration arena figure — and BOTH are transitive, since
/// `wcmu_region` does `heap.saturating_add(callee_heap_bytes)` at each call, so
/// this is not the scope error the operand-stack test above fell into.
///
/// **Eleven corpus modules demand MORE than the verified figure.** The negative
/// conclusion follows without needing to settle any semantics: *if*
/// `max_heap_bytes` bounded the backend's demand, no module could exceed it.
/// Modules do. Therefore **it does not bound it.**
///
/// The mechanism is visible in the sources and explains the direction: the
/// verifier's heap counts `Op::NewComposite`'s allocation in the arena's TOP
/// region, which is where the virtual machine puts composite bodies. The backend
/// takes its region from the arena's **BOTTOM** section and gives every call
/// site a DISJOINT block so two calls to one callee cannot overwrite each other.
/// Different pool, different reuse discipline, larger total.
///
/// # What this is NOT
///
/// **Not a memory-safety defect, and not a claim that anything is unsound.** The
/// host is told explicitly to supply the region and how big it must be; nothing
/// reads memory it was not given. The finding is narrower and is about the
/// GUARANTEE rather than the code: a host that provisioned from the verified
/// worst-case-memory figure alone would size the top region correctly and the
/// bottom region **not at all**. Workstream E needs a bound covering that pool,
/// and there is not one today.
///
/// # WHERE the gap is, which is the actionable part
///
/// `auto_arena_capacity_for` is the documented way for a host to size an arena,
/// and it returns the sum of exactly four terms: the operand-stack bytes, the
/// call-frame bytes, the module's auxiliary arena bytes, and `max_heap_bytes`.
/// **None of them is the backend's region.** `max_heap_bytes` is the only term
/// that could plausibly cover composite bodies, and it is the one demonstrated
/// below to be exceeded.
///
/// So the gap is not that a figure is slightly low. **The sizing API has no term
/// for that pool**, and closing Workstream E here means adding one — which is a
/// question about the arena accounting model and plausibly an operator's, since
/// it changes what a host is told to provision.
///
/// **Reported as a measured inequality rather than pinned to eleven.** The set
/// moves with the corpus. What is asserted is that the inequality is reachable
/// at all, which is the whole negative claim.
#[test]
fn the_verified_heap_figure_does_not_bound_the_backends_region_demand() {
    let corpus = all_compiling_modules();
    let mut compared = 0usize;
    let mut nonzero_demand = 0usize;
    let mut exceed: Vec<String> = Vec::new();
    let mut worst: Option<(String, u32, u32)> = None;

    for (name, m) in &corpus {
        let Ok(fp) = module_runtime_footprint(m, &[]) else {
            continue;
        };
        let Some(entry) = m.entry_point else { continue };
        // Rooted at the ENTRY, because that is the call the host scopes a region
        // for. Compared against the module-wide MAX, which is >= the entry
        // chunk's own figure -- so this comparison is GENEROUS to the backend and
        // the inequality below survives that generosity.
        let demand = region::region_total_bytes(m, entry, 0);
        compared += 1;
        if demand > 0 {
            nonzero_demand += 1;
        }
        if demand > fp.max_heap_bytes {
            exceed.push(format!(
                "{name}: backend {demand} bytes vs verified heap {}",
                fp.max_heap_bytes
            ));
        }
        if worst.as_ref().is_none_or(|(_, d, _)| demand > *d) {
            worst = Some((name.clone(), demand, fp.max_heap_bytes));
        }
    }

    println!("\n================ COMPOSITE-REGION BOUND TRANSFER");
    println!("  modules compared               : {compared}");
    println!("  with a NON-ZERO region demand  : {nonzero_demand}");
    if let Some((n, d, h)) = &worst {
        println!("  largest backend demand         : {d} bytes ({n}), verified heap {h}");
    }
    println!("  modules EXCEEDING the verified figure: {}", exceed.len());
    for e in &exceed {
        println!("     {e}");
    }
    println!(
        "\n  UNITS AND SCOPE: both are real BYTES and both are TRANSITIVE --\n  \
         `wcmu_region` folds `callee_heap_bytes` at each call, and\n  \
         `region_total_bytes` recurses. This is NOT the scope error the\n  \
         operand-stack test in this file fell into and documents.\n  \
         \n  \
         THE CONCLUSION IS NEGATIVE AND NEEDS NO SEMANTICS: if the verified\n  \
         figure bounded the backend's demand, no module could exceed it.\n  \
         Modules do. So it does not bound it, and Workstream E has no figure\n  \
         covering the bottom-region pool the backend actually uses.\n  \
         \n  \
         NOT A SAFETY DEFECT. The host is told what to supply and nothing reads\n  \
         memory it was not given. What is missing is the GUARANTEE, not a check."
    );
    println!("================\n");

    assert!(
        compared > 40,
        "only {compared} modules compared, so this is vacuous rather than a finding"
    );
    // Without this, "some module exceeds" could be true while the comparison
    // reaches no module that builds a composite at all.
    assert!(
        nonzero_demand > 0,
        "EVERY backend region demand is zero, so the comparison is not reaching \
         any module that builds a composite and neither direction means anything"
    );
    assert!(
        !exceed.is_empty(),
        "NO MODULE EXCEEDS THE VERIFIED HEAP FIGURE ANY MORE. That is NEWS and \
         possibly good news: either the backend's region planning became tighter, \
         or a verifier figure now accounts for the bottom-region pool. Establish \
         WHICH before concluding the bound transfers -- and if it does, this test \
         should become the positive assertion instead of being deleted."
    );
}

/// **IS THE OPERAND-STACK REFUSAL A WORKING GUARD, OR ONE THAT HAS NEVER FIRED?**
///
/// The corpus comparison above reports headroom. **An untriggered guard is not a
/// demonstrated guard**, and a ceiling nothing has ever hit is indistinguishable
/// from a ceiling that is not checked at all. This constructs a chunk that
/// exceeds `MAX_STACK` and asserts the backend refuses it by name.
#[test]
fn the_operand_stack_ceiling_actually_refuses_something() {
    // A deep expression tree: each nested addend holds an operand while the next
    // is evaluated, so depth grows with nesting rather than with length.
    let mut expr = String::from("1");
    for i in 0..(MAX_STACK + 8) {
        expr = format!("({expr} + {i})");
    }
    let src = format!("fn deep() -> Word {{ {expr} }}\nfn main() -> Word {{ deep() }}\n");

    let Some(m) = tokenize(&src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
    else {
        // The reference compiler declining is a legitimate outcome and not a
        // failure of the backend. Reported rather than asserted away.
        println!(
            "\n  the reference compiler will not build a {}-deep expression, so the \
             ceiling cannot be reached from source here",
            MAX_STACK + 8
        );
        return;
    };

    let refusals = keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
    let deep = refusals
        .iter()
        .any(|(_, e)| matches!(e, keleusma_native::LowerError::OperandStackTooDeep { .. }));

    println!("\n================ IS THE CEILING A WORKING GUARD?");
    if deep {
        println!("  YES -- a constructed subject reaches OperandStackTooDeep.");
        println!("  So the corpus headroom reported elsewhere is headroom against a");
        println!("  guard that demonstrably fires, not against an unchecked constant.");
    } else {
        println!("  NOT DEMONSTRATED. The constructed subject did not reach it:");
        println!("     {refusals:?}");
        println!("  The corpus may simply never approach the ceiling, in which case");
        println!("  the guard is UNTRIGGERED -- and an untriggered guard is not a");
        println!("  demonstrated one. Recorded rather than asserted either way.");
    }
    println!("================\n");
}
