//! **THE SHIPPED EXAMPLE, BUILT AND CHECKED RATHER THAN TRUSTED.**
//!
//! `examples/motor_policy/` is a worked answer to "can I let a customer change
//! this rule", not a proof that the linker works. This test keeps it honest: it
//! builds the object, links the real C host against it, runs the binary, and
//! compares the shared buffer against the virtual machine for the same inputs.
//!
//! # Why the buffer and not the printed output
//!
//! The policy's whole effect is on the shared segment. Comparing what it PRINTS
//! would pass a lowering that wrote the right number to the wrong offset, which
//! is the defect class this package has actually found twice.
//!
//! # Skipping
//!
//! It needs a C compiler. Where none is found it SKIPS LOUDLY, for the reason
//! the ahead-of-time linkage test does: a step that quietly does nothing reads
//! as a step that passed.

mod common;

use keleusma::bytecode::Value;
use keleusma::vm::{
    Vm, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};

const Q: i64 = 8;
fn to_q(v: f64) -> i64 {
    (v * f64::from(1 << Q)) as i64
}

/// Drive the policy on the reference, returning the shared buffer it leaves.
fn reference_buffer(src: &str, temps: [f64; 3], amps: f64) -> Vec<u8> {
    let m = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile");
    let n = shared_data_bytes_for(&m);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");

    let mut buf = vec![0u8; n];
    for (i, t) in temps.iter().enumerate() {
        buf[i * 8..i * 8 + 8].copy_from_slice(&to_q(*t).to_le_bytes());
    }
    buf[24..32].copy_from_slice(&to_q(amps).to_le_bytes());
    vm.call_with_shared(&mut buf, &[Value::Int(0), Value::Int(0)])
        .expect("vm run");
    buf
}

#[test]
fn the_shipped_example_builds_links_runs_and_agrees_with_the_vm() {
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    if std::process::Command::new(&cc)
        .arg("--version")
        .output()
        .is_err()
    {
        println!("  NO C COMPILER FOUND ({cc}); the shipped example cannot be built here.");
        println!("  SKIPPING LOUDLY: this test asserts nothing on this machine.");
        return;
    }

    let dir = std::env::temp_dir().join("kel-motor-policy-test");
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();

    // Emit the object and the header through the same path a user would run.
    let emit = std::process::Command::new(env!("CARGO"))
        .args([
            "run",
            "--quiet",
            "--example",
            "emit_object",
            "--",
            "examples/motor_policy/policy.kel",
            &out,
        ])
        .output()
        .expect("run emit_object");
    assert!(
        emit.status.success(),
        "emit_object failed: {}",
        String::from_utf8_lossy(&emit.stderr)
    );

    // The C host must compile against the GENERATED header. A duplicate macro
    // from two array elements sharing a slot name broke this once, and the
    // header would not compile at all.
    let bin = format!("{out}/motor_host");
    let link = std::process::Command::new(&cc)
        .args([
            "-I",
            &out,
            "-O2",
            "-o",
            &bin,
            "examples/motor_policy/host.c",
            &format!("{out}/policy.o"),
        ])
        .output()
        .expect("run cc");
    assert!(
        link.status.success(),
        "linking the C host failed: {}",
        String::from_utf8_lossy(&link.stderr)
    );

    // The host dumps the raw contract only when asked, so a person running the
    // example sees the summary and the oracle still gets the bytes.
    let run = std::process::Command::new(&bin)
        .env("KEL_DUMP_RAW", "1")
        .output()
        .expect("run host");
    assert!(run.status.success(), "the linked host did not exit cleanly");
    let printed = String::from_utf8_lossy(&run.stdout);
    assert!(
        printed.lines().count() >= 4,
        "the host printed {} lines, so it did not run its cases: {printed}",
        printed.lines().count()
    );

    // **THE ORACLE.** Same inputs, reference implementation, buffer compared.
    let src = std::fs::read_to_string("examples/motor_policy/policy.kel").expect("read policy");
    // **THE BYTES, NOT THE SUMMARY.** A wrong offset or a wrong width writes the
    // right number to the wrong place and leaves the printed line unchanged,
    // which is the defect class this package has found twice.
    let mut raw = printed
        .lines()
        .filter_map(|l| l.strip_prefix("RAW ").map(str::to_owned));
    for (temps, amps) in [
        ([20.0, 25.0, 30.0], 10.0),
        ([75.0, 25.0, 30.0], 10.0),
        ([95.0, 75.0, 30.0], 10.0),
        ([20.0, 25.0, 30.0], 250.0),
    ] {
        let want = reference_buffer(&src, temps, amps);
        let hex: String = want.iter().map(|b| format!("{b:02x}")).collect();
        assert!(
            raw.next().is_some_and(|got| got == hex),
            "the LINKED C host and the reference disagree on the shared buffer \
             for temps {temps:?} at {amps} A.\n  reference {hex}"
        );
    }
    assert!(
        raw.next().is_none(),
        "the host printed more raw lines than the oracle checked, so some case \
         went uncompared"
    );

    println!("\n{printed}");
    let _ = common::CORPUS_ROOTS;
}
