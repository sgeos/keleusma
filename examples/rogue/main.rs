//! Roguelike example. SDL3 host driving a collection of Keleusma
//! scripts for dungeon generation, monster artificial intelligence,
//! and item effects.
//!
//! See `docs/guide/ROGUE.md` for the long-form manual once the
//! example is complete. This file is the host entry point. The
//! supporting modules are declared below.
//!
//! Build with `cargo run --release --example rogue --features
//! sdl3-example,text`.
//!
//! # Script location
//!
//! The Keleusma scripts for this example live in
//! `examples/scripts/rogue/`. The `include_str!` lines below
//! reference that directory through a relative path, and the
//! `SCRIPT_DIR` constant points there for the hot-reload path.
//! Edit a `.kel` file there and press `F5` in the running game
//! to pick up the change without rebuilding the host binary.
//!
//! The `dead_code` allow is global because several enum variants,
//! struct fields, and table entries are defined ahead of the
//! phase that consumes them. Each landing phase removes the
//! corresponding portion of unused-marker debt.

#![allow(dead_code)]

mod ai;
mod bestiary;
mod combat;
mod fov;
mod input;
mod items;
mod natives;
mod render;
mod text;
mod tiles;
mod world;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use sdl3::event::Event;
use sdl3::pixels::Color;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Module, Value};

use crate::ai::{AiModules, AiPool, AiPoolHandle};
use crate::input::Command;
use crate::render::{GameOver, Renderer};
use crate::tiles::TileAtlas;
use crate::world::World;

/// Map dimensions in tiles. Eighty by twenty-four is the classic
/// rogue grid size.
pub const MAP_W: u32 = 80;
pub const MAP_H: u32 = 24;

/// Tile size in pixels. The window's grid region is therefore
/// twenty-four pixels times the map dimensions.
pub const TILE_PX: u32 = 24;

/// Pixel height of the HUD line drawn above the map and the
/// message line drawn below. Each is one tile tall.
pub const HUD_PX: u32 = TILE_PX;
pub const MSG_PX: u32 = TILE_PX;

/// Total window dimensions derived from the map dimensions plus
/// the HUD row above and the message row below.
pub const WINDOW_W: u32 = MAP_W * TILE_PX;
pub const WINDOW_H: u32 = HUD_PX + MAP_H * TILE_PX + MSG_PX;

/// Script sources embedded at compile time so the example does
/// not depend on the current working directory.
const SRC_DUNGEN: &str = include_str!("../scripts/rogue/rogue_dungen.kel");
const SRC_AI_IDLE: &str = include_str!("../scripts/rogue/rogue_ai_idle.kel");
const SRC_AI_CHASER: &str = include_str!("../scripts/rogue/rogue_ai_chaser.kel");
const SRC_AI_WANDER: &str = include_str!("../scripts/rogue/rogue_ai_wander.kel");
const SRC_AI_SLEEPER: &str = include_str!("../scripts/rogue/rogue_ai_sleeper.kel");
const SRC_AI_RANGED: &str = include_str!("../scripts/rogue/rogue_ai_ranged.kel");
const SRC_AI_FAST: &str = include_str!("../scripts/rogue/rogue_ai_fast.kel");
const SRC_AI_SMART: &str = include_str!("../scripts/rogue/rogue_ai_smart.kel");
const SRC_AI_BOSS: &str = include_str!("../scripts/rogue/rogue_ai_boss.kel");
const SRC_AI_TRACKER: &str = include_str!("../scripts/rogue/rogue_ai_tracker.kel");
const SRC_ITEM_POTION: &str = include_str!("../scripts/rogue/rogue_item_potion.kel");
const SRC_ITEM_SCROLL: &str = include_str!("../scripts/rogue/rogue_item_scroll.kel");
const SRC_GAME: &str = include_str!("../scripts/rogue/rogue_game.kel");
const SRC_PLAYER_AI: &str = include_str!("../scripts/rogue/rogue_player_ai.kel");
const SRC_COMBAT: &str = include_str!("../scripts/rogue/rogue_combat.kel");

