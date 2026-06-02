//! Compile-level and run-level smoke tests for the rogue
//! example's Keleusma scripts. These tests exercise the lex /
//! parse / compile pipeline and a stubbed-out native surface so
//! script errors surface in `cargo test` rather than at SDL3
//! startup. The tests are guarded by the `text` feature because
//! the host scripts may use string literals or f-strings.

#![cfg(all(feature = "compile", feature = "verify"))]

use std::sync::{Arc, Mutex};

use keleusma::bytecode::{TupleBody, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::value_layout::ScalarKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{Arena, Module};

/// Materialize the components of an all-`Word` tuple returned by a
/// rogue script, handling both the flat and boxed bodies (B28 P2).
///
/// The rogue AI, combat, and item scripts return tuples whose elements
/// are all `Word`, so each component reads back as an `i64`. A flat
/// body carries no element count, so the arity is derived from the
/// byte length at the bundled runtime's eight-byte word width; this is
/// the typed read the host performs now that a scalar tuple is pure
/// bytes. Panics if the value is not such a tuple.
fn word_tuple(v: &Value) -> Vec<i64> {
    match v {
        Value::Tuple(TupleBody::Boxed(elems)) => elems
            .iter()
            .map(|e| match e {
                Value::Int(n) => *n,
                other => panic!("non-int tuple element: {:?}", other),
            })
            .collect(),
        Value::Tuple(TupleBody::Flat(fc)) => {
            let bytes = fc.as_bytes();
            (0..bytes.len() / 8)
                .map(
                    |i| match Value::read_scalar_le(bytes, i * 8, ScalarKind::Int, 8, 8) {
                        Value::Int(n) => n,
                        other => panic!("non-int flat tuple element: {:?}", other),
                    },
                )
                .collect()
        }
        other => panic!("expected a Word tuple, got {:?}", other),
    }
}

const SRC_BESTIARY: &str = include_str!("../examples/scripts/rogue/rogue_bestiary.kel");
const SRC_GEAR: &str = include_str!("../examples/scripts/rogue/rogue_gear.kel");
const SRC_DUNGEN: &str = include_str!("../examples/scripts/rogue/rogue_dungen.kel");
const SRC_AI_IDLE: &str = include_str!("../examples/scripts/rogue/rogue_ai_idle.kel");
const SRC_AI_CHASER: &str = include_str!("../examples/scripts/rogue/rogue_ai_chaser.kel");
const SRC_AI_WANDER: &str = include_str!("../examples/scripts/rogue/rogue_ai_wander.kel");
const SRC_AI_SLEEPER: &str = include_str!("../examples/scripts/rogue/rogue_ai_sleeper.kel");
const SRC_AI_RANGED: &str = include_str!("../examples/scripts/rogue/rogue_ai_ranged.kel");
const SRC_AI_FAST: &str = include_str!("../examples/scripts/rogue/rogue_ai_fast.kel");
const SRC_AI_SMART: &str = include_str!("../examples/scripts/rogue/rogue_ai_smart.kel");
const SRC_AI_BOSS: &str = include_str!("../examples/scripts/rogue/rogue_ai_boss.kel");
const SRC_AI_TRACKER: &str = include_str!("../examples/scripts/rogue/rogue_ai_tracker.kel");
const SRC_ITEM_POTION: &str = include_str!("../examples/scripts/rogue/rogue_item_potion.kel");
const SRC_ITEM_SCROLL: &str = include_str!("../examples/scripts/rogue/rogue_item_scroll.kel");
const SRC_GAME: &str = include_str!("../examples/scripts/rogue/rogue_game.kel");
const SRC_PLAYER_AI: &str = include_str!("../examples/scripts/rogue/rogue_player_ai.kel");
const SRC_COMBAT: &str = include_str!("../examples/scripts/rogue/rogue_combat.kel");

fn build(src: &str) -> Module {
    let tokens = tokenize(src).expect("lex error");
    let program = parse(&tokens).expect("parse error");
    compile(&program).expect("compile error")
}

/// Stub-natives runner. Each native called by dungen returns a
/// deterministic value so the script's control flow exercises
/// every code path without panicking on missing natives.
struct DungenStub {
    rng_state: u32,
    map_set_count: usize,
    spawn_monster_count: usize,
    spawn_item_count: usize,
    place_player: Option<(i32, i32)>,
    place_stairs: Option<(i32, i32)>,
    place_exit: Option<(i32, i32)>,
    clear_count: usize,
}

impl DungenStub {
    fn new() -> Self {
        Self {
            rng_state: 0x9E37_79B9,
            map_set_count: 0,
            spawn_monster_count: 0,
            spawn_item_count: 0,
            place_player: None,
            place_stairs: None,
            place_exit: None,
            clear_count: 0,
        }
    }

    fn rng_next(&mut self) -> u32 {
        let mut s = self.rng_state;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.rng_state = s;
        s
    }
}

fn register_dungen_stub(vm: &mut Vm, state: &Arc<Mutex<DungenStub>>) {
    let s = state.clone();
    vm.register_native_closure(
        "host::rng_range",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let lo = match args[0] {
                Value::Int(n) => n,
                _ => 0,
            };
            let hi = match args[1] {
                Value::Int(n) => n,
                _ => 1,
            };
            if hi <= lo {
                return Err(VmError::NativeError(format!(
                    "rng_range: hi {} not greater than lo {}",
                    hi, lo
                )));
            }
            let mut st = s.lock().unwrap();
            let r = st.rng_next() % (hi - lo) as u32;
            Ok(Value::Int(lo + r as i64))
        }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::map_set",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> {
            s.lock().unwrap().map_set_count += 1;
            Ok(Value::Unit)
        }),
    );

    vm.register_native_closure(
        "host::map_get",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> { Ok(Value::Int(1)) }),
    );

    vm.register_native_closure(
        "host::map_w",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> { Ok(Value::Int(80)) }),
    );

    vm.register_native_closure(
        "host::map_h",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> { Ok(Value::Int(24)) }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::clear_floor",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> {
            s.lock().unwrap().clear_count += 1;
            Ok(Value::Unit)
        }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::place_player",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let (x, y) = match (&args[0], &args[1]) {
                (Value::Int(x), Value::Int(y)) => (*x as i32, *y as i32),
                _ => (0, 0),
            };
            s.lock().unwrap().place_player = Some((x, y));
            Ok(Value::Unit)
        }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::place_stairs",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let (x, y) = match (&args[0], &args[1]) {
                (Value::Int(x), Value::Int(y)) => (*x as i32, *y as i32),
                _ => (0, 0),
            };
            s.lock().unwrap().place_stairs = Some((x, y));
            Ok(Value::Unit)
        }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::place_exit",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let (x, y) = match (&args[0], &args[1]) {
                (Value::Int(x), Value::Int(y)) => (*x as i32, *y as i32),
                _ => (0, 0),
            };
            s.lock().unwrap().place_exit = Some((x, y));
            Ok(Value::Unit)
        }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::spawn_monster",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> {
            s.lock().unwrap().spawn_monster_count += 1;
            Ok(Value::Unit)
        }),
    );

    let s = state.clone();
    vm.register_native_closure(
        "host::spawn_item",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> {
            s.lock().unwrap().spawn_item_count += 1;
            Ok(Value::Unit)
        }),
    );

    vm.register_native_closure(
        "host::floor",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> { Ok(Value::Int(1)) }),
    );
}

