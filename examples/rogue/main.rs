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

use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

use crate::ai::{
    AiModules, AiPool, AiPoolHandle, compile_disk, compile_embedded, zero_data_slots,
};
use crate::input::Command;
use crate::natives::push_msg;
use crate::render::{GameOver, Renderer};
use crate::tiles::TileAtlas;
use crate::world::World;

/// Map dimensions in tiles. Sixty-four by forty matches a
/// sixteen-by-ten aspect ratio for the map area at sixteen-
/// pixel display tile size.
pub const MAP_W: u32 = 64;
pub const MAP_H: u32 = 40;

/// Display tile size in pixels. Each cell on the map renders
/// into this many pixels. Sixteen is chosen so the window fits
/// a twelve-eighty-pixel-wide display comfortably.
pub const TILE_PX: u32 = 16;

/// Native authoring size for procedural sprite art. The sprite
/// generators in `tiles.rs` draw at this resolution, and SDL3
/// downscales to `TILE_PX` on copy. Keeping the authoring size
/// larger than the display size preserves the original sprite
/// detail without forcing every primitive draw to rescale.
pub const SPRITE_PX: u32 = 24;

/// Pixel heights of the head-up display rows and the message
/// bar drawn below the map. The head-up display is split across
/// two rows. The top row is the hit-point bar by itself; the
/// player's hit-point cap grows with depth and at high floors
/// the pip strip stretches across the full window width. The
/// second row carries the gear icons, the floor ticks, the
/// floor and gold readouts, the held potion and scroll icons,
/// and the hunger gauge. The two-row split keeps each readout
/// at a fixed location regardless of how long the hit-point
/// bar runs.
pub const HP_BAR_PX: u32 = 24;
pub const INFO_BAR_PX: u32 = 24;
pub const HUD_PX: u32 = HP_BAR_PX + INFO_BAR_PX;
pub const MSG_PX: u32 = 24;

/// Total window dimensions derived from the map dimensions plus
/// the HUD row above and the message row below.
pub const WINDOW_W: u32 = MAP_W * TILE_PX;
pub const WINDOW_H: u32 = HUD_PX + MAP_H * TILE_PX + MSG_PX;

