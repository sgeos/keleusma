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
fn every_coroutine_intrinsic_is_declarable_through_the_bindings() {
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