#[test]
fn dungen_compiles() {
    let _ = build(SRC_DUNGEN);
}

#[test]
fn game_tick_compiles() {
    let _ = build(SRC_GAME);
}

#[test]
fn player_ai_compiles() {
    let _ = build(SRC_PLAYER_AI);
}

#[test]
fn ai_tracker_compiles() {
    let _ = build(SRC_AI_TRACKER);
}

#[test]
fn ai_tracker_chases_when_seen() {
    let module = build(SRC_AI_TRACKER);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    let input = Value::tuple(vec![
        Value::Int(5),
        Value::Int(5),
        Value::Int(10),
        Value::Int(10),
        Value::Int(1),
    ]);
    let result = vm.call(&[input]).expect("vm call");
    match result {
        VmState::Yielded(ref v @ Value::Tuple(_)) => {
            let c = word_tuple(v);
            assert_eq!(c.len(), 3, "expected Yielded triple");
            assert_eq!(c[0], 1);
            assert_eq!((c[1], c[2]), (6, 6));
        }
        other => panic!("expected Yielded triple, got {:?}", other),
    }
}

#[test]
fn ai_tracker_pursues_last_known_when_unseen() {
    let module = build(SRC_AI_TRACKER);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    // First turn: player visible at (10, 10). Tracker chases and
    // records the last known position.
    let visible_input = Value::tuple(vec![
        Value::Int(5),
        Value::Int(5),
        Value::Int(10),
        Value::Int(10),
        Value::Int(1),
    ]);
    vm.call(&[visible_input]).expect("vm call");
    // Loop main wraps; the host walks past Reset to the next
    // Yielded with a fresh input. Second turn: player not
    // visible. Tracker should move toward the stored last
    // position.
    let unseen_input = Value::tuple(vec![
        Value::Int(6),
        Value::Int(6),
        Value::Int(0),
        Value::Int(0),
        Value::Int(0),
    ]);
    let mut state = vm.resume(unseen_input.clone()).expect("vm resume");
    for _ in 0..16 {
        match state {
            VmState::Yielded(ref v @ Value::Tuple(_)) => {
                let c = word_tuple(v);
                assert_eq!(c.len(), 3, "non-int tuple");
                assert_eq!(c[0], 1, "should chase last known");
                assert_eq!((c[1], c[2]), (7, 7), "step toward (10, 10)");
                return;
            }
            VmState::Reset => {
                state = vm.resume(unseen_input.clone()).expect("vm resume");
            }
            other => panic!("expected Yielded or Reset, got {:?}", other),
        }
    }
    panic!("tracker did not yield within sixteen resumes");
}

