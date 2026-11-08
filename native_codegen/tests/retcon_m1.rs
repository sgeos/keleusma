//! **R4.1 MILESTONE M1: a returned-continuation coroutine driving a
//! Keleusma-shaped allocator.**
//!
//! `V0_4_0_NATIVE_CODEGEN.md` calls M1 "the single highest-risk technical item in
//! V0.4.0" and says it should be executed before the rest of the IR generator is
//! built out. It specifies a minimal LLVM IR fragment using `coro.id.retcon`
//! **with a Keleusma-shaped allocator** — one whose size and alignment "surface
//! through `coro.id.retcon`'s size and align arguments".
//!
//! # What was already true, and why it is NOT this milestone
//!
//! `coroutine_feasibility.rs` establishes that every coroutine intrinsic is
//! findable and declarable through inkwell, that both returned-continuation forms
//! exist, and that the pass pipeline runs. **That is declarability, not M1.**
//!
//! `NATIVE_LOWERING_INVENTORY.md` records that a coroutine verifies, splits and
//! executes, that a named external allocator survives `coro-split`, and that the
//! frame size folds to a compile-time constant. **Those were measured with the
//! SWITCH-RESUME family**, which the same document calls "the form demonstrated
//! end to end here". They do not transfer to the returned-continuation family by
//! assumption: retcon has a different ABI, with a prototype function and a
//! continuation return, and it is retcon whose size and align arguments the
//! roadmap's allocator design depends on.
//!
//! # THE FINDING THIS FILE PRODUCED, which was not anticipated
//!
//! **Under retcon the allocator is the OVERFLOW path, not the default.** The
//! third argument to `coro.id.retcon` is a caller-provided buffer and the first
//! is its size. When the frame fits, `coro-split` uses the buffer and the
//! allocator is never called. The first version of this fragment declared a
//! 32-byte buffer, and the split output contained **no call** to the allocator at
//! all — while a naive text search for the allocator's name still succeeded,
//! because the `declare` line is always present.
//!
//! So the roadmap's sentence, that size and alignment "surface through
//! `coro.id.retcon`'s size and align arguments", is true but easy to misread:
//! that size is the INLINE BUFFER's, and the arena is what serves frames too
//! large for it. Both cases are pinned below.
//!
//! # Both layers, because one has been true while the other was false
//!
//! The `.stack_sizes` probe worked at the LLVM layer and was blocked at the
//! binding layer. Each test reports which layer answered: whether LLVM accepted
//! the construct, and whether the Rust bindings could reach it.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
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

/// **The M1 fragment**, parameterised by the caller-provided buffer size.
///
/// What makes it M1 rather than a coroutine demonstration:
///
/// * the identity intrinsic is `llvm.coro.id.retcon`, not the switch-resume
///   `llvm.coro.id`;
/// * the fifth and sixth arguments are `@kel_arena_alloc` and `@kel_arena_free`,
///   the shape Keleusma's arena would provide;
/// * the live state across the suspend is wide enough that a small buffer forces
///   the arena path.
///
/// `llvm.coro.end` returns `void` in LLVM 22, not `i1`. Published examples use
/// `i1` and fail verification with "Intrinsic has incorrect return type", naming
/// the function rather than the signature. Recorded in
/// `NATIVE_LOWERING_INVENTORY.md` before this file was written, and it cost
/// nothing here because of that.
fn m1_ir(buffer_bytes: u32) -> String {
    format!(
        r#"
declare token @llvm.coro.id.retcon(i32, i32, ptr, ptr, ptr, ptr)
declare ptr @llvm.coro.begin(token, ptr)
declare i1 @llvm.coro.suspend.retcon.i1(...)
declare void @llvm.coro.end(ptr, i1, token)

; The Keleusma-shaped allocator pair. A region reserved in the master arena,
; sized by the coroutine's static bound, and returned to it on completion.
declare ptr @kel_arena_alloc(i32)
declare void @kel_arena_free(ptr)

; The continuation prototype. Retcon's coroutine returns one of these.
declare ptr @kel_coro_prototype(ptr, i1)

declare void @kel_yield(i32)
declare void @kel_sink(i64, i64, i64, i64)

define ptr @kel_stream(ptr %buffer, i32 %n) presplitcoroutine {{
entry:
  %id = call token @llvm.coro.id.retcon(i32 {buffer_bytes}, i32 8, ptr %buffer, ptr @kel_coro_prototype, ptr @kel_arena_alloc, ptr @kel_arena_free)
  %hdl = call ptr @llvm.coro.begin(token %id, ptr null)
  br label %loop

loop:
  %v = phi i32 [ %n, %entry ], [ %next, %resume ]
  %a = phi i64 [ 1, %entry ], [ %a2, %resume ]
  %b = phi i64 [ 2, %entry ], [ %b2, %resume ]
  %c = phi i64 [ 3, %entry ], [ %c2, %resume ]
  %d = phi i64 [ 4, %entry ], [ %d2, %resume ]
  call void @kel_yield(i32 %v)
  %unwind = call i1 (...) @llvm.coro.suspend.retcon.i1()
  br i1 %unwind, label %cleanup, label %resume

resume:
  %next = add i32 %v, 1
  %a2 = add i64 %a, 11
  %b2 = add i64 %b, 22
  %c2 = add i64 %c, 33
  %d2 = add i64 %d, 44
  call void @kel_sink(i64 %a2, i64 %b2, i64 %c2, i64 %d2)
  br label %loop

cleanup:
  call void @llvm.coro.end(ptr %hdl, i1 0, token none)
  unreachable
}}
"#
    )
}

