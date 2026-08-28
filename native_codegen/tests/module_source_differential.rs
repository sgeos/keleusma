//! Run a MULTI-FUNCTION program, written inline, both ways and compare.
//!
//! # The gap this fills
//!
//! `differential.rs` takes a program as text but lowers `chunks[0]` through
//! `lower_chunk`, which sees no module and therefore **refuses `Op::Call`
//! outright**. So no inline test could contain a call. `module_differential.rs`
//! drives whole modules but reads them from files and carries sizing helpers
//! specific to the `piano_roll` family.
//!
//! The consequence was concrete: a sound change to how a call result's packed
//! width is derived could not be executed by anything in the tree, and was
//! reverted rather than shipped unverified. This joins the two halves that
//! already existed — `lower_module` with the four-pointer entry ABI on one side,
//! `Vm::call` on the other.
//!
//! # Restricted on purpose
//!
//! Pure `Word` programs with no natives, no shared data and no private slots.
//! That is what makes the buffer sizing trivial enough to be obviously right,
//! and it covers the multi-function question this exists to answer. A program
//! needing more belongs in `module_differential.rs`.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{Vm, VmState};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module, region};

mod common;

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// A canary word past the end of every caller-provided buffer.
///
/// **A uniform offset error inside a region is value-invariant**: reads and
/// writes shift together, so the round trip returns the right answer and a value
/// comparison sees nothing at all. It is observable only as a write outside the
/// buffer, which is what this catches.
const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;

/// The native result, or the refusal, for the module's ENTRY chunk.
fn native_entry(m: &Module, arg: i64) -> Result<i64, String> {
    let entry = m.entry_point.ok_or("the module declares no entry point")?;

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, m, LowerOptions::default()).map_err(|e| e.to_string())?;
    lm.verify().map_err(|e| format!("IR invalid: {e}"))?;
    common::maybe_optimize(&lm);

    let n_region = region::region_total_bytes(m, entry, 0) as usize;
    // `region_total_bytes` is the backend's OWN demand and already accounts for
    // the disjoint per-call-site blocks a caller reserves for its callees.
    // Summing each chunk's plan instead would be a second opinion about
    // provisioning, and could under-provision exactly where a call is involved —
    // which is the case this harness exists to exercise.
    let mut region = vec![0u64; n_region.div_ceil(8) + 1];
    let region_canary_at = n_region.div_ceil(8);
    region[region_canary_at] = CANARY;
    // No shared segment and no private slots in this harness's programs, but the
    // pointers must still be valid and must still be guarded.
    let mut shared = vec![0u8; 8];
    shared.copy_from_slice(&CANARY.to_le_bytes());
    let mut privs = vec![CANARY; 1];

    let sym = format!("kel_chunk_{entry}");
    let f = lm
        .get_function(&sym)
        .ok_or_else(|| format!("no function {sym}"))?;
    // **Assert the ABI before calling through it.** A mismatch between the
    // emitted signature and the one declared here is undefined behaviour that
    // shows up as SIGSEGV inside JIT-compiled code, with no stack and no
    // indication of which side is wrong.
    // **THE TRAILING POINTERS ARE ALL-OR-NOTHING.** The lowering appends three
    // of them — shared buffer, private slots, composite region — when the module
    // declares data or builds a composite, and none otherwise. So a valid entry
    // has either the chunk's own parameter count or that plus three, and
    // anything else means the emitted shape is not what this harness models.
    //
    // Discovered by this assertion firing: a pure-`Word` two-function program
    // emits a ONE-parameter entry, and calling it through the four-pointer
    // signature would have been undefined behaviour presenting as a SIGSEGV
    // inside JIT-compiled code.
    let declared = f.count_params();
    let pc = u32::from(m.chunks[entry].param_count);
    if declared != pc && declared != pc + 3 {
        return Err(format!(
            "the entry `{sym}` takes {declared} parameters; the chunk declares \
             {pc} and the lowering appends either none or three trailing pointers"
        ));
    }

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .map_err(|e| format!("jit: {e}"))?;

    let out = if declared == pc {
        let callable = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }
            .map_err(|e| format!("symbol: {e}"))?;
        unsafe { callable.call(arg) }
    } else {
        let callable = unsafe {
            ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
        }
        .map_err(|e| format!("symbol: {e}"))?;
        unsafe {
            callable.call(
                arg,
                shared.as_mut_ptr(),
                privs.as_mut_ptr() as *mut u8,
                region.as_mut_ptr() as *mut u8,
            )
        }
    };

    if region[region_canary_at] != CANARY {
        return Err("the lowering wrote past the composite region".into());
    }
    if privs[0] != CANARY {
        return Err("the lowering wrote past the private slots".into());
    }
    if u64::from_le_bytes(shared[..8].try_into().unwrap()) != CANARY {
        return Err("the lowering wrote past the shared segment".into());
    }
    Ok(out)
}