#[test]
fn combat_compiles() {
    let _ = build(SRC_COMBAT);
}

fn run_player_ai(mx: i64, my: i64, cmd: i64) -> (i64, i64, i64) {
    let module = build(SRC_PLAYER_AI);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    let result = vm
        .call(&[Value::Int(mx), Value::Int(my), Value::Int(cmd)])
        .expect("vm call");
    match result {
        VmState::Finished(ref v @ Value::Tuple(_)) => {
            let c = word_tuple(v);
            assert_eq!(c.len(), 3, "player ai returned non-int tuple components");
            (c[0], c[1], c[2])
        }
        other => panic!("expected Finished triple, got {:?}", other),
    }
}

#[test]
fn player_ai_wait_returns_action_zero() {
    let (action, tx, ty) = run_player_ai(5, 5, 0);
    assert_eq!(action, 0);
    assert_eq!((tx, ty), (5, 5));
}

#[test]
fn player_ai_move_north_returns_action_one() {
    let (action, tx, ty) = run_player_ai(5, 5, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (5, 4));
}

#[test]
fn player_ai_move_diagonal_southeast() {
    let (action, tx, ty) = run_player_ai(5, 5, 8);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (6, 6));
}

#[test]
fn player_ai_descend_returns_action_three() {
    let (action, _tx, _ty) = run_player_ai(5, 5, 9);
    assert_eq!(action, 3);
}

#[test]
fn player_ai_quaff_returns_action_four() {
    let (action, _tx, _ty) = run_player_ai(5, 5, 10);
    assert_eq!(action, 4);
}

#[test]
fn player_ai_read_returns_action_five() {
    let (action, _tx, _ty) = run_player_ai(5, 5, 11);
    assert_eq!(action, 5);
}

fn run_combat(skill: i64, dmg: i64, evasion: i64, armor: i64, roll: i64) -> (i64, i64) {
    let module = build(SRC_COMBAT);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    let result = vm
        .call(&[
            Value::Int(skill),
            Value::Int(dmg),
            Value::Int(evasion),
            Value::Int(armor),
            Value::Int(roll),
        ])
        .expect("vm call");
    match result {
        VmState::Finished(ref v @ Value::Tuple(_)) => {
            let c = word_tuple(v);
            assert_eq!(c.len(), 2, "combat returned non-int tuple");
            (c[0], c[1])
        }
        other => panic!("expected Finished pair, got {:?}", other),
    }
}

#[test]
fn combat_fumble_always_misses() {
    let (hit, dmg) = run_combat(20, 10, 0, 0, 1);
    assert_eq!((hit, dmg), (0, 0));
}

#[test]
fn combat_critical_always_hits() {
    let (hit, dmg) = run_combat(0, 5, 50, 0, 20);
    assert_eq!(hit, 2);
    assert_eq!(dmg, 10);
}

#[test]
fn combat_ordinary_hit_subtracts_armor() {
    let (hit, dmg) = run_combat(10, 8, 0, 3, 12);
    assert_eq!(hit, 1);
    assert_eq!(dmg, 5);
}

