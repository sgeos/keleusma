//! **R4.1 MILESTONE M2: a returned-continuation coroutine EXECUTING across a
//! linked boundary.**
//!
//! M1 (`retcon_m1.rs`) established that a `coro.id.retcon` fragment verifies,
//! splits, and calls a Keleusma-shaped allocator with a compile-time literal
//! size. **Nothing executed it.** M2 lowers that fragment to a real object file,
//! links it against a C host with the system linker, and drives spawn, resume and
//! release as a separate process.
//!
//! # The host protocol was READ off the split IR, not assumed
//!
//! A wrong guess about the returned-continuation ABI produces a crash that looks
//! like the ABI being unusable, so `dump_the_split_retcon_module` in `retcon_m1.rs`
//! printed the post-split module and the protocol below is what it showed:
//!
//! * `ptr @kel_stream(ptr %buffer, i32 %n)` is the ramp. It allocates the frame,
//!   **stores the frame pointer into `*buffer`**, produces the first value, and
//!   returns a continuation pointer.
//! * the continuation is `ptr (ptr %buffer, i1 %unwind)`. It loads the frame from
//!   `*buffer`. With `%unwind == 0` it resumes, produces the next value, and
//!   **returns itself**. With `%unwind == 1` it branches to `CoroEnd`, calls
//!   `@kel_arena_free`, and **returns null**.
//!
//! So release is signalled by the second argument, and the caller supplies the
//! same buffer every time. That is measured, not documentation habit.
//!
//! # Why the ahead-of-time path and not the JIT
//!
//! `aot_linkage.rs` says it: the JIT "never produces an object file, never goes
//! through a linker, and never crosses a real C calling convention". M1 already
//! inspected IR. Running through the JIT would add nothing M1 did not have.
//!
//! # What this does NOT establish
//!
//! **M3 is untouched.** Nothing here measures per-coroutine overhead.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use std::path::PathBuf;
use std::process::Command;

/// Per-test scratch directory, so concurrent tests do not link over each other.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kel_retcon_m2_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn c_compiler() -> Option<String> {
    for cc in ["cc", "clang", "gcc"] {
        if Command::new(cc)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cc.to_string());
        }
    }
    None
}

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
            // Position-independent, matching `aot_linkage.rs`: a modern platform
            // links PIE by default and a non-PIC object fails at link time.
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine")
}

