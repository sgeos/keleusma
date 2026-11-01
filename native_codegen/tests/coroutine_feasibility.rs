//! Workstream B feasibility: can this toolchain build, split and run an LLVM
//! coroutine, through the bindings the backend actually uses?
//!
//! `V0_3_X_ROADMAP.md` calls sub-coroutine lowering "the load-bearing primitive"
//! and "where the risk concentrates". Nothing had probed it. This file is the
//! probe, kept as tests rather than written up as a note, because the last
//! mechanism probed this way (`.stack_sizes`) turned out to be blocked at the
//! BINDING layer and not at the LLVM layer. Evidence gathered with command-line
//! tools would not have caught that, and does not here either.
//!
//! **Nothing here is a lowering.** Mapping Keleusma's `Stream`/`Yield`/`Reset`
//! onto coroutines is Workstream B proper and is not attempted. These tests
//! establish which parts of the mechanism are reachable, so that work starts
//! from measured ground.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::intrinsics::Intrinsic;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};

fn host_machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).expect("init native target");
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        )
        .expect("target machine")
}

#[test]
fn the_coroutine_intrinsics_this_backend_would_need_are_declarable() {
    // **RENAMED from a claim about EVERY coroutine intrinsic.** The body iterates
    // nine named ones; LLVM defines more (`coro.alloc`, `coro.promise`,
    // `coro.save`, `coro.noop` among them), so "every" asserted more than a fixed
    // list can show. **The narrower claim is also the useful one** — what matters
    // is whether the intrinsics a suspension lowering would need are reachable
    // through the bindings, not whether the list is exhaustive.
    // The concern this settles. Every intrinsic that opens or closes a coroutine
    // traffics in LLVM's `token` type: `llvm.coro.id` RETURNS one, and
    // `llvm.coro.suspend` and `llvm.coro.end` CONSUME one. inkwell has no
    // `token` type constructor -- it panics with "FIXME: Unsupported type:
    // Token" if one reaches its type enum -- so these declarations cannot be
    // written the way every other declaration in this backend is written.
    //
    // `Intrinsic::find` plus `get_declaration` sidesteps that entirely by asking
    // LLVM to construct the signature from the intrinsic's own definition, so no
    // Rust code ever names the token type. Verified rather than assumed, because
    // the natural inference from "inkwell has no token type" is that coroutines
    // are unreachable, and that inference is wrong.
    let ctx = Context::create();
    let m = ctx.create_module("coro_decl");

    for name in [
        "llvm.coro.id",
        "llvm.coro.size",
        "llvm.coro.begin",
        "llvm.coro.suspend",
        "llvm.coro.free",
        "llvm.coro.end",
        "llvm.coro.resume",
        "llvm.coro.destroy",
        "llvm.coro.done",
    ] {
        let intr = Intrinsic::find(name)
            .unwrap_or_else(|| panic!("LLVM 22.1 does not know the intrinsic {name}"));
        // `llvm.coro.size` is overloaded on its return width and needs the
        // overload supplied; the rest are not overloaded.
        let overloads: &[inkwell::types::BasicTypeEnum] = if name == "llvm.coro.size" {
            &[ctx.i32_type().into()]
        } else {
            &[]
        };
        assert!(
            intr.get_declaration(&m, overloads).is_some(),
            "{name} is known to LLVM but could not be declared through inkwell"
        );
    }
}

#[test]
fn the_returned_continuation_family_exists_as_well() {
    // The inventory recorded Workstream B as going "through the returned-
    // continuation intrinsic family", and `V0_4_0_NATIVE_CODEGEN.md` names
    // `llvm.coro.id.retcon.once` for one-shot coroutines. Both are present, so
    // the choice between the switched-resume form and the returned-continuation
    // form is a DESIGN decision rather than an availability one.
    for name in ["llvm.coro.id.retcon", "llvm.coro.id.retcon.once"] {
        assert!(
            Intrinsic::find(name).is_some(),
            "{name} is absent from LLVM 22.1, which would force the \
             switched-resume form"
        );
    }
}

#[test]
fn the_coroutine_passes_run_through_the_bindings() {
    // The passes matter as much as the intrinsics. A pipeline that can declare
    // the intrinsics but cannot run `coro-split` emits a module that fails at
    // code generation, which is a worse outcome than not starting.
    let machine = host_machine();
    let ctx = Context::create();
    let m = ctx.create_module("passes");

    assert!(
        m.run_passes(
            "coro-early,cgscc(coro-split),coro-cleanup",
            &machine,
            PassBuilderOptions::create(),
        )
        .is_ok(),
        "the coroutine pass pipeline must be runnable through inkwell"
    );

    // MUST-NOT-FIRE CASE: a nonexistent pass must be REJECTED. Without this the
    // assertion above is satisfied by a `run_passes` that accepts any string,
    // and would say nothing about coroutine support specifically.
    assert!(
        m.run_passes(
            "definitely-not-a-real-pass",
            &machine,
            PassBuilderOptions::create(),
        )
        .is_err(),
        "run_passes accepted a nonexistent pass, so its success proves nothing"
    );
}
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