#[test]
fn combat_miss_returns_zero_damage() {
    let (hit, dmg) = run_combat(0, 8, 5, 0, 10);
    assert_eq!((hit, dmg), (0, 0));
}

#[test]
fn combat_damage_floored_at_one() {
    let (hit, dmg) = run_combat(10, 2, 0, 8, 12);
    assert_eq!(hit, 1);
    assert_eq!(dmg, 1);
}

#[test]
fn game_tick_runs_with_stubbed_natives() {
    // Drive the game-tick loop main with stubbed natives. The
    // four natives return deterministic values so the script's
    // control flow is exercised end-to-end without the full
    // host. The test verifies that the script reaches a yielded
    // outcome on the first turn.
    use std::cell::RefCell;
    let module = build(SRC_GAME);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }

    let monster_calls = std::rc::Rc::new(RefCell::new(0_i64));
    let book_calls = std::rc::Rc::new(RefCell::new(0_i64));

    vm.register_native_closure(
        "host::run_player_turn",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> { Ok(Value::Int(0)) }),
    );
    vm.register_native_closure(
        "host::monster_count",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> { Ok(Value::Int(3)) }),
    );
    let mc = monster_calls.clone();
    vm.register_native_closure(
        "host::run_monster_ai",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> {
            *mc.borrow_mut() += 1;
            Ok(Value::Unit)
        }),
    );
    let bc = book_calls.clone();
    vm.register_native_closure(
        "host::tick_book_keeping",
        Box::new(move |_args: &[Value]| -> Result<Value, VmError> {
            *bc.borrow_mut() += 1;
            Ok(Value::Int(0))
        }),
    );

    let result = vm.call(&[Value::Int(0)]).expect("vm call");
    match result {
        VmState::Yielded(Value::Int(outcome)) => {
            assert_eq!(outcome, 0, "first turn should yield continue");
        }
        other => panic!("expected Yielded(Int), got {:?}", other),
    }
    assert_eq!(
        *monster_calls.borrow(),
        3,
        "run_monster_ai should fire once per declared monster"
    );
    assert_eq!(
        *book_calls.borrow(),
        1,
        "tick_book_keeping should fire once per turn"
    );
}

#[test]
fn bestiary_compiles() {
    let _ = build(SRC_BESTIARY);
}

#[test]
fn bestiary_negative_one_returns_last_entry() {
    let module = build(SRC_BESTIARY);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    vm.call(&[Value::Int(-1)]).expect("call -1");
    let last_id = match vm.get_data(0).expect("get_data id") {
        Value::Int(n) => *n,
        other => panic!("expected Int, got {:?}", other),
    };
    assert_eq!(last_id, 99, "the bestiary ships with one hundred entries");
}

#[test]
fn bestiary_returns_name_as_text() {
    let module = build(SRC_BESTIARY);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    let state = vm.call(&[Value::Int(0)]).expect("call 0");
    match state {
        VmState::Finished(Value::StaticStr(s)) => assert_eq!(s, "Sewer Rat"),
        other => panic!("expected Finished(StaticStr), got {:?}", other),
    }
}

#[test]
fn bestiary_corpse_data_derived_from_shape() {
    // Entry 0 is the Sewer Rat (shape Tiny=0). Tiny corpse:
    // drop_chance=50, satiation=8, hp_delta=0.
    let module = build(SRC_BESTIARY);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    vm.call(&[Value::Int(0)]).expect("call 0");
    let read = |slot: usize| -> i64 {
        match vm.get_data(slot).expect("get_data") {
            Value::Int(n) => *n,
            _ => panic!(),
        }
    };
    assert_eq!(read(16), 50, "Tiny corpse drop chance");
    assert_eq!(read(17), 8, "Tiny corpse satiation");
    assert_eq!(read(18), 0, "Tiny corpse hp delta");
}

#[test]
fn bestiary_entry_zero_is_sewer_rat_stats() {
    let module = build(SRC_BESTIARY);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    vm.call(&[Value::Int(0)]).expect("call 0");
    // Sewer Rat: shape Tiny=0, max_hp=3, ai Chaser=2,
    // first_floor=1, score=1. See rogue_bestiary.kel entry 0.
    let read = |slot: usize| -> i64 {
        match vm.get_data(slot).expect("get_data") {
            Value::Int(n) => *n,
            _ => panic!(),
        }
    };
    assert_eq!(read(0), 0, "id");
    assert_eq!(read(1), 0, "shape Tiny");
    assert_eq!(read(8), 3, "max_hp");
    assert_eq!(read(13), 2, "ai Chaser");
    assert_eq!(read(14), 1, "first_floor");
    assert_eq!(read(15), 1, "score");
}

