//! **OPERATOR QUESTION 6: THE SEEDED `interp.kel` DIFFERENTIAL.**
//!
//! `corpus_differential.rs` exempts `interp.kel`, because the virtual machine
//! refuses it with empty `io` tables. V0.2.X made the seeding public in
//! absorption 116, as `keleusma::hosted::seeded_shared` and the
//! `keleusma::hosted::protocol` constants, so the interpreter can now be given a
//! loaded program by any executor. This file builds `interp.kel` as a
//! `NativeStage`, seeds its shared segment for each program below, and drives it
//! with the protocol's commands. The same seeded buffer is driven through the
//! virtual machine the same way, and the reference outcome is V0.2.X's own
//! driver, `keleusma::hosted::Interpreter`. The native run must agree with the
//! driver in its final status and tick count, and with the virtual machine in
//! every status it yields and in the final shared segment, byte for byte.
//!
//! # What this does not establish
//!
//! The programs run to their end, so `protocol::RESUME`, which reads its input
//! from two slots the seeding does not write, is not exercised, as in V0.2.X's
//! own `tests/hosted_seed.rs`. The driver's result value is compared through the
//! shared segment rather than decoded. The corpus is five hand-written programs
//! and fourteen of the fifteen example scripts, the longest nine ticks, so
//! agreement here covers the interpreter's paths these programs reach and not
//! every instruction tag it handles. Removing the seeding fails both
//! comparisons, which was checked by mutation.
use keleusma::bytecode::{Module, Value};
use keleusma::hosted::{
    HostedValue, Interpreter, Outcome, admit, interpreter_module, interpreter_persistent_bytes,
    protocol, seeded_shared,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::stage::NativeStage;

/// More ticks than any program below takes. A run that reaches it fails.
const MAX_TICKS: u64 = 100_000;
/// The example scripts compared on 2026-10-30. A drop fails the test.
const MIN_SCRIPTS: usize = 14;

fn compiled(source: &str) -> Module {
    compile(&parse(&tokenize(source).expect("lex")).expect("parse")).expect("compile")
}

fn arena_for(interp: &Module) -> keleusma_arena::Arena {
    let need = interpreter_persistent_bytes(interp);
    let mut arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    arena
}

/// One command to the virtual machine, as an outside executor sends it. The
/// first enters `loop main` by a call and every later one by a resume, and a
/// reset re-enters the body with the same command, as `Interpreter` does.
fn vm_send(vm: &mut Vm<'_, '_>, shared: &mut [u8], cmd: i64) -> i64 {
    for _ in 0..2 {
        let state = match vm.resume_with_shared(shared, Value::Int(cmd)) {
            Err(VmError::NotSuspended) => vm.call_with_shared(shared, &[Value::Int(cmd)]),
            other => other,
        }
        .expect("interp.kel runs on the virtual machine");
        match state {
            VmState::Yielded(Value::Int(status)) => return status,
            VmState::Reset => continue,
            other => panic!("interp.kel produced {other:?}"),
        }
    }
    panic!("interp.kel reset twice without a status")
}

/// Every status a run yields, from `START` until it leaves `RUNNING` or
/// reaches the tick limit. The tick count is the number of `CONTINUE`s, as the
/// driver counts it.
fn drive(mut send: impl FnMut(i64) -> i64) -> (Vec<i64>, u64) {
    let mut statuses = vec![send(protocol::START)];
    let mut ticks = 0u64;
    while statuses.last() == Some(&protocol::RUNNING) && ticks < MAX_TICKS {
        statuses.push(send(protocol::CONTINUE));
        ticks += 1;
    }
    (statuses, ticks)
}

/// Programs reaching calls, nested branches, a tuple and a trap.
fn corpus() -> Vec<(&'static str, Vec<HostedValue>, i64)> {
    vec![
        (
            "fn main(a: Word, b: Word) -> Word { a * b + 1 }",
            vec![HostedValue::Word(6), HostedValue::Word(7)],
            protocol::FINISHED,
        ),
        (
            "fn f(n: Word) -> Word { if n > 2 { n * 3 } else { n + 1 } } \
             fn main(a: Word) -> Word { f(a) + f(a + 1) * 2 }",
            vec![HostedValue::Word(3)],
            protocol::FINISHED,
        ),
        (
            "fn main(n: Word) -> Word { if n > 5 { if n > 8 { n - 8 } else { n - 5 } } else { 0 } }",
            vec![HostedValue::Word(9)],
            protocol::FINISHED,
        ),
        (
            "fn swap(a: Word, b: Word) -> (Word, Word) { (b, a) } \
             fn main(a: Word) -> Word { let (x, y) = swap(a, a + 5); x - y }",
            vec![HostedValue::Word(4)],
            protocol::FINISHED,
        ),
        ("fn main() -> Word { 7 / 0 }", vec![], protocol::TRAPPED),
    ]
}

#[test]
fn the_seeded_interpreter_agrees_natively_with_the_driver_and_the_vm() {
    let interp = interpreter_module().expect("interp.kel compiles");
    let mut agreed = 0usize;
    for (source, args, want) in corpus() {
        let adm = admit(&compiled(source)).unwrap_or_else(|e| panic!("{source}: admit: {e:?}"));

        let arena = arena_for(&interp);
        let mut driver = Interpreter::new(interp.clone(), &arena).expect("interpreter");
        let expected = driver
            .run(&adm, &args, MAX_TICKS)
            .unwrap_or_else(|e| panic!("{source}: driver: {e:?}"));
        let driver_status = match expected.outcome {
            Outcome::Finished(_) => protocol::FINISHED,
            Outcome::Trapped { .. } => protocol::TRAPPED,
            ref other => panic!("{source}: unexpected driver outcome {other:?}"),
        };
        assert_eq!(driver_status, want, "{source}: the driver changed outcome");

        let seeded = seeded_shared(&interp, &adm, &args).expect("seeded");

        let mut vm_shared = seeded.clone();
        let vm_arena = arena_for(&interp);
        let mut vm = Vm::new(interp.clone(), &vm_arena).expect("interp.kel verifies");
        let (vm_statuses, vm_ticks) = drive(|cmd| vm_send(&mut vm, &mut vm_shared, cmd));

        let mut native = NativeStage::new(&interp).expect("interp.kel builds natively");
        assert_eq!(native.shared().len(), seeded.len(), "{source}");
        native.shared_mut().copy_from_slice(&seeded);
        let (native_statuses, native_ticks) =
            drive(|cmd| native.turn(cmd).unwrap_or_else(|e| panic!("{source}: {e}")));

        assert_eq!(native_statuses.last(), Some(&driver_status), "{source}");
        assert_eq!(
            native_ticks, expected.ticks,
            "{source}: ticks against the driver"
        );
        assert_eq!(
            native_statuses, vm_statuses,
            "{source}: statuses against the vm"
        );
        assert_eq!(native_ticks, vm_ticks, "{source}: ticks against the vm");
        assert!(
            native.shared() == vm_shared.as_slice(),
            "{source}: the final shared segments differ"
        );
        eprintln!(
            "  {source}: {} statuses over {native_ticks} ticks agree",
            native_statuses.len()
        );
        agreed += 1;
    }
    assert_eq!(agreed, corpus().len(), "a program was not compared");
}

/// The example scripts whose `main` takes no argument, driven the same way to
/// their first status other than `RUNNING`, which includes a first yield. A
/// script the hosted admission or the driver refuses is reported and not
/// compared, and `14_frame_log.kel` is refused for its private composite data.
/// At least `MIN_SCRIPTS` must be compared.
#[test]
fn the_seeded_interpreter_agrees_natively_on_the_example_scripts() {
    let interp = interpreter_module().expect("interp.kel compiles");
    let mut paths: Vec<_> = std::fs::read_dir("../examples/scripts")
        .expect("examples/scripts")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();
    let mut compared = 0usize;
    let mut longest = 0u64;
    for path in &paths {
        let name = path.display().to_string();
        let source = std::fs::read_to_string(path).expect("read");
        let Ok(module) = tokenize(&source)
            .map_err(|e| format!("{e:?}"))
            .and_then(|t| parse(&t).map_err(|e| format!("{e:?}")))
            .and_then(|p| compile(&p).map_err(|e| format!("{e:?}")))
        else {
            eprintln!("  {name}: does not compile standalone");
            continue;
        };
        let adm = match admit(&module) {
            Ok(adm) => adm,
            Err(e) => {
                eprintln!("  {name}: not admitted by the hosted runtime: {e:?}");
                continue;
            }
        };
        let arena = arena_for(&interp);
        let mut driver = Interpreter::new(interp.clone(), &arena).expect("interpreter");
        let expected = match driver.run(&adm, &[], MAX_TICKS) {
            Ok(run) => run,
            Err(e) => {
                eprintln!("  {name}: the driver refuses it: {e:?}");
                continue;
            }
        };
        let driver_status = match expected.outcome {
            Outcome::Finished(_) => protocol::FINISHED,
            Outcome::Trapped { .. } => protocol::TRAPPED,
            Outcome::Yielded(_) => protocol::YIELDED,
            ref other => {
                eprintln!("  {name}: driver outcome {other:?}, not compared");
                continue;
            }
        };
        let seeded = seeded_shared(&interp, &adm, &[]).expect("seeded");
        let mut vm_shared = seeded.clone();
        let vm_arena = arena_for(&interp);
        let mut vm = Vm::new(interp.clone(), &vm_arena).expect("interp.kel verifies");
        let (vm_statuses, _) = drive(|cmd| vm_send(&mut vm, &mut vm_shared, cmd));
        let mut native = NativeStage::new(&interp).expect("interp.kel builds natively");
        native.shared_mut().copy_from_slice(&seeded);
        let (native_statuses, native_ticks) =
            drive(|cmd| native.turn(cmd).unwrap_or_else(|e| panic!("{name}: {e}")));
        assert_eq!(native_statuses.last(), Some(&driver_status), "{name}");
        assert_eq!(
            native_ticks, expected.ticks,
            "{name}: ticks against the driver"
        );
        assert_eq!(
            native_statuses, vm_statuses,
            "{name}: statuses against the vm"
        );
        assert!(
            native.shared() == vm_shared.as_slice(),
            "{name}: the final shared segments differ"
        );
        eprintln!("  {name}: {native_ticks} ticks agree");
        longest = longest.max(native_ticks);
        compared += 1;
    }
    eprintln!(
        "  compared {compared} of {} scripts, the longest {longest} ticks",
        paths.len()
    );
    assert!(
        compared >= MIN_SCRIPTS,
        "only {compared} scripts were compared"
    );
}

/// MUST-FIRE: an unseeded interpreter does not reach the seeded outcome, so
/// the agreement above depends on the seeding and not on a program that
/// ignores its buffer.
#[test]
fn an_unseeded_native_interpreter_does_not_finish_the_program() {
    let interp = interpreter_module().expect("interp.kel compiles");
    let (source, args, _) = corpus().swap_remove(0);
    let adm = admit(&compiled(source)).expect("admitted");
    let seeded = seeded_shared(&interp, &adm, &args).expect("seeded");
    let mut native = NativeStage::new(&interp).expect("interp.kel builds natively");
    assert!(
        native.shared() != seeded.as_slice(),
        "seeding wrote nothing, so this control measures nothing"
    );
    let (statuses, _) = drive(|cmd| native.turn(cmd).expect("native turn"));
    assert_ne!(statuses.last(), Some(&protocol::FINISHED), "{statuses:?}");
}