/// What one run of the fragment measured.
struct Measured {
    /// Names of the functions present after splitting.
    split_fns: Vec<String>,
    /// CALL sites of the allocator, not `declare` lines.
    alloc_calls: Vec<String>,
    /// CALL sites of the deallocator.
    free_calls: usize,
    /// The argument text of each allocator call.
    sizes: Vec<String>,
}

/// Parse, verify, split, and report — at both layers.
///
/// **The buffer must be NUL-terminated.** That is a BINDING-layer requirement:
/// `create_from_memory_range` asserts it and panics with a message about byte
/// values rather than about IR, which would read as LLVM rejecting the fragment.
fn build_split_and_measure(buffer_bytes: u32, label: &str) -> Measured {
    let ctx = Context::create();
    let ir = m1_ir(buffer_bytes);

    let mut bytes = ir.as_bytes().to_vec();
    bytes.push(0);
    let buf = MemoryBuffer::create_from_memory_range(&bytes, "m1");
    let module = match ctx.create_module_from_ir(buf) {
        Ok(m) => {
            println!("  [{label}] bindings layer : REACHED, inkwell parsed the fragment");
            m
        }
        Err(e) => {
            println!("  [{label}] bindings layer : parse returned an error");
            println!("  [{label}] LLVM layer     : REJECTED the fragment");
            panic!("M1 fragment did not parse: {}", e.to_string());
        }
    };

    match module.verify() {
        Ok(()) => println!("  [{label}] LLVM layer     : ACCEPTED, the module verifies"),
        Err(e) => {
            println!("  [{label}] LLVM layer     : REJECTED at verification");
            panic!("M1 fragment does not verify: {}", e.to_string());
        }
    }

    assert!(
        module
            .print_to_string()
            .to_string()
            .contains("llvm.coro.id.retcon"),
        "the fragment does not use the returned-continuation family, so it is not M1"
    );

    // **The control that makes the pipeline's success mean something.** If
    // `run_passes` accepted any string, the coroutine pipeline running would be
    // vacuous, so a nonexistent pass must fail first.
    let machine = host_machine();
    assert!(
        module
            .run_passes(
                "definitely-not-a-real-pass",
                &machine,
                PassBuilderOptions::create()
            )
            .is_err(),
        "run_passes accepted a nonexistent pass, so its success proves nothing"
    );

    module
        .run_passes(
            "coro-early,coro-split,coro-cleanup",
            &machine,
            PassBuilderOptions::create(),
        )
        .unwrap_or_else(|e| panic!("the coroutine pipeline did not run: {}", e.to_string()));

    let after = module.print_to_string().to_string();

    // **CALL sites, not name occurrences.** A first version searched the text for
    // the allocator's name, which the `declare` line satisfies whether or not any
    // call survives. That assertion could not fail.
    let alloc_calls: Vec<String> = after
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("@kel_arena_alloc(") && l.contains("call"))
        .map(str::to_string)
        .collect();
    let free_calls = after
        .lines()
        .filter(|l| l.contains("@kel_arena_free(") && l.contains("call"))
        .count();
    let split_fns: Vec<String> = after
        .lines()
        .filter(|l| l.starts_with("define"))
        .filter_map(|l| l.split_whitespace().find(|w| w.starts_with('@')))
        .map(|w| w.split('(').next().unwrap_or(w).to_string())
        .collect();

    // Read the ARGUMENT, not the line: the result register is `%0`, so a
    // whole-line test for `%` rejects every call site including literal ones.
    let sizes: Vec<String> = alloc_calls
        .iter()
        .filter_map(|l| {
            let open = l.find("@kel_arena_alloc(")? + "@kel_arena_alloc(".len();
            let rest = &l[open..];
            let close = rest.find(')')?;
            Some(rest[..close].trim().to_string())
        })
        .collect();

    println!(
        "  [{label}] functions after split : {}",
        split_fns.join(" ")
    );
    println!("  [{label}] allocator CALL sites  : {}", alloc_calls.len());
    for l in &alloc_calls {
        println!("  [{label}]     {l}");
    }
    println!("  [{label}] deallocator CALL sites: {free_calls}");
    println!("  [{label}] size arguments        : {sizes:?}");

    Measured {
        split_fns,
        alloc_calls,
        free_calls,
        sizes,
    }
}