#[test]
fn gear_compiles() {
    let _ = build(SRC_GEAR);
}

#[test]
fn gear_weapon_zero_is_fists_damage_two() {
    let module = build(SRC_GEAR);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    // table 0 = weapons, tier 0 = fists, damage 2.
    vm.call(&[Value::Int(0), Value::Int(0)]).expect("call");
    let value = match vm.get_data(1).expect("get_data") {
        Value::Int(n) => *n,
        _ => panic!(),
    };
    assert_eq!(value, 2, "fists damage");
}

#[test]
fn gear_armor_negative_one_is_last_guard_defense_forty() {
    let module = build(SRC_GEAR);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    // table 1 = armors, tier -1 = last guard, defense 40.
    vm.call(&[Value::Int(1), Value::Int(-1)]).expect("call");
    let read = |slot: usize| -> i64 {
        match vm.get_data(slot).expect("get_data") {
            Value::Int(n) => *n,
            _ => panic!(),
        }
    };
    assert_eq!(read(0), 19, "last guard id");
    assert_eq!(read(1), 40, "last guard defense");
}

#[test]
fn dungen_runs_floor_1() {
    let module = build(SRC_DUNGEN);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    let stub = Arc::new(Mutex::new(DungenStub::new()));
    register_dungen_stub(&mut vm, &stub);
    let result = vm.call(&[Value::Int(1)]).expect("vm call");
    match result {
        VmState::Finished(_) => {}
        other => panic!("expected Finished, got {:?}", other),
    }
    let st = stub.lock().unwrap();
    assert_eq!(st.clear_count, 1, "clear_floor should be called once");
    assert!(st.place_player.is_some(), "player should be placed");
    assert!(
        st.place_stairs.is_some(),
        "stairs should be placed on floor 1"
    );
    assert!(st.place_exit.is_none(), "exit should not appear on floor 1");
    assert!(
        st.spawn_monster_count > 0,
        "at least one monster should spawn"
    );
    assert!(st.spawn_item_count > 0, "at least one item should spawn");
    assert!(st.map_set_count > 100, "many map cells should be written");
}

// -- Artificial-intelligence script tests ---------------------------

fn call_ai(src: &str, mx: i64, my: i64, px: i64, py: i64, sees: i64) -> (i64, i64, i64) {
    let module = build(src);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    // Provide rng_range for archetypes that import it. Returns
    // a deterministic value chosen to exercise the random path.
    vm.register_native_closure(
        "host::rng_range",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let lo = match args[0] {
                Value::Int(n) => n,
                _ => 0,
            };
            Ok(Value::Int(lo))
        }),
    );
    let result = vm
        .call(&[
            Value::Int(mx),
            Value::Int(my),
            Value::Int(px),
            Value::Int(py),
            Value::Int(sees),
        ])
        .expect("ai vm call");
    match result {
        VmState::Finished(ref v @ Value::Tuple(_)) => {
            let c = word_tuple(v);
            assert_eq!(c.len(), 3, "ai returned non-int tuple components");
            (c[0], c[1], c[2])
        }
        other => panic!("expected Finished tuple, got {:?}", other),
    }
}

#[test]
fn ai_idle_waits_in_place() {
    let (action, tx, ty) = call_ai(SRC_AI_IDLE, 5, 5, 10, 10, 1);
    assert_eq!(action, 0);
    assert_eq!((tx, ty), (5, 5));
}

#[test]
fn ai_chaser_steps_toward_player_when_seen() {
    let (action, tx, ty) = call_ai(SRC_AI_CHASER, 5, 5, 10, 10, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (6, 6));
}

#[test]
fn ai_chaser_waits_when_unseen() {
    let (action, _tx, _ty) = call_ai(SRC_AI_CHASER, 5, 5, 10, 10, 0);
    assert_eq!(action, 0);
}

#[test]
fn ai_wander_chases_when_seen() {
    let (action, tx, ty) = call_ai(SRC_AI_WANDER, 5, 5, 10, 10, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (6, 6));
}

#[test]
fn ai_sleeper_chases_when_seen() {
    let (action, tx, ty) = call_ai(SRC_AI_SLEEPER, 5, 5, 10, 10, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (6, 6));
}