// Script loading and the `EMBEDDED` table live in `ai.rs` so the
// script-list source of truth sits next to the [`AiModules`]
// constructor. The host pulls `compile_embedded`, `compile_disk`,
// and `zero_data_slots` from there.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl_context = sdl3::init()?;
    let video_subsystem = sdl_context.video()?;

    // Add audio processing here. The `examples/piano_roll.rs`
    // example demonstrates the full SDL3 audio pipeline against
    // a Keleusma `loop main` score sequencer. The same pattern
    // would carry background music for the rogue example: open
    // the audio device, share a voice array under an
    // `Arc<Mutex<_>>` with an SDL3 audio callback, and drive
    // note triggers from a `rogue_music.kel` script per turn or
    // per floor. See Exercise 2.6 in `docs/guide/ROGUE.md`.

    let window = video_subsystem
        .window("Keleusma Roguelike (work in progress)", WINDOW_W, WINDOW_H)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas();
    let texture_creator = canvas.texture_creator();

    // Load the bestiary by running `rogue_bestiary.kel` once
    // per monster id and reading the data segment. The
    // resolved table is installed in the global `OnceLock`
    // behind `bestiary::install` and read through
    // `bestiary::kind` thereafter. Run before `TileAtlas::build`
    // because the atlas iterates the bestiary to render
    // per-monster sprites.
    load_bestiary()?;
    // Load weapon and armor stats from `rogue_gear.kel`. The
    // script holds two tables sharing the same data-segment
    // shape; the host iterates each table by tier index.
    load_gear()?;

    let mut atlas = TileAtlas::build(&mut canvas, &texture_creator)?;

    // Shared world state. The dungeon generator, artificial-
    // intelligence pool, and turn dispatcher all mutate this
    // through Arc<Mutex<_>>.
    let world: natives::WorldHandle = Arc::new(Mutex::new(World::new()));

    // Build the dungeon-generation virtual machine.
    let dungen_arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let dungen_module = compile_embedded("rogue_dungen.kel")?;
    let mut dungen_vm =
        Vm::new(dungen_module, &dungen_arena).map_err(|e| format!("vm new: {:?}", e))?;
    zero_data_slots(&mut dungen_vm);
    natives::register_natives(&mut dungen_vm, &world);

    // Build the artificial-intelligence virtual-machine pool.
    // The pool owns its arenas through `Box::leak` so it can be
    // wrapped in `Arc<Mutex<_>>` and shared with native closures.
    let ai_modules = AiModules::build(compile_embedded)?;
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
    let game_module = compile_embedded("rogue_game.kel")?;
    let mut game_vm = Vm::new(game_module, &game_arena).map_err(|e| format!("vm new: {:?}", e))?;
    zero_data_slots(&mut game_vm);
    natives::register_game_natives(&mut game_vm, &world, &ai_pool);
    let mut game_started = false;

    // Generate the first floor.
    run_dungen(&mut dungen_vm, 1)?;
    push_msg(&world, "Welcome, brave adventurer.");
    world.lock().unwrap().recompute_fov();

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
                        match keycode {
                            sdl3::keyboard::Keycode::R => {
                                restart_run(&world, &mut dungen_vm, &ai_pool)?;
                                game_over = None;
                            }
                            sdl3::keyboard::Keycode::Q | sdl3::keyboard::Keycode::Escape => {
                                break 'running;
                            }
                            _ => {}
                        }
                        continue;
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
                                        descend_floor(&world, &mut dungen_vm, &ai_pool)?;
                                    }
                                    2 => {
                                        push_msg(&world, "You step into the light. You win!");
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

/// Reset the world to a fresh run starting on floor one. The
/// game-tick virtual machine is left at its current yield point
/// because the loop body re-reads world state through natives
/// every iteration, so the next user input resumes against the
/// fresh world correctly. The per-archetype loop-main data
/// segments are zeroed so monster memory from the previous run
/// does not bleed into the new run.
fn restart_run<'a1, 'b1>(
    world: &natives::WorldHandle,
    dungen_vm: &mut Vm<'a1, 'b1>,
    ai_pool: &ai::AiPoolHandle,
) -> Result<(), Box<dyn std::error::Error>>
where
    'b1: 'a1,
{
    *world.lock().unwrap() = World::new();
    push_msg(world, "A new dungeon spreads before you.");
    run_dungen(dungen_vm, 1)?;
    world.lock().unwrap().recompute_fov();
    ai_pool.lock().unwrap().reset_loop_main_data();
    Ok(())
}

/// Load the bestiary from the Keleusma script. The script
/// owns the per-kind numeric data; the host owns the parallel
/// name table. Loading is a single startup pass that calls the
/// script once per monster id and reads the data segment.
///
/// The discovery step calls the script with `-1` first to read
/// the last entry's `id` field, which equals
/// `MONSTER_COUNT - 1`. This demonstrates the negative-index
/// convention even though `bestiary::MONSTER_NAMES.len()`
/// happens to mirror the value on the host side; an assertion
/// catches drift between the script and the name table.
fn load_bestiary() -> Result<(), Box<dyn std::error::Error>> {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let module = compile_embedded("rogue_bestiary.kel")?;
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("bestiary vm new: {:?}", e))?;
    zero_data_slots(&mut vm);
    // Discovery: ask the script for the last entry.
    vm.call(&[Value::Int(-1)])
        .map_err(|e| format!("bestiary discovery: {:?}", e))?;
    let last_id = read_data_int(&vm, 0)? as usize;
    let count = last_id + 1;
    assert_eq!(
        count,
        bestiary::MONSTER_COUNT,
        "bestiary script reports {} entries but MONSTER_COUNT is {}",
        count,
        bestiary::MONSTER_COUNT
    );
    // Fill the runtime table. The script's `fn main` returns
    // the entry's name as a `Text`; the host extracts it from
    // the call's `Finished` payload and leaks it for the
    // program's lifetime.
    let mut table = Vec::with_capacity(count);
    for i in 0..count {
        let state = vm
            .call(&[Value::Int(i as i64)])
            .map_err(|e| format!("bestiary entry {}: {:?}", i, e))?;
        let name = leak_finished_static_str(state, i)?;
        table.push(read_bestiary_entry(&vm, name)?);
    }
    bestiary::install(table);
    Ok(())
}

fn leak_finished_static_str(
    state: VmState,
    id: usize,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    match state {
        VmState::Finished(Value::StaticStr(s)) => Ok(Box::leak(s.into_boxed_str())),
        other => Err(format!("bestiary entry {} returned non-string: {:?}", id, other).into()),
    }
}

fn read_data_int(vm: &Vm, slot: usize) -> Result<i64, Box<dyn std::error::Error>> {
    match vm
        .get_data(slot)
        .map_err(|e| format!("get_data({}): {:?}", slot, e))?
    {
        Value::Int(n) => Ok(*n),
        other => Err(format!("expected Int at slot {}, got {:?}", slot, other).into()),
    }
}

