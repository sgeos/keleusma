//! **ORDER 4, WORKSTREAM F: A HOST NATIVE CAN FAIL SOFTLY IN NATIVE CODE.**
//!
//! The virtual machine reifies every native call. A host function that returns
//! `VmError::NativeErrorCode` gives the program its code through a
//! `host_error` arm, or reaches the default `Trap(HostError)` when the program
//! wrote none. Before Order 4 a native coroutine called `i64 f(i64...)`, which
//! cannot report a failure, so its flag was a constant zero and neither the arm
//! nor the trap was reachable.
//!
//! A coroutine's `HostContracts::fallible_natives` now names natives whose
//! symbol returns `{i64 value, i64 failed}`. This file drives the same
//! programs through the virtual machine and through native code with a host
//! that fails on one input, and requires the same outcome. The last test links
//! a C host against an exported library, so the two-word return is checked
//! against the C structure the header declares and not only against Rust.
//!
//! # What this does not establish
//!
//! The virtual machine also lets a host return a message without a code, which
//! reifies as code zero. The native interface carries codes only. A fallible
//! native returning a flat body is refused. Only the AAPCS64 and System V
//! x86-64 return conventions are admitted, and only the first is executed here.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use keleusma::bytecode::{TrapKind, Value};
use keleusma::vm::{Vm, VmError, VmState};
use keleusma_native::coroutine::{self, HostContracts};
use keleusma_native::region;
use std::collections::BTreeSet;

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    let triple = TargetMachine::get_default_triple();
    machine_for(&triple)
}

