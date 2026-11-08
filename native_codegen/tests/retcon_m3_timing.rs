//! **R4.1 MILESTONE M3, the timing half.**
//!
//! `retcon_m3.rs` measured the memory half: the arena frame is `4 + 8 x live
//! words`, statically recoverable. This file measures what a resumption COSTS,
//! and it is a separate file because the two halves have different failure modes
//! and should be able to fail independently.
//!
//! # This was deferred on an untested assumption, and that was the weak part
//!
//! The earlier deferral said two sessions share this machine and a gate saturates
//! it, so a wall-clock figure would be a confident wrong number. That was honest
//! but untested, and this project had already solved the problem.
//! `keleusma-bench` calibrates per-opcode cost on exactly this host by warming
//! up, inlining repetitions to amortise counter resolution, and **taking the
//! MINIMUM across measurement passes**.
//!
//! **The minimum is the right estimator and the reason is structural**:
//! contention can only ADD time, never subtract it, so the least sample is the
//! least contaminated. A mean would fold in precisely what must be excluded. The
//! methodology here mirrors `BenchConfig` rather than inventing one, so the
//! figure is comparable with the project's other calibrated numbers.
//!
//! # What it is measured against
//!
//! **A hand-written step function doing the same work**, not an empty loop. The
//! baseline keeps the same five values in a struct, performs the same additions,
//! and calls the same two external functions — `kel_yield` and `kel_sink` — that
//! the coroutine calls. Both sides therefore pay identical callee cost, and the
//! difference is the coroutine machinery: the indirect continuation call, the
//! frame reload and the spill.
//!
//! # The noise floor is measured, not assumed
//!
//! The baseline is timed **twice, as two separate workloads**. Those two are
//! identical, so the gap between their minima is what this host can resolve. If
//! the coroutine-against-baseline difference is not comfortably larger than that
//! gap, the effect is not resolvable here and the report says so.
//!
//! # This is a REPORT with two assertions
//!
//! The figures are printed rather than asserted, because they are properties of
//! the host rather than of the tree. The two assertions are the properties that
//! would make the measurement meaningless: that each pass is long enough to dwarf
//! timer granularity, and that no workload was optimised away.
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use std::path::PathBuf;
use std::process::Command;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kel_retcon_m3t_{tag}"));
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
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine")
}

/// The same coroutine M1 and M2 use, with an 8-byte buffer so the frame is
/// arena-resident. Spawn happens once per pass; the timed loop is resumption.
const TIMING_IR: &str = r#"
declare token @llvm.coro.id.retcon(i32, i32, ptr, ptr, ptr, ptr)
declare ptr @llvm.coro.begin(token, ptr)
declare i1 @llvm.coro.suspend.retcon.i1(...)
declare void @llvm.coro.end(ptr, i1, token)

declare ptr @kel_arena_alloc(i32)
declare void @kel_arena_free(ptr)
declare ptr @kel_coro_prototype(ptr, i1)
declare void @kel_yield(i32)
declare void @kel_sink(i64, i64, i64, i64)

define ptr @kel_stream(ptr %buffer, i32 %n) presplitcoroutine {
entry:
  %id = call token @llvm.coro.id.retcon(i32 8, i32 8, ptr %buffer, ptr @kel_coro_prototype, ptr @kel_arena_alloc, ptr @kel_arena_free)
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
}
"#;

/// The timing host.
///
/// `kel_yield` and `kel_sink` are `noinline` and accumulate into globals that are
/// printed, so neither workload can be optimised away and both pay the same
/// callee cost. The baseline `step` is the coroutine's body written directly.
const HOST_C: &str = r#"
#include <stdio.h>
#include <stdlib.h>
#include <stdbool.h>
#include <time.h>

#define REPS    2000000
#define WARMUP  3
#define PASSES  15

static long long yield_sum = 0;
static long long sink_sum  = 0;

__attribute__((noinline)) void kel_yield(int v) { yield_sum += v; }
__attribute__((noinline)) void kel_sink(long long a, long long b, long long c, long long d) {
    sink_sum += a + b + c + d;
}
void *kel_arena_alloc(int n) { return malloc((size_t)n); }
void  kel_arena_free(void *p) { free(p); }

typedef void *(*cont_t)(void *, bool);
void *kel_stream(void *buffer, int n);

/* The BASELINE: the coroutine's own body, written as a direct call. Same state,
   same arithmetic, same two external calls. Not an empty loop. */
typedef struct { long long a, b, c, d; int v; } state_t;
__attribute__((noinline)) static void step(state_t *s) {
    kel_yield(s->v);
    s->v += 1;
    s->a += 11; s->b += 22; s->c += 33; s->d += 44;
    kel_sink(s->a, s->b, s->c, s->d);
}

/* **The clock's granularity, MEASURED rather than assumed.** The first run
   produced pass times that were all multiples of 1000 ns, which is evidence the
   tick is a microsecond here and not the nanosecond the API suggests. Reported
   so the per-operation figure can be judged against it. */
static double probe_granularity_ns(void);

static double now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec * 1e9 + (double)ts.tv_nsec;
}

static double probe_granularity_ns(void) {
    double smallest = 1e18;
    for (int i = 0; i < 200000; i++) {
        double a = now_ns();
        double b = now_ns();
        double d = b - a;
        if (d > 0.0 && d < smallest) smallest = d;
    }
    return smallest;
}

static double time_coroutine(void) {
    static _Alignas(16) unsigned char buffer[1024];
    double t0 = now_ns();
    cont_t k = (cont_t)kel_stream(buffer, 1);
    for (int i = 0; i < REPS; i++) k = (cont_t)k(buffer, false);
    k(buffer, true);
    return now_ns() - t0;
}