/// The same fragment M1 uses, parameterised by the caller-provided buffer size.
///
/// Kept as its own copy rather than shared with `retcon_m1.rs` because integration
/// tests are separate crates; the two are compared by the assertions below rather
/// than by a shared constant.
fn m2_ir(buffer_bytes: u32) -> String {
    format!(
        r#"
declare token @llvm.coro.id.retcon(i32, i32, ptr, ptr, ptr, ptr)
declare ptr @llvm.coro.begin(token, ptr)
declare i1 @llvm.coro.suspend.retcon.i1(...)
declare void @llvm.coro.end(ptr, i1, token)

declare ptr @kel_arena_alloc(i32)
declare void @kel_arena_free(ptr)
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

/// The C host. Counts allocator traffic and records every produced value, so the
/// assertions are about OBSERVED BEHAVIOUR rather than about the process exiting.
const HOST_C: &str = r#"
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>

static int alloc_calls = 0;
static int free_calls  = 0;
static int produced[64];
static int n_produced  = 0;

void *kel_arena_alloc(int n) { alloc_calls++; return malloc((size_t)n); }
void  kel_arena_free(void *p) { free_calls++; free(p); }
void  kel_yield(int v) { if (n_produced < 64) produced[n_produced++] = v; }
void  kel_sink(long long a, long long b, long long c, long long d) {
    (void)a; (void)b; (void)c; (void)d;
}

typedef void *(*cont_t)(void *, bool);
void *kel_stream(void *buffer, int n);

int main(void) {
    static _Alignas(16) unsigned char buffer[1024];

    cont_t k = (cont_t)kel_stream(buffer, 7);
    if (!k) { printf("SPAWN_RETURNED_NULL\n"); return 2; }

    k = (cont_t)k(buffer, false);
    if (!k) { printf("FIRST_RESUME_RETURNED_NULL\n"); return 3; }

    k = (cont_t)k(buffer, false);
    if (!k) { printf("SECOND_RESUME_RETURNED_NULL\n"); return 4; }

    void *end = k(buffer, true);

    printf("produced:");
    for (int i = 0; i < n_produced; i++) printf(" %d", produced[i]);
    printf("\n");
    printf("alloc=%d free=%d released_null=%d\n",
           alloc_calls, free_calls, end == NULL ? 1 : 0);
    return 0;
}
"#;

struct RunResult {
    stdout: String,
    status: i32,
}

/// Emit, link and run. Returns what the process printed.
fn build_link_and_run(buffer_bytes: u32, tag: &str, cc: &str) -> RunResult {
    let dir = scratch(tag);
    let ctx = Context::create();

    let mut bytes = m2_ir(buffer_bytes).as_bytes().to_vec();
    bytes.push(0);
    let buf = MemoryBuffer::create_from_memory_range(&bytes, "m2");
    let module = ctx
        .create_module_from_ir(buf)
        .unwrap_or_else(|e| panic!("[{tag}] the fragment did not parse: {}", e.to_string()));
    module
        .verify()
        .unwrap_or_else(|e| panic!("[{tag}] the fragment does not verify: {}", e.to_string()));

    let machine = host_machine();
    // The control that keeps the pipeline's success from being vacuous.
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
        .unwrap_or_else(|e| panic!("[{tag}] the coroutine pipeline failed: {}", e.to_string()));

    let obj = dir.join("coro.o");
    machine
        .write_to_file(&module, FileType::Object, &obj)
        .unwrap_or_else(|e| panic!("[{tag}] object emission failed: {}", e.to_string()));

    let host = dir.join("host.c");
    std::fs::write(&host, HOST_C).expect("write host");
    let exe = dir.join("m2");

    let link = Command::new(cc)
        .arg(&host)
        .arg(&obj)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("invoke the C compiler");
    assert!(
        link.status.success(),
        "[{tag}] LINK FAILED\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&link.stdout),
        String::from_utf8_lossy(&link.stderr)
    );

    let run = Command::new(&exe).output().expect("run the linked binary");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    let status = run.status.code().unwrap_or(-1);
    println!("  [{tag}] exit={status}");
    for line in stdout.lines() {
        println!("  [{tag}] {line}");
    }
    if !run.stderr.is_empty() {
        println!("  [{tag}] stderr: {}", String::from_utf8_lossy(&run.stderr));
    }
    RunResult { stdout, status }
}

/// **R4.1 M2: the coroutine runs, resumes more than once, and releases.**
///
/// Buffer of 8 bytes, so the frame overflows it and the arena serves it.
#[test]
fn m2_a_retcon_coroutine_spawns_resumes_and_releases_when_linked() {
    let Some(cc) = c_compiler() else {
        println!(
            "\n\x1b[1;33mSKIPPED: no C compiler found, so M2 was NOT exercised. \
             This is not evidence that the milestone holds.\x1b[0m\n"
        );
        return;
    };

    println!("\n================ R4.1 M2: linked retcon coroutine (arena path)");
    let r = build_link_and_run(8, "overflow", &cc);
    println!("================\n");

    assert_eq!(r.status, 0, "the linked binary did not exit cleanly");

    // **Values observed across MORE THAN ONE resumption.** A process that merely
    // ran to completion would say nothing about the coroutine resuming.
    assert!(
        r.stdout.contains("produced: 7 8 9"),
        "expected three produced values across two resumptions, got:\n{}",
        r.stdout
    );
    // The arena is used here because the frame overflows the 8-byte buffer, and
    // the region is returned on release.
    assert!(
        r.stdout.contains("alloc=1 free=1 released_null=1"),
        "expected one arena allocation, one release, and a null continuation after \
         unwind, got:\n{}",
        r.stdout
    );
}

/// **THE MUST-NOT-FIRE HALF, at RUN TIME this time.**
///
/// M1 showed the two buffer configurations differ in the emitted IR. This shows
/// they differ in behaviour: with a buffer large enough, the coroutine still
/// spawns, resumes and releases, and the arena is never touched.
#[test]
fn m2_a_buffer_large_enough_never_reaches_the_arena_at_run_time() {
    let Some(cc) = c_compiler() else {
        println!(
            "\n\x1b[1;33mSKIPPED: no C compiler found, so the run-time buffer \
             control was NOT exercised.\x1b[0m\n"
        );
        return;
    };

    println!("\n================ R4.1 M2: linked retcon coroutine (buffer path)");
    let r = build_link_and_run(256, "fits", &cc);
    println!("================\n");

    assert_eq!(r.status, 0, "the linked binary did not exit cleanly");
    assert!(
        r.stdout.contains("produced: 7 8 9"),
        "the coroutine must still resume when its frame fits the buffer, got:\n{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("alloc=0 free=0"),
        "the arena was touched even though the frame fits the caller's buffer, \
         which contradicts the overflow-path reading M1 recorded, got:\n{}",
        r.stdout
    );
}