#[test]
fn ai_sleeper_waits_when_unseen() {
    let (action, _tx, _ty) = call_ai(SRC_AI_SLEEPER, 5, 5, 10, 10, 0);
    assert_eq!(action, 0);
}

#[test]
fn ai_ranged_attacks_when_distant() {
    let (action, tx, ty) = call_ai(SRC_AI_RANGED, 5, 5, 10, 10, 1);
    assert_eq!(action, 2);
    assert_eq!((tx, ty), (10, 10));
}

#[test]
fn ai_ranged_retreats_when_adjacent() {
    let (action, tx, ty) = call_ai(SRC_AI_RANGED, 5, 5, 6, 5, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (4, 5));
}

#[test]
fn ai_fast_steps_toward_player() {
    let (action, tx, ty) = call_ai(SRC_AI_FAST, 5, 5, 10, 10, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (6, 6));
}

#[test]
fn ai_smart_dominant_axis_step() {
    // Player far east, same row. Smart picks the dominant axis.
    let (action, tx, ty) = call_ai(SRC_AI_SMART, 5, 5, 20, 5, 1);
    assert_eq!(action, 1);
    assert_eq!((tx, ty), (6, 5));
}

/// Helper for the boss archetype which uses `loop main` with a
/// five-tuple input. Returns the yielded action triple for the
/// supplied turn inputs.
fn call_boss_first(
    mx: i64,
    my: i64,
    px: i64,
    py: i64,
    sees: i64,
) -> (i64, i64, i64, Vm<'static, 'static>, Arena) {
    let module = build(SRC_AI_BOSS);
    // The arena and vm need to escape this helper for callers
    // that want to resume. Return them by value.
    let arena: Arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    // Safety. We leak the arena reference because the borrow
    // checker cannot prove the vm's reference outlives the
    // function frame. The test process is short-lived so the
    // leak is irrelevant.
    let arena_ref: &'static Arena = Box::leak(Box::new(arena));
    let mut vm: Vm<'static, 'static> = Vm::new(module, arena_ref).expect("vm new");
    for slot in 0..vm.data_len() {
        vm.set_data(slot, Value::Int(0)).expect("set_data");
    }
    let input = Value::tuple(vec![
        Value::Int(mx),
        Value::Int(my),
        Value::Int(px),
        Value::Int(py),
        Value::Int(sees),
    ]);
    let result = vm.call(&[input]).expect("vm call");
    let triple = match result {
        VmState::Yielded(ref v @ Value::Tuple(_)) => {
            let c = word_tuple(v);
            assert_eq!(c.len(), 3, "boss yielded non-int tuple components");
            (c[0], c[1], c[2])
        }
        other => panic!("expected Yielded triple, got {:?}", other),
    };
    let dummy_arena = Arena::with_capacity(0);
    (triple.0, triple.1, triple.2, vm, dummy_arena)
}

#[test]
fn ai_boss_first_turn_attacks_at_range_when_distant() {
    // Phase zero is a ranged attack when the player is visible.
    let (action, tx, ty, _vm, _arena) = call_boss_first(5, 5, 12, 12, 1);
    assert_eq!(action, 2);
    assert_eq!((tx, ty), (12, 12));
}

#[test]
fn ai_boss_first_turn_waits_when_unseen() {
    let (action, _tx, _ty, _vm, _arena) = call_boss_first(5, 5, 12, 12, 0);
    assert_eq!(action, 0);
}

#[test]
fn ai_boss_second_turn_chases() {
    // The boss alternates ranged and chase. The second visible
    // turn lands on phase one which is a chase step. `loop main`
    // emits Reset at the body's wrap point so the helper loops
    // past Reset until the next Yielded.
    let (_a1, _x1, _y1, mut vm, _arena) = call_boss_first(5, 5, 12, 12, 1);
    let input = Value::tuple(vec![
        Value::Int(5),
        Value::Int(5),
        Value::Int(12),
        Value::Int(12),
        Value::Int(1),
    ]);
    let mut state = vm.resume(input.clone()).expect("vm resume");
    for _ in 0..16 {
        match state {
            VmState::Yielded(ref v @ Value::Tuple(_)) => {
                let c = word_tuple(v);
                assert_eq!(c.len(), 3, "non-int tuple components");
                assert_eq!(c[0], 1, "second turn should chase");
                assert_eq!((c[1], c[2]), (6, 6), "should step diagonally toward player");
                return;
            }
            VmState::Reset => {
                state = vm.resume(input.clone()).expect("vm resume after reset");
            }
            other => panic!("expected Yielded or Reset, got {:?}", other),
        }
    }
    panic!("boss vm did not yield within sixteen resume cycles");
}