static double time_baseline(void) {
    state_t s = { 1, 2, 3, 4, 1 };
    double t0 = now_ns();
    for (int i = 0; i < REPS; i++) step(&s);
    return now_ns() - t0;
}

static int cmp_double(const void *x, const void *y) {
    double a = *(const double *)x, b = *(const double *)y;
    return (a > b) - (a < b);
}

static void run(const char *label, double (*f)(void)) {
    for (int i = 0; i < WARMUP; i++) (void)f();
    double s[PASSES];
    for (int i = 0; i < PASSES; i++) s[i] = f();
    qsort(s, PASSES, sizeof(double), cmp_double);
    /* Minimum is the estimate, as keleusma-bench does. Median and maximum are
       printed so the spread is visible rather than hidden. */
    printf("%s min=%.1f med=%.1f max=%.1f ns_per_op=%.4f\n",
           label, s[0], s[PASSES / 2], s[PASSES - 1], s[0] / (double)REPS);
}

int main(void) {
    printf("reps=%d warmup=%d passes=%d\n", REPS, WARMUP, PASSES);
    printf("clock_tick_ns=%.1f\n", probe_granularity_ns());
    run("coroutine", time_coroutine);
    run("baselineA", time_baseline);
    run("baselineB", time_baseline);
    printf("checksums yield=%lld sink=%lld\n", yield_sum, sink_sum);
    return 0;
}
"#;

/// **WHAT DOES A RESUMPTION COST, against a direct call doing the same work?**
#[test]
fn m3_timing_a_resumption_against_an_equivalent_direct_call() {
    let Some(cc) = c_compiler() else {
        println!(
            "\n\x1b[1;33mSKIPPED: no C compiler found, so M3's timing half was NOT \
             exercised. This is not evidence about the cost.\x1b[0m\n"
        );
        return;
    };

    let dir = scratch("timing");
    let ctx = Context::create();
    let mut bytes = TIMING_IR.as_bytes().to_vec();
    bytes.push(0);
    let buf = MemoryBuffer::create_from_memory_range(&bytes, "m3t");
    let module = ctx
        .create_module_from_ir(buf)
        .unwrap_or_else(|e| panic!("fragment did not parse: {}", e.to_string()));
    module
        .verify()
        .unwrap_or_else(|e| panic!("fragment does not verify: {}", e.to_string()));

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
        .unwrap_or_else(|e| panic!("coroutine pipeline failed: {}", e.to_string()));

    let obj = dir.join("coro.o");
    machine
        .write_to_file(&module, FileType::Object, &obj)
        .unwrap_or_else(|e| panic!("object emission failed: {}", e.to_string()));

    let host = dir.join("host.c");
    std::fs::write(&host, HOST_C).expect("write host");
    let exe = dir.join("m3t");
    let link = Command::new(&cc)
        .arg(&host)
        .arg(&obj)
        .arg("-O2")
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("invoke the C compiler");
    assert!(
        link.status.success(),
        "LINK FAILED\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    // **The timed region is INSIDE the linked binary.** Emission, linking and
    // process startup are not measured; they dwarf a resumption by orders of
    // magnitude and would swamp the quantity entirely.
    let run = Command::new(&exe).output().expect("run the linked binary");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();

    println!("\n================ R4.1 M3 (timing half): resumption cost");
    println!("  estimator : MINIMUM across passes, as `keleusma-bench` uses.");
    println!("              Contention only ADDS time, so the least sample is the");
    println!("              least contaminated. Median and max show the spread.");
    println!("  baseline  : a direct call to the coroutine's own body, same state,");
    println!("              same arithmetic, same two external calls. Not an empty loop.");
    println!("  clock     : CLOCK_MONOTONIC. Its actual tick is MEASURED below,");
    println!("              because the first run produced only multiples of 1000 ns.");
    for line in stdout.lines() {
        println!("  {line}");
    }

    let field = |label: &str, key: &str| -> Option<f64> {
        stdout
            .lines()
            .find(|l| l.starts_with(label))?
            .split_whitespace()
            .find_map(|w| w.strip_prefix(key)?.parse::<f64>().ok())
    };

    let (co, a, b) = (
        field("coroutine", "min=").expect("coroutine min"),
        field("baselineA", "min=").expect("baselineA min"),
        field("baselineB", "min=").expect("baselineB min"),
    );

    // **The noise floor, MEASURED.** Two identical baseline workloads; the gap
    // between their minima is what this host can resolve.
    let floor = (a - b).abs();
    let effect = co - (a + b) / 2.0;
    println!("  ---");
    println!("  noise floor (identical baselines differ by) : {floor:.1} ns per pass");
    println!("  coroutine minus baseline                    : {effect:.1} ns per pass");
    if effect.abs() > floor * 4.0 {
        println!(
            "  RESOLVABLE: the difference is more than 4x the floor, so a resumption \
             costs about {:.4} ns more than an equivalent direct call on this host.",
            effect / 2_000_000.0
        );
    } else {
        println!(
            "  NOT RESOLVABLE on this host: the difference is within 4x the measured \
             floor, so no per-resumption cost is claimed."
        );
    }
    println!("================\n");

    // The two properties that would make the figures meaningless.
    assert!(
        co > 1.0e6 && a > 1.0e6 && b > 1.0e6,
        "a pass took under a millisecond, which is too close to timer granularity \
         to divide by {} repetitions: coroutine={co} a={a} b={b}",
        2_000_000
    );
    assert!(
        stdout.contains("checksums yield=") && !stdout.contains("checksums yield=0 sink=0"),
        "the checksums are zero, so a workload was optimised away and the timings \
         measure nothing:\n{stdout}"
    );
}