fn read_bestiary_entry(
    vm: &Vm,
    name: &'static str,
) -> Result<bestiary::MonsterKind, Box<dyn std::error::Error>> {
    // Slot order mirrors the data-segment declaration in
    // `rogue_bestiary.kel`. The host reads each slot, decodes
    // enums by ordinal, and copies the rest as integers. The
    // `name` argument is the script's return value for this
    // entry, leaked to `&'static str` by the caller.
    let r = |s| read_data_int(vm, s);
    Ok(bestiary::MonsterKind {
        name,
        shape: bestiary::Shape::from_ord(r(1)?),
        primary: (r(2)? as u8, r(3)? as u8, r(4)? as u8),
        accent: (r(5)? as u8, r(6)? as u8, r(7)? as u8),
        max_hp: r(8)? as i32,
        skill: r(9)? as i32,
        evasion: r(10)? as i32,
        damage: r(11)? as i32,
        armor: r(12)? as i32,
        ai: bestiary::AiKind::from_ord(r(13)?),
        first_floor: r(14)? as u32,
        score: r(15)? as u32,
        corpse_drop_chance: r(16)? as u8,
        corpse_satiation: r(17)? as i32,
        corpse_hp_delta: r(18)? as i32,
    })
}

/// Load weapon damages and armor defenses from `rogue_gear.kel`.
/// Two tables share one script; the host calls each with
/// `(table, tier)` and reads the `value` slot. Names live in
/// `items::WEAPON_NAMES` and `items::ARMOR_NAMES`.
fn load_gear() -> Result<(), Box<dyn std::error::Error>> {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let module = compile_embedded("rogue_gear.kel")?;
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("gear vm new: {:?}", e))?;
    zero_data_slots(&mut vm);
    let weapon_count = load_gear_table(&mut vm, 0)?;
    let mut damages = Vec::with_capacity(weapon_count);
    for i in 0..weapon_count {
        vm.call(&[Value::Int(0), Value::Int(i as i64)])
            .map_err(|e| format!("gear weapon {}: {:?}", i, e))?;
        damages.push(read_data_int(&vm, 1)? as i32);
    }
    items::install_weapons(&damages);
    let armor_count = load_gear_table(&mut vm, 1)?;
    let mut defenses = Vec::with_capacity(armor_count);
    for i in 0..armor_count {
        vm.call(&[Value::Int(1), Value::Int(i as i64)])
            .map_err(|e| format!("gear armor {}: {:?}", i, e))?;
        defenses.push(read_data_int(&vm, 1)? as i32);
    }
    items::install_armors(&defenses);
    Ok(())
}

fn load_gear_table(vm: &mut Vm, table: i64) -> Result<usize, Box<dyn std::error::Error>> {
    vm.call(&[Value::Int(table), Value::Int(-1)])
        .map_err(|e| format!("gear discovery table {}: {:?}", table, e))?;
    Ok(read_data_int(vm, 0)? as usize + 1)
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
    let dungen_module = match compile_disk("rogue_dungen.kel") {
        Ok(m) => m,
        Err(e) => return push_msg(world, format!("Reload failed: {}", e)),
    };
    let ai_modules = match AiModules::build(compile_disk) {
        Ok(m) => m,
        Err(e) => return push_msg(world, format!("Reload failed: {}", e)),
    };
    // Swap the dungen virtual machine. The new module replaces
    // the old one in the same arena. Re-registering the natives
    // captures fresh world handles into the new closures.
    *dungen_vm = match Vm::new(dungen_module, dungen_arena) {
        Ok(vm) => vm,
        Err(e) => return push_msg(world, format!("Reload failed: dungen verify error. {:?}", e)),
    };
    zero_data_slots(dungen_vm);
    natives::register_natives(dungen_vm, world);
    // Swap every artificial-intelligence and item virtual
    // machine.
    if let Err(e) = ai_pool.lock().unwrap().reload(ai_modules, world) {
        return push_msg(world, format!("Reload partial: {}", e));
    }
    push_msg(world, "Scripts reloaded from disk.");
}

/// Advance the player to the next floor. Level up first, then
/// invoke the dungeon generator. The exit on floor 100 is
/// handled by the turn dispatcher and never reaches this path.
/// The level-up arithmetic lives in `rogue_descend.kel`.
fn descend_floor(
    world: &Arc<Mutex<World>>,
    dungen_vm: &mut Vm,
    ai_pool: &AiPoolHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = {
        let w = world.lock().unwrap();
        (
            w.player.level as i64,
            w.player.max_hp as i64,
            w.player.hp as i64,
            w.player.skill as i64,
            w.floor as i64,
        )
    };
    let (level, max_hp, hp, skill, floor) = ai_pool
        .lock()
        .unwrap()
        .dispatch_descend(snapshot.0, snapshot.1, snapshot.2, snapshot.3, snapshot.4)?;
    {
        let mut w = world.lock().unwrap();
        w.player.level = level as u32;
        w.player.max_hp = max_hp as i32;
        w.player.hp = hp as i32;
        w.player.skill = skill as i32;
        w.floor = floor as u32;
    }
    push_msg(
        world,
        format!("You descend to floor {}. You feel stronger.", floor),
    );
    run_dungen(dungen_vm, floor)?;
    world.lock().unwrap().recompute_fov();
    Ok(())
}
