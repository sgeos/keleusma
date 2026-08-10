// ---------------------------------------------------------------------------
// PREPARED WHILE ANOTHER SESSION'S GATE HELD THE MACHINE. Not yet compiled.
// Append to native_codegen/tests/coroutine_feasibility.rs.
//
// Closes the ONE clause `V0_4_0_NATIVE_CODEGEN.md` R4.4 leaves at medium
// confidence: "Whether inkwell exposes `coro.id.retcon` with a safe wrapper
// still requires a source-tree audit when implementation begins."
//
// The source-tree half is already answered by reading: inkwell 0.9.0 has NO
// coroutine wrapper at all. Grepping its `src/` for `coro` returns only
// `passes.rs`, a pass-pipeline name. So the generic `Intrinsic::find` plus
// `get_declaration` route is the only route.
//
// WHAT THIS ADDS, AND WHY THE EXISTING TEST IS NOT ENOUGH.
//
// `the_returned_continuation_family_exists_as_well` asserts only
// `Intrinsic::find(name).is_some()`. That proves LLVM knows the NAME. It does
// not prove inkwell can EMIT a declaration, which is the thing R4.4 asks and the
// thing Workstream B would fail on.
//
// The gap is not pedantic. `coro.id.retcon` takes allocator and deallocator
// function pointers, making it a candidate for being overloaded — and
// `get_declaration` returns `None` for an overloaded intrinsic given no overload
// types. So `find` succeeding while declaration fails is exactly the plausible
// case, and the switch-resume family in the same file IS checked both ways.
// The family the architecture specifies is the one validated more weakly.
//
// REPORTING RATHER THAN ASSERTING, deliberately. Whether LLVM 22.1 lets inkwell
// declare these is a fact about the toolchain, not about our code. A red test
// would be the wrong shape: it would block the suite over something no change of
// ours can fix. The printed answer converts R4.4's clause from medium confidence
// to measured, and the assertion covers only what a defect on our side could
// break.
// ---------------------------------------------------------------------------

/// MEASURES R4.4's open clause: can the retcon family actually be DECLARED?
///
/// Run with `--nocapture` to read the answer. The outcome decides Workstream B's
/// shape:
///
/// - **declarable** — the architecture's `coro.id.retcon` design is reachable
///   through inkwell alone, and no escape hatch is needed;
/// - **overloaded, needs types** — declarable once the overload is supplied;
///   a small amount of extra work, not a blocker;
/// - **not declarable** — Workstream B needs the `coro_intrinsics.rs` `llvm-sys`
///   escape hatch R4.4 anticipates, or the switched-resume form instead.
#[test]
fn spike_report_retcon_declarability() {
    let ctx = Context::create();
    let m = ctx.create_module("retcon_decl");

    println!("\n================ R4.4: can the retcon family be DECLARED?");
    let mut all_findable = true;
    // Explicitly typed, because `&[]` alone does not infer `BasicTypeEnum` here.
    // The existing test in this file types it the same way; copying that was the
    // difference between compiling and not.
    let no_overloads: &[inkwell::types::BasicTypeEnum] = &[];

    for name in ["llvm.coro.id.retcon", "llvm.coro.id.retcon.once"] {
        let Some(intr) = Intrinsic::find(name) else {
            all_findable = false;
            println!("  {name}: NOT FOUND in LLVM 22.1");
            continue;
        };
        let overloaded = intr.is_overloaded();
        // No overload types supplied. For a non-overloaded intrinsic this is the
        // whole answer; for an overloaded one `get_declaration` returns `None`
        // and the printed `overloaded` flag explains why.
        let declared = intr.get_declaration(&m, no_overloads).is_some();

        println!("  {name}");
        println!("     found        : yes");
        println!("     overloaded   : {overloaded}");
        println!("     declarable   : {declared}   (with no overload types)");
        println!(
            "     -> {}",
            match (declared, overloaded) {
                (true, _) => "reachable through inkwell alone; no escape hatch needed",
                (false, true) => "supply the overload types; extra work, not a blocker",
                (false, false) =>
                    "NOT declarable and NOT overloaded: needs the llvm-sys escape hatch",
            }
        );
    }

    // The switch-resume family, for contrast. It is already asserted elsewhere in
    // this file; printed here so both families appear side by side rather than
    // requiring a reader to correlate two tests.
    if let Some(id) = Intrinsic::find("llvm.coro.id") {
        println!(
            "\n  llvm.coro.id (switch-resume, for contrast): overloaded={}, declarable={}",
            id.is_overloaded(),
            id.get_declaration(&m, no_overloads).is_some()
        );
    }
    println!("================\n");

    // The only assertion. Absence of the NAMES would contradict the existing
    // test in this file and would mean the toolchain changed under us, which is
    // a real regression. Declarability is reported rather than asserted, because
    // no change of ours can alter it.
    assert!(
        all_findable,
        "the retcon intrinsics vanished from LLVM 22.1; this contradicts \
         the_returned_continuation_family_exists_as_well and means the toolchain moved"
    );
}