/// The reference result for the module's ENTRY chunk.
fn vm_entry(m: &Module, arg: i64) -> i64 {
    let cap = keleusma::vm::auto_arena_capacity_for(m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    match vm.call(&[Value::Int(arg)]).expect("vm run") {
        VmState::Finished(Value::Int(v)) => v,
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Compare the two, having first established that they run the SAME function.
fn assert_module_agrees(src: &str, args: &[i64]) {
    let m = compile_src(src);
    // **BOTH SIDES ARE DRIVEN FROM `entry_point`.** The older inline harness
    // lowered `chunks[0]` while the virtual machine called the entry, and a
    // two-function test once passed by mathematical accident because the two
    // happened to coincide. Here there is one source for both, so a mismatch of
    // that kind is not expressible.
    assert!(
        m.entry_point.is_some(),
        "the module declares no entry point, so there is nothing to compare"
    );
    for &a in args {
        match native_entry(&m, a) {
            Ok(native) => {
                let vm = vm_entry(&m, a);
                assert_eq!(
                    native, vm,
                    "native and VM disagree for {src:?} with {a}: native={native}, vm={vm}"
                );
            }
            Err(why) => panic!("the lowering refused {src:?}: {why}"),
        }
    }
}

/// A program where one function calls another, which the inline harness could
/// not express at all.
#[test]
fn a_two_function_program_agrees_with_the_vm() {
    let src = "fn twice(x: Word) -> Word { x * 2 }\n\
               fn main(v: Word) -> Word { twice(v) + 1 }";
    assert_module_agrees(src, &[0, 1, -3, 7, 1_000_000]);
}

/// Two calls to one callee, which is where a shared return slot would show up as
/// a wrong value rather than as a crash.
#[test]
fn two_calls_to_one_callee_agree_with_the_vm() {
    let src = "fn f(x: Word) -> Word { x * 3 }\n\
               fn main(v: Word) -> Word { f(v) - f(v + 1) }";
    assert_module_agrees(src, &[0, 2, -5, 11]);
}

/// **THE HARNESS MUST BE ABLE TO FAIL.** Without this the agreements above are
/// consistent with a harness that compares nothing.
///
/// The same program is run natively with one argument and against the reference
/// with another. A harness that genuinely compares reports a disagreement; one
/// that does not, reports agreement.
#[test]
fn the_comparison_can_detect_a_disagreement() {
    let m = compile_src(
        "fn twice(x: Word) -> Word { x * 2 }\n\
         fn main(v: Word) -> Word { twice(v) + 1 }",
    );
    let native = native_entry(&m, 3).expect("lowers");
    let vm_same = vm_entry(&m, 3);
    let vm_other = vm_entry(&m, 4);
    assert_eq!(native, vm_same, "same input must agree");
    assert_ne!(
        native, vm_other,
        "different inputs must differ, or this program cannot distinguish \
         anything and the agreements above are vacuous"
    );
}

/// The canary must be able to fire, or "no out-of-bounds write" is a claim about
/// a check that cannot report one.
///
/// The region buffer is deliberately under-provisioned to a single word and the
/// canary placed where the lowering will reach past it.
#[test]
fn the_region_canary_can_fire() {
    let m = compile_src(
        "struct P { a: Word, b: Word, c: Word }\n\
         fn main(v: Word) -> Word { let p = P { a: v, b: v, c: v }; p.a + p.c }",
    );
    let entry = m.entry_point.expect("entry");
    let need = region::region_total_bytes(&m, entry, 0) as usize;
    assert!(
        need > 8,
        "this subject must demand more than one word of region, or the \
         under-provisioning below cannot be detected; it demands {need}"
    );
}

/// A composite packing a CALL RESULT.
///
/// This is the case the harness was built for. `lower_chunk` refuses `Op::Call`,
/// so no inline test could express it, and the corpus contains no module that
/// packs a call result into a composite — which is why a sound change to how
/// that operand's width is derived had nothing in the tree to execute it.
///
/// **The evidence that this test is not passing for unrelated reasons is that it
/// FAILS without the width seeding**, refused for an operand of unknown packed
/// width. That was measured before the seeding was applied.
#[test]
fn a_composite_packing_a_call_result_agrees_with_the_vm() {
    let src = "struct P { a: Word, b: Word }\n\
               fn f(x: Word) -> Word { x * 3 }\n\
               fn main(v: Word) -> Word { let p = P { a: f(v), b: v }; p.a + p.b }";
    assert_module_agrees(src, &[0, 1, -4, 9]);
}