/// Directory containing the Keleusma script sources on disk.
/// The initial load uses the `include_str!` constants above so
/// the example runs even without filesystem access. The hot
/// reload path reads from this directory at run time, allowing
/// scripts edited in another window to take effect on `F5`
/// without rebuilding the host binary.
const SCRIPT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts/rogue");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl_context = sdl3::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("Keleusma Roguelike (work in progress)", WINDOW_W, WINDOW_H)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();
    let mut atlas = TileAtlas::build(&mut canvas, &texture_creator)?;

    // Shared world state. The dungeon generator, artificial-
    // intelligence pool, and turn dispatcher all mutate this
    // through Arc<Mutex<_>>.
    let world: natives::WorldHandle = Arc::new(Mutex::new(World::new()));

    // Build the dungeon-generation virtual machine.
    let dungen_arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let dungen_module = build_module(SRC_DUNGEN)?;
    let mut dungen_vm =
        Vm::new(dungen_module, &dungen_arena).map_err(|e| format!("vm new: {:?}", e))?;
    init_data_slots(&mut dungen_vm);
    natives::register_natives(&mut dungen_vm, &world);

    // Build the artificial-intelligence virtual-machine pool.
    // The pool owns its arenas through `Box::leak` so it can be
    // wrapped in `Arc<Mutex<_>>` and shared with native closures.
    let ai_modules = AiModules {
        idle: build_module(SRC_AI_IDLE)?,
        chaser: build_module(SRC_AI_CHASER)?,
        wander: build_module(SRC_AI_WANDER)?,
        sleeper: build_module(SRC_AI_SLEEPER)?,
        ranged: build_module(SRC_AI_RANGED)?,
        fast: build_module(SRC_AI_FAST)?,
        smart: build_module(SRC_AI_SMART)?,
        boss: build_module(SRC_AI_BOSS)?,
        tracker: build_module(SRC_AI_TRACKER)?,
        potion: build_module(SRC_ITEM_POTION)?,
        scroll: build_module(SRC_ITEM_SCROLL)?,
        player: build_module(SRC_PLAYER_AI)?,
        combat: build_module(SRC_COMBAT)?,
    };
    // The pool wraps non-Send virtual machines, but the example
    // is single-threaded so the `Arc<Mutex<_>>` is safe in
    // practice. The lint is acknowledged with an `allow`.
    #[allow(clippy::arc_with_non_send_sync)]
    let ai_pool: AiPoolHandle = Arc::new(Mutex::new(AiPool::new(ai_modules, &world)?));

    // Build the game-tick virtual machine. The script is a
    // `loop main` that the host resumes once per player input.
    // Its natives drive per-monster artificial-intelligence
    // dispatch through the shared `AiPool` handle.
    let game_arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let game_module = build_module(SRC_GAME)?;
    let mut game_vm = Vm::new(game_module, &game_arena).map_err(|e| format!("vm new: {:?}", e))?;
    init_data_slots(&mut game_vm);
    natives::register_game_natives(&mut game_vm, &world, &ai_pool);
    let mut game_started = false;

    // Generate the first floor.
    run_dungen(&mut dungen_vm, 1)?;
    {
        let mut w = world.lock().unwrap();
        w.push_message(String::from("Welcome, brave adventurer."));
        w.recompute_fov();
    }

    let mut renderer = Renderer::new();
    let mut event_pump = sdl_context.event_pump()?;
    let mut game_over: Option<GameOver> = None;
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => {
                    if game_over.is_some() {
                        break 'running;
                    }
                    if let Some(cmd) = input::translate(keycode) {
                        match cmd {
                            Command::Quit => break 'running,
                            Command::Reload => {
                                reload_scripts(&world, &mut dungen_vm, &dungen_arena, &ai_pool);
                            }
                            other => {
                                let cmd_code = encode_command(other);
                                let outcome =
                                    run_game_tick(&mut game_vm, &mut game_started, cmd_code)?;
                                match outcome {
                                    0 => {}
                                    1 => {
                                        descend_floor(&world, &mut dungen_vm)?;
                                    }
                                    2 => {
                                        let mut w = world.lock().unwrap();
                                        w.push_message(String::from(
                                            "You step into the light. You win!",
                                        ));
                                        game_over = Some(GameOver::Won);
                                    }
                                    _ => {
                                        game_over = Some(GameOver::Died);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        {
            let w = world.lock().unwrap();
            renderer.draw(&mut canvas, &mut atlas, &w)?;
            if let Some(outcome) = game_over {
                renderer.draw_game_over(&mut canvas, &w, outcome)?;
            }
        }
        canvas.present();

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

/// Translate the input::Command enum into the integer code the
/// game-tick script expects.
fn encode_command(cmd: Command) -> i64 {
    match cmd {
        Command::Move(0, 0) => 0,
        Command::Move(0, -1) => 1,
        Command::Move(0, 1) => 2,
        Command::Move(-1, 0) => 3,
        Command::Move(1, 0) => 4,
        Command::Move(-1, -1) => 5,
        Command::Move(1, -1) => 6,
        Command::Move(-1, 1) => 7,
        Command::Move(1, 1) => 8,
        Command::Move(_, _) => 0,
        Command::Descend => 9,
        Command::Quaff => 10,
        Command::Read => 11,
        Command::Reload | Command::Quit => 0,
    }
}

/// Resume the game-tick virtual machine with the supplied
/// command code. Returns the outcome code yielded by the script.
/// Walks past `Reset` boundaries until the next `Yielded`.
fn run_game_tick(
    vm: &mut Vm,
    started: &mut bool,
    cmd: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let mut state = if *started {
        vm.resume(Value::Int(cmd))
    } else {
        *started = true;
        vm.call(&[Value::Int(cmd)])
    }
    .map_err(|e| format!("game vm: {:?}", e))?;
    for _ in 0..16 {
        match state {
            VmState::Yielded(Value::Int(n)) => return Ok(n),
            VmState::Reset => {
                state = vm
                    .resume(Value::Int(cmd))
                    .map_err(|e| format!("game vm: {:?}", e))?;
            }
            VmState::Finished(_) => return Err("game vm finished unexpectedly".into()),
            other => return Err(format!("game vm returned unexpected shape: {:?}", other).into()),
        }
    }
    Err("game vm exhausted Reset budget without yielding".into())
}

fn init_data_slots(vm: &mut Vm) {
    for slot in 0..vm.data_len() {
        let _ = vm.set_data(slot, Value::Int(0));
    }
}

fn build_module(src: &str) -> Result<Module, Box<dyn std::error::Error>> {
    let tokens = tokenize(src).map_err(|e| format!("lex error: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("parse error: {:?}", e))?;
    let module = compile(&program).map_err(|e| format!("compile error: {:?}", e))?;
    Ok(module)
}

fn run_dungen(vm: &mut Vm, floor: i64) -> Result<(), Box<dyn std::error::Error>> {
    match vm
        .call(&[Value::Int(floor)])
        .map_err(|e| format!("dungen vm: {:?}", e))?
    {
        VmState::Finished(_) => Ok(()),
        VmState::Yielded(_) => Err("dungen yielded; expected fn-style return".into()),
        VmState::Reset => Err("dungen reset; expected fn-style return".into()),
    }
}

/// Hot reload every Keleusma script from the on-disk script
/// directory. If any source fails to load or compile, every
/// running virtual machine is left untouched and the message
/// log records the failure. On success, the dungeon-generation
/// virtual machine and every artificial-intelligence and item
/// virtual machine are replaced. The world state is not
/// touched. Re-running the dungen happens implicitly on the
/// next stairs descent; in-flight gameplay continues against
/// the freshly reloaded artificial-intelligence and item
/// scripts immediately.
fn reload_scripts<'a, 'd>(
    world: &natives::WorldHandle,
    dungen_vm: &mut Vm<'a, 'd>,
    dungen_arena: &'d Arena,
    ai_pool: &AiPoolHandle,
) where
    'd: 'a,
{
    let names = [
        "rogue_dungen.kel",
        "rogue_ai_idle.kel",
        "rogue_ai_chaser.kel",
        "rogue_ai_wander.kel",
        "rogue_ai_sleeper.kel",
        "rogue_ai_ranged.kel",
        "rogue_ai_fast.kel",
        "rogue_ai_smart.kel",
        "rogue_ai_boss.kel",
        "rogue_ai_tracker.kel",
        "rogue_item_potion.kel",
        "rogue_item_scroll.kel",
        "rogue_player_ai.kel",
        "rogue_combat.kel",
    ];
    let mut sources = Vec::with_capacity(names.len());
    for name in names {
        let path = format!("{}/{}", SCRIPT_DIR, name);
        match std::fs::read_to_string(&path) {
            Ok(s) => sources.push(s),
            Err(e) => {
                let mut w = world.lock().unwrap();
                w.push_message(format!("Reload failed: cannot read {}. {}", name, e));
                return;
            }
        }
    }
    let mut modules = Vec::with_capacity(names.len());
    for (i, src) in sources.iter().enumerate() {
        match build_module(src) {
            Ok(m) => modules.push(m),
            Err(e) => {
                let mut w = world.lock().unwrap();
                w.push_message(format!(
                    "Reload failed: {} did not compile. {}",
                    names[i], e
                ));
                return;
            }
        }
    }
    let mut drain = modules.into_iter();
    let dungen_module = drain.next().unwrap();
    let ai_modules = AiModules {
        idle: drain.next().unwrap(),
        chaser: drain.next().unwrap(),
        wander: drain.next().unwrap(),
        sleeper: drain.next().unwrap(),
        ranged: drain.next().unwrap(),
        fast: drain.next().unwrap(),
        smart: drain.next().unwrap(),
        boss: drain.next().unwrap(),
        tracker: drain.next().unwrap(),
        potion: drain.next().unwrap(),
        scroll: drain.next().unwrap(),
        player: drain.next().unwrap(),
        combat: drain.next().unwrap(),
    };

    // Swap the dungen virtual machine. The new module replaces
    // the old one in the same arena. Re-registering the natives
    // captures fresh world handles into the new closures.
    let new_dungen = match Vm::new(dungen_module, dungen_arena) {
        Ok(vm) => vm,
        Err(e) => {
            let mut w = world.lock().unwrap();
            w.push_message(format!("Reload failed: dungen verify error. {:?}", e));
            return;
        }
    };
    *dungen_vm = new_dungen;
    init_data_slots(dungen_vm);
    natives::register_natives(dungen_vm, world);

    // Swap every artificial-intelligence and item virtual
    // machine.
    {
        let mut pool = ai_pool.lock().unwrap();
        if let Err(e) = pool.reload(ai_modules, world) {
            let mut w = world.lock().unwrap();
            w.push_message(format!("Reload partial: {}", e));
            return;
        }
    }

    let mut w = world.lock().unwrap();
    w.push_message(String::from("Scripts reloaded from disk."));
}

/// Advance the player to the next floor. Level up first, then
/// invoke the dungeon generator. The exit on floor 100 is
/// handled by the turn dispatcher and never reaches this path.
fn descend_floor(
    world: &Arc<Mutex<World>>,
    dungen_vm: &mut Vm,
) -> Result<(), Box<dyn std::error::Error>> {
    {
        let mut w = world.lock().unwrap();
        // Level-up. Damage persists. The max-HP delta is added
        // to current HP so the newly added slots come in full
        // but pre-existing damage remains.
        let hp_gain = 3;
        let skill_gain = 1;
        w.player.level += 1;
        w.player.max_hp += hp_gain;
        w.player.hp += hp_gain;
        w.player.skill += skill_gain;
        w.floor += 1;
        let next_floor = w.floor;
        w.push_message(format!(
            "You descend to floor {}. You feel stronger.",
            next_floor
        ));
    }
    let next_floor = world.lock().unwrap().floor as i64;
    run_dungen(dungen_vm, next_floor)?;
    {
        let mut w = world.lock().unwrap();
        w.recompute_fov();
    }
    Ok(())
}