/// **R4.1 M1: the milestone itself.**
///
/// A returned-continuation coroutine whose frame does NOT fit the caller's
/// buffer, so the Keleusma-shaped allocator serves it. Reports both layers.
#[test]
fn m1_retcon_coroutine_drives_a_keleusma_shaped_allocator() {
    println!("\n================ R4.1 M1: returned-continuation + arena allocator");
    let m = build_split_and_measure(8, "overflow");
    println!("  family                : RETURNED-CONTINUATION (llvm.coro.id.retcon)");
    println!("================\n");

    assert!(
        m.split_fns.len() >= 2,
        "coro-split produced {} function(s), so the coroutine was not split and \
         nothing below is evidence about a split frame",
        m.split_fns.len()
    );
    assert!(
        !m.alloc_calls.is_empty(),
        "the split output contains no CALL to the Keleusma-shaped allocator, so \
         the retcon frame is not arena-resident and M1 is not met"
    );
    assert!(
        m.free_calls > 0,
        "the frame is allocated but never returned to the arena"
    );
    // A literal size is what makes a coroutine's memory contribution statically
    // recoverable, which is the input Workstream E needs for a native bound.
    assert!(
        m.sizes
            .iter()
            .any(|a| a.starts_with("i32 ") && !a.contains('%')),
        "the allocation size is not a literal, so the frame's contribution is not \
         statically recoverable from the emitted IR: {:?}",
        m.sizes
    );
}

/// **THE MUST-NOT-FIRE HALF, and it is a finding rather than decoration.**
///
/// With a buffer large enough for the frame, `coro-split` uses the buffer and the
/// allocator is never called. Without this case the milestone above would look
/// like "retcon always allocates from the arena", which is false and would have
/// misled the frame-accounting work that depends on it.
#[test]
fn the_arena_is_the_overflow_path_not_the_default_under_retcon() {
    println!("\n================ retcon: a buffer large enough is used INSTEAD");
    let m = build_split_and_measure(256, "fits");
    println!("  family                : RETURNED-CONTINUATION (llvm.coro.id.retcon)");
    println!("================\n");

    assert!(
        m.split_fns.len() >= 2,
        "the coroutine was not split, so this says nothing about buffer use"
    );
    assert!(
        m.alloc_calls.is_empty(),
        "the allocator was called even though the frame fits the caller's buffer, \
         which contradicts the overflow-path reading recorded in this file: {:?}",
        m.alloc_calls
    );
}

/// **A DIAGNOSTIC, not an assertion.** Prints the post-split module so the host
/// protocol is READ off the actual signatures rather than assumed. What the
/// caller passes on resumption, and how release is signalled, are ABI details
/// that a wrong guess turns into a crash that looks like the ABI being unusable.
///
/// `retcon_m2.rs` was written from this output. The protocol it showed: the ramp
/// stores the frame pointer into the caller's buffer and returns a continuation;
/// the continuation takes that same buffer and an unwind flag, returns itself
/// while resuming, and on unwind calls the deallocator and returns null.
#[test]
fn dump_the_split_retcon_module() {
    let ctx = Context::create();
    let mut bytes = m1_ir(8).as_bytes().to_vec();
    bytes.push(0);
    let buf = MemoryBuffer::create_from_memory_range(&bytes, "dump");
    let module = ctx.create_module_from_ir(buf).expect("parse");
    let machine = host_machine();
    module
        .run_passes(
            "coro-early,coro-split,coro-cleanup",
            &machine,
            PassBuilderOptions::create(),
        )
        .expect("passes");
    println!(
        "\n================ SPLIT RETCON MODULE\n{}",
        module.print_to_string().to_string()
    );
    println!("================\n");
}
