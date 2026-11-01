//! `hosted::seeded_shared` seeds `interp.kel` as `Interpreter::run` does.
//!
//! The function exists so that another executor of `interp.kel`, a native build
//! of it in particular, can be given the same seeded buffer and the same
//! command protocol as the Rust driver. The test plays that executor with a
//! plain virtual machine that knows only the protocol constants, and requires
//! the same outcome as the driver.
//!
//! # What these tests do NOT establish
//!
//! They drive a program to its end. They do not exercise `protocol::RESUME`,
//! which reads its input from two slots the seeding does not write.
#![cfg(feature = "self-host")]

use keleusma::bytecode::Module;
use keleusma::hosted::{
    HostedValue, Interpreter, Outcome, admit, interpreter_module, interpreter_persistent_bytes,
    protocol, seeded_shared,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{Arena, Value};

const MAX_TICKS: u64 = 100_000;

fn compiled(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse")).expect("compile")
}

fn arena_for(interp: &Module) -> Arena {
    let need = interpreter_persistent_bytes(interp);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    arena
}

/// One command, as an outside executor sends it. The first enters `loop main`
/// by a call and every later one by a resume, and a reset re-enters the body
/// with the same command, which is what `Interpreter` does privately.
fn send(vm: &mut Vm<'_, '_>, shared: &mut [u8], cmd: i64) -> i64 {
    for _ in 0..2 {
        let state = match vm.resume_with_shared(shared, Value::Int(cmd)) {
            Err(VmError::NotSuspended) => vm.call_with_shared(shared, &[Value::Int(cmd)]),
            other => other,
        }
        .expect("interp.kel runs");
        match state {
            VmState::Yielded(Value::Int(status)) => return status,
            VmState::Reset => continue,
            other => panic!("interp.kel produced {other:?}"),
        }
    }
    panic!("interp.kel reset twice without a status")
}

#[test]
fn a_seeded_buffer_runs_to_the_driver_outcome() {
    let interp = interpreter_module().expect("interp.kel compiles");
    let cases: [(&str, Vec<HostedValue>); 3] = [
        (
            "fn main(a: Word, b: Word) -> Word { a * b + 1 }",
            vec![HostedValue::Word(6), HostedValue::Word(7)],
        ),
        (
            "fn f(n: Word) -> Word { if n > 2 { n * 3 } else { n + 1 } } fn main(a: Word) -> Word { f(a) + f(a + 1) * 2 }",
            vec![HostedValue::Word(3)],
        ),
        ("fn main() -> Word { 7 / 0 }", vec![]),
    ];
    let mut finished = 0usize;
    for (source, args) in cases {
        let adm = admit(&compiled(source)).expect("admitted");

        let arena = arena_for(&interp);
        let mut driver = Interpreter::new(interp.clone(), &arena).expect("interpreter");
        let expected = driver.run(&adm, &args, MAX_TICKS).expect("driver run");

        let mut shared = seeded_shared(&interp, &adm, &args).expect("seeded");
        let arena = arena_for(&interp);
        let mut vm = Vm::new(interp.clone(), &arena).expect("interp.kel verifies");
        assert_eq!(shared.len(), vm.shared_data_bytes(), "{source}");
        let mut status = send(&mut vm, &mut shared, protocol::START);
        let mut ticks = 0u64;
        while status == protocol::RUNNING && ticks < MAX_TICKS {
            status = send(&mut vm, &mut shared, protocol::CONTINUE);
            ticks += 1;
        }
        let want = match expected.outcome {
            Outcome::Finished(_) => protocol::FINISHED,
            Outcome::Trapped { .. } => protocol::TRAPPED,
            ref other => panic!("{source}: unexpected driver outcome {other:?}"),
        };
        assert_eq!(status, want, "{source}");
        assert_eq!(ticks, expected.ticks, "{source}");
        if status == protocol::FINISHED {
            finished += 1;
        }
    }
    assert_eq!(finished, 2, "the corpus lost a finishing case");
}

#[test]
fn seeding_refuses_more_arguments_than_the_interpreter_holds() {
    let interp = interpreter_module().expect("interp.kel compiles");
    let adm = admit(&compiled("fn main() -> Word { 0 }")).expect("admitted");
    let args = vec![HostedValue::Word(0); keleusma::hosted::caps::ARGS + 1];
    assert!(seeded_shared(&interp, &adm, &args).is_err());
}

#[test]
fn seeding_refuses_a_module_that_is_not_the_interpreter() {
    let other = compiled("fn main() -> Word { 0 }");
    let adm = admit(&other).expect("admitted");
    assert!(seeded_shared(&other, &adm, &[]).is_err());
}