fn machine_for(triple: &TargetTriple) -> TargetMachine {
    Target::from_triple(triple)
        .unwrap()
        .create_target_machine(
            triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    status: u64,
    value: i64,
}

/// The C structure a fallible native returns.
#[repr(C)]
struct NativeResult {
    value: i64,
    failed: i64,
}

type Start = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
type Resume = unsafe extern "C" fn(*mut u8, i64) -> Outcome;

/// The code the host reports on its failing input.
const CODE: i64 = 42;

/// Fails with [`CODE`] on zero and doubles anything else.
extern "C" fn risky(a: i64) -> NativeResult {
    if a == 0 {
        NativeResult {
            value: CODE,
            failed: 1,
        }
    } else {
        NativeResult {
            value: a * 2,
            failed: 0,
        }
    }
}

/// The virtual machine's counterpart of [`risky`].
fn register(vm: &mut Vm<'_, '_>) {
    vm.register_native("host::risky", |args| match args[0] {
        Value::Int(0) => Err(VmError::NativeErrorCode {
            code: CODE,
            message: String::from("refused"),
        }),
        Value::Int(a) => Ok(Value::Int(a * 2)),
        ref other => panic!("risky received {other:?}"),
    });
}

/// How a run ended, in terms both executions can express.
#[derive(Debug, PartialEq, Eq)]
enum End {
    Completed(i64),
    Yielded,
    Trapped(u16),
}

fn reference(src: &str, first: i64, replies: &[i64]) -> (Vec<i64>, End) {
    let program = common::build(src);
    let arena = keleusma_arena::Arena::with_capacity(1 << 16);
    let mut vm = Vm::new(program, &arena).unwrap();
    register(&mut vm);
    let mut yields = Vec::new();
    let mut state = vm.call(&[Value::Int(first)]);
    let mut replies = replies.iter();
    let mut last = 0;
    loop {
        match state {
            Ok(VmState::Yielded(Value::Int(v))) => {
                yields.push(v);
                let Some(&r) = replies.next() else {
                    return (yields, End::Yielded);
                };
                last = r;
                state = vm.resume(Value::Int(r));
            }
            Ok(VmState::Reset) => state = vm.resume(Value::Int(last)),
            Ok(VmState::Finished(Value::Int(v))) => return (yields, End::Completed(v)),
            Err(VmError::NativeErrorCode { code, .. }) => {
                assert_eq!(code, CODE, "the host's own code surfaces");
                return (yields, End::Trapped(TrapKind::HostError.code()));
            }
            other => panic!("{src}: {other:?}"),
        }
    }
}

fn native(src: &str, first: i64, replies: &[i64], optimise: bool) -> (Vec<i64>, End) {
    let program = common::build(src);
    let tm = machine();
    let contracts = HostContracts {
        fallible_natives: BTreeSet::from([0]),
        ..HostContracts::default()
    };
    let bound = coroutine::minimum_frame_bytes(&program, &tm, &contracts).unwrap();
    let ctx = Context::create();
    let module = coroutine::lower_with_contracts(&ctx, &program, &tm, bound, &contracts).unwrap();
    if optimise {
        common::force_optimize(&module);
    }
    let symbol = keleusma_native::native_symbol("host::risky");
    let declaration = module.get_function(&symbol).expect("the native is called");
    assert_eq!(
        declaration
            .get_type()
            .get_return_type()
            .map(|t| t.print_to_string().to_string()),
        Some(String::from("{ i64, i64 }")),
        "a fallible native returns its value and its failure word"
    );
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    engine.add_global_mapping(&declaration, risky as *const () as usize);
    let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
    let (start, resume) = unsafe {
        (
            engine
                .get_function::<Start>(&format!("{prefix}_start"))
                .unwrap(),
            engine
                .get_function::<Resume>(&format!("{prefix}_resume"))
                .unwrap(),
        )
    };
    let words = |bytes: usize| vec![0u64; bytes.div_ceil(8).max(1)];
    let mut slot = words(coroutine::slot_bytes(bound).unwrap() as usize);
    let mut shared = words(program.shared_data_bytes as usize);
    let mut private = words(
        keleusma::vm::required_persistent_capacity_for(&program)
            + region::persistent_supplement_bytes(&program) as usize,
    );
    let mut bodies = words(region::host_arena_supplement_bytes(&program) as usize);
    common::install_private_init_bytes(&program, unsafe {
        std::slice::from_raw_parts_mut(private.as_mut_ptr().cast::<u8>(), private.len() * 8)
    });
    let slot = slot.as_mut_ptr().cast::<u8>();
    let mut outcome = unsafe {
        start.call(
            first,
            shared.as_mut_ptr().cast(),
            private.as_mut_ptr().cast(),
            bodies.as_mut_ptr().cast(),
            slot,
        )
    };
    let mut yields = Vec::new();
    let mut replies = replies.iter();
    loop {
        match outcome.status {
            1 => {
                yields.push(outcome.value);
                let Some(&r) = replies.next() else {
                    return (yields, End::Yielded);
                };
                outcome = unsafe { resume.call(slot, r) };
            }
            2 => return (yields, End::Completed(outcome.value)),
            3 => return (yields, End::Trapped(outcome.value as u16)),
            other => panic!("{src}: status {other}"),
        }
    }
}

const HANDLED: &str = "use host::risky(Word) -> Word\n\
    yield main(x: Word) -> Word { let a = yield x; host::risky(a) { ok(v) => v, host_error(c) => c + 1000 } }";
const BARE: &str = "use host::risky(Word) -> Word\n\
    yield main(x: Word) -> Word { let a = yield x; host::risky(a) + 1 }";
const IN_A_CALLEE: &str = "use host::risky(Word) -> Word\n\
    fn safe(a: Word) -> Word { host::risky(a) { ok(v) => v, host_error(c) => 0 - c } }\n\
    loop main(x: Word) -> Word { let a = yield safe(x); yield a + 1 }";
const BARE_IN_A_CALLEE: &str = "use host::risky(Word) -> Word\n\
    fn raw(a: Word) -> Word { host::risky(a) * 3 }\n\
    loop main(x: Word) -> Word { let a = yield raw(x); yield a + 1 }";

#[test]
fn a_soft_host_failure_reaches_the_same_outcome_as_in_the_vm() {
    let cases: &[(&str, &str, i64, &[i64])] = &[
        ("handled, failing", HANDLED, 7, &[0]),
        ("handled, succeeding", HANDLED, 7, &[5]),
        ("bare, failing", BARE, 7, &[0]),
        ("bare, succeeding", BARE, 7, &[5]),
        ("handled in a callee, failing", IN_A_CALLEE, 3, &[9, 0, 4]),
        ("bare in a callee, failing", BARE_IN_A_CALLEE, 3, &[9, 0, 4]),
        (
            "bare in a callee, succeeding",
            BARE_IN_A_CALLEE,
            3,
            &[9, 2, 4],
        ),
    ];
    let mut failures_reached = 0;
    for &(what, src, first, replies) in cases {
        let expected = reference(src, first, replies);
        if matches!(expected.1, End::Trapped(_)) || expected.0.iter().any(|&v| v < 0) {
            failures_reached += 1;
        }
        for optimise in [false, true] {
            assert_eq!(
                native(src, first, replies, optimise),
                expected,
                "{what} (optimised {optimise})"
            );
        }
        eprintln!("  {what:<32} {expected:?}");
    }
    // The handled failure completes with the code plus 1000, and the callee's
    // handled failure yields minus the code, so both handled forms are counted
    // through their observable value as well.
    assert_eq!(
        reference(HANDLED, 7, &[0]).1,
        End::Completed(CODE + 1000),
        "the handled subject must reach its host_error arm"
    );
    assert!(
        failures_reached >= 3,
        "too few subjects reached a failure for this to test the failure path"
    );
}

#[test]
fn malformed_fallible_contracts_are_refused() {
    let tm = machine();
    let program = common::build(HANDLED);
    let out_of_range = HostContracts {
        fallible_natives: BTreeSet::from([5]),
        ..HostContracts::default()
    };
    let error =
        coroutine::lower_with_contracts(&Context::create(), &program, &tm, 4096, &out_of_range)
            .unwrap_err();
    assert!(error.to_string().contains("native table"), "{error}");

    let mut flat = common::build(
        "use host::pair\nloop main(t: Word) -> Word { let p: (Word, Word) = host::pair(t); yield p.0 }",
    );
    flat.native_return_shapes[0] = keleusma::bytecode::WireShape::Flat { kind: 0, size: 16 };
    let both = HostContracts {
        native_body_returns: [(0, coroutine::NativeBodyReturn::Snapshot)].into(),
        fallible_natives: BTreeSet::from([0]),
        ..HostContracts::default()
    };
    let error =
        coroutine::lower_with_contracts(&Context::create(), &flat, &tm, 4096, &both).unwrap_err();
    assert!(error.to_string().contains("flat body"), "{error}");
}

/// The Microsoft x64 convention returns a two-word structure through a hidden
/// pointer, so the declaration would not match the host. The lowering must
/// refuse rather than emit it.
#[test]
fn a_target_whose_two_word_return_differs_from_c_is_refused() {
    Target::initialize_all(&InitializationConfig::default());
    let windows = machine_for(&TargetTriple::create("x86_64-pc-windows-msvc"));
    let program = common::build(HANDLED);
    let contracts = HostContracts {
        fallible_natives: BTreeSet::from([0]),
        ..HostContracts::default()
    };
    let error =
        coroutine::lower_with_contracts(&Context::create(), &program, &windows, 4096, &contracts)
            .unwrap_err();
    assert!(error.to_string().contains("two words"), "{error}");
    // MUST-NOT-FIRE: the same target without a fallible native still lowers.
    coroutine::lower(&Context::create(), &program, &windows, 4096)
        .expect("an infallible native lowers on the same target");
}

/// The exported library declares the native with the C structure, and a C host
/// defining it that way agrees with the virtual machine on both inputs.
#[test]
fn a_c_host_defines_a_fallible_native_from_the_generated_header() {
    use std::process::Command;
    let tm = machine();
    let program = common::build(BARE);
    let contracts = HostContracts {
        fallible_natives: BTreeSet::from([0]),
        ..HostContracts::default()
    };
    let ctx = Context::create();
    let library =
        keleusma_native::linkage::build(&ctx, &program, &tm, "kel_risky", &contracts).unwrap();
    let native = library
        .natives
        .iter()
        .find(|n| n.symbol == keleusma_native::native_symbol("host::risky"))
        .expect("the import is listed");
    assert!(native.fallible, "the import is recorded as fallible");
    let header = library.c_header();
    assert!(header.contains("kel_native_result"), "{header}");
    assert!(
        library.rust_bindings().contains("NativeResult"),
        "the Rust bindings render the two-word result"
    );

    let dir = std::path::PathBuf::from("../tmp")
        .join(format!("native-host-failure-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    library.write_object(&tm, &dir.join("kel_risky.o")).unwrap();
    std::fs::write(dir.join("kel_risky.h"), &header).unwrap();
    let symbol = &native.symbol;
    let host = format!(
        r#"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "kel_risky.h"
kel_native_result {symbol}(int64_t a) {{
    kel_native_result r;
    if (a == 0) {{ r.value = {CODE}; r.failed = 1; }} else {{ r.value = a * 2; r.failed = 0; }}
    return r;
}}
static _Alignas(8) unsigned char slot[KEL_RISKY_SLOT_BYTES];
static _Alignas(8) unsigned char shared[KEL_RISKY_SHARED_BYTES + 8];
static _Alignas(8) unsigned char priv[KEL_RISKY_PRIVATE_BYTES + 8];
static _Alignas(8) unsigned char region[KEL_RISKY_REGION_BYTES + 8];
int main(int argc, char **argv) {{
    for (int i = 1; i < argc; i++) {{
        memset(priv, 0, sizeof priv);
        struct kel_risky_outcome o = kel_risky_start(7, shared, priv, region, slot);
        if (o.status != 1 || o.value != 7) return 2;
        o = kel_risky_resume(slot, atoll(argv[i]));
        printf("%llu %lld\n", (unsigned long long)o.status, (long long)o.value);
        kel_risky_release(slot);
    }}
    return 0;
}}
"#
    );
    std::fs::write(dir.join("host.c"), host).unwrap();
    let exe = dir.join("host");
    let built = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(&dir)
        .arg(dir.join("host.c"))
        .arg(dir.join("kel_risky.o"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let run = Command::new(&exe).args(["0", "5"]).output().unwrap();
    assert!(run.status.success(), "{run:?}");
    let lines: Vec<String> = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(String::from)
        .collect();
    let expect = |reply: i64| match reference(BARE, 7, &[reply]).1 {
        End::Completed(v) => format!("2 {v}"),
        End::Trapped(code) => format!("3 {code}"),
        End::Yielded => panic!("the reference kept yielding"),
    };
    assert_eq!(lines, [expect(0), expect(5)]);
    assert_eq!(lines[0], format!("3 {}", TrapKind::HostError.code()));
    std::fs::remove_dir_all(&dir).unwrap();
}