// -- Item-effect script tests --------------------------------------

fn call_5_tuple(src: &str, args: &[i64]) -> (i64, i64, i64, i64, i64) {
    let module = build(src);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    let values: Vec<Value> = args.iter().map(|n| Value::Int(*n)).collect();
    let result = vm.call(&values).expect("vm call");
    match result {
        VmState::Finished(ref v @ Value::Tuple(_)) => {
            let c = word_tuple(v);
            assert_eq!(c.len(), 5, "expected 5-tuple");
            (c[0], c[1], c[2], c[3], c[4])
        }
        other => panic!("expected 5-tuple, got {:?}", other),
    }
}

#[test]
fn potion_healing_heals_five() {
    let (hp, _, _, _, _) = call_5_tuple(SRC_ITEM_POTION, &[0, 5, 12]);
    assert_eq!(hp, 5);
}

#[test]
fn potion_greater_healing_heals_fifteen() {
    let (hp, _, _, _, _) = call_5_tuple(SRC_ITEM_POTION, &[1, 5, 30]);
    assert_eq!(hp, 15);
}

#[test]
fn potion_restoration_returns_status_11() {
    let (_, _, _, status, _) = call_5_tuple(SRC_ITEM_POTION, &[2, 5, 30]);
    assert_eq!(status, 11);
}

#[test]
fn potion_poison_damages_three() {
    let (hp, _, _, _, _) = call_5_tuple(SRC_ITEM_POTION, &[3, 5, 30]);
    assert_eq!(hp, -3);
}

#[test]
fn potion_strength_raises_max_hp() {
    let (hp, max_hp, _, _, _) = call_5_tuple(SRC_ITEM_POTION, &[5, 5, 12]);
    assert_eq!((hp, max_hp), (2, 2));
}

#[test]
fn potion_skill_raises_skill() {
    let (_, _, skill, _, _) = call_5_tuple(SRC_ITEM_POTION, &[6, 5, 12]);
    assert_eq!(skill, 1);
}

#[test]
fn scroll_identify_returns_status_3() {
    let (_, _, _, status, _) = call_5_tuple(SRC_ITEM_SCROLL, &[0]);
    assert_eq!(status, 3);
}

#[test]
fn scroll_magic_mapping_returns_status_1() {
    let (_, _, _, status, _) = call_5_tuple(SRC_ITEM_SCROLL, &[1]);
    assert_eq!(status, 1);
}

#[test]
fn scroll_teleport_returns_status_2() {
    let (_, _, _, status, _) = call_5_tuple(SRC_ITEM_SCROLL, &[2]);
    assert_eq!(status, 2);
}

#[test]
fn scroll_enchant_weapon_returns_status_4_arg_1() {
    let (_, _, _, status, arg) = call_5_tuple(SRC_ITEM_SCROLL, &[3]);
    assert_eq!((status, arg), (4, 1));
}

#[test]
fn scroll_enchant_armor_returns_status_5_arg_1() {
    let (_, _, _, status, arg) = call_5_tuple(SRC_ITEM_SCROLL, &[4]);
    assert_eq!((status, arg), (5, 1));
}

// Gated against narrow-word runtimes because the dungeon-generator
// at depth 100 computes intermediate values (e.g., a width seeded
// from `floor * scaling`) that wrap on a narrow Word and trip a
// runtime range check (`rng_range: hi -108 not greater than lo 100`).
// The smaller-depth test (`dungen_runs_floor_1`) stays within range
// on every supported Word and continues to run on narrowed builds.
#[test]
#[cfg(not(any(
    feature = "narrow-word-8",
    feature = "narrow-word-16",
    feature = "narrow-word-32"
)))]
fn dungen_runs_floor_100_places_exit() {
    let module = build(SRC_DUNGEN);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("vm new");
    let stub = Arc::new(Mutex::new(DungenStub::new()));
    register_dungen_stub(&mut vm, &stub);
    vm.call(&[Value::Int(100)]).expect("vm call");
    let st = stub.lock().unwrap();
    assert!(st.place_exit.is_some(), "floor 100 should place the exit");
    assert!(
        st.place_stairs.is_none(),
        "floor 100 should not place stairs down"
    );
}
