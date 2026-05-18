//! Host native registration. Each Keleusma virtual machine that
//! the rogue example instantiates registers the natives this
//! module installs. Natives are partitioned by usage. Dungeon
//! generation needs map writes and entity spawns. Monster
//! artificial-intelligence scripts need monster and player
//! queries plus movement commit calls. Item-effect scripts need
//! player modifiers and identification.
//!
//! All natives operate on a shared `Arc<Mutex<World>>`. The host
//! locks the world for the duration of each native call. Native
//! calls are non-reentrant; scripts cannot recursively invoke
//! natives that would deadlock.

use std::sync::{Arc, Mutex};

use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmError};

use crate::ai::{AiAction, AiPool, AiPoolHandle};
use crate::bestiary;
use crate::combat;
use crate::items::{self, ItemKind};
use crate::world::{Tile, World};

pub type WorldHandle = Arc<Mutex<World>>;

/// Convenience: lock the world and push a single message line.
/// Used by host code that wants to log without holding the lock
/// across other work.
pub fn push_msg(world: &WorldHandle, msg: impl Into<String>) {
    world.lock().unwrap().push_message(msg);
}

/// Register the natives the dungeon-generation script needs. This
/// is also the set every other script can call when the host runs
/// them, so installing it once per virtual machine is sufficient.
pub fn register_natives(vm: &mut Vm, world: &WorldHandle) {
    register_rng(vm, world);
    register_map(vm, world);
    register_entities(vm, world);
    register_floor(vm, world);
}

/// Register the natives the game-tick script needs. The game
/// script is `loop main` and drives each tick by calling these
/// four natives in sequence. The natives manage their own lock
/// discipline so the world mutex is released before any nested
/// virtual-machine dispatch occurs.
pub fn register_game_natives(vm: &mut Vm, world: &WorldHandle, ai_pool: &AiPoolHandle) {
    register_run_player_turn(vm, world, ai_pool);
    register_monster_count(vm, world);
    register_run_monster_ai(vm, world, ai_pool);
    register_tick_book_keeping(vm, world, ai_pool);
}

// -- Game-tick natives ---------------------------------------------

/// `host::run_player_turn(cmd)` dispatches the player
/// artificial-intelligence script with the player's current
/// position and the supplied keypress, then applies the
/// returned action through the same resolver that handles
/// monster actions. Returns the outcome code yielded back to
/// the game-tick script.
fn register_run_player_turn(vm: &mut Vm, world: &WorldHandle, ai_pool: &AiPoolHandle) {
    let w = world.clone();
    let p = ai_pool.clone();
    vm.register_native_closure(
        "host::run_player_turn",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("run_player_turn", 1, args)?;
            let cmd = as_i64(&args[0])?;
            Ok(Value::Int(run_player_turn(&w, &p, cmd)))
        }),
    );
}

fn register_monster_count(vm: &mut Vm, world: &WorldHandle) {
    let w = world.clone();
    vm.register_native_closure(
        "host::monster_count",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("monster_count", 0, args)?;
            let world = w.lock().unwrap();
            Ok(Value::Int(world.monsters.len() as i64))
        }),
    );
}

fn register_run_monster_ai(vm: &mut Vm, world: &WorldHandle, ai_pool: &AiPoolHandle) {
    let w = world.clone();
    let p = ai_pool.clone();
    vm.register_native_closure(
        "host::run_monster_ai",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("run_monster_ai", 1, args)?;
            let idx = as_i64(&args[0])? as usize;
            run_one_monster_turn(&w, &p, idx);
            Ok(Value::Unit)
        }),
    );
}

fn register_tick_book_keeping(vm: &mut Vm, world: &WorldHandle, ai_pool: &AiPoolHandle) {
    let w = world.clone();
    let p = ai_pool.clone();
    vm.register_native_closure(
        "host::tick_book_keeping",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("tick_book_keeping", 0, args)?;
            Ok(Value::Int(tick_book_keeping(&w, &p)))
        }),
    );
}

// -- Game-tick helpers ----------------------------------------------

const REGEN_PERIOD: u64 = 10;
const STARVATION_DAMAGE_PER_TURN: i32 = 1;
const MAX_MONSTER_COUNT: usize = 24;

/// Outcome codes the host translates between native return
/// values and the higher-level result enum.
mod outcome {
    pub const CONTINUE: i64 = 0;
    pub const DESCENDED: i64 = 1;
    pub const WON: i64 = 2;
    pub const DIED: i64 = 3;
}

/// Dispatch the player artificial-intelligence script with the
/// player's current position and the supplied keypress, then
/// apply the returned action. The player and monsters travel
/// through the same per-actor dispatch shape; the player's
/// distinction is only the source of intent.
fn run_player_turn(world: &WorldHandle, ai_pool: &AiPoolHandle, cmd: i64) -> i64 {
    // Snapshot the player position without holding the world
    // lock across virtual-machine dispatch.
    let (mx, my) = {
        let w = world.lock().unwrap();
        (w.player.x, w.player.y)
    };
    let action = {
        let mut pool = ai_pool.lock().unwrap();
        pool.dispatch_player(mx, my, cmd)
    };
    let Ok(action) = action else {
        return outcome::CONTINUE;
    };
    apply_player_action(world, ai_pool, action)
}

/// Apply the player's chosen action. Move and melee both flow
/// through `handle_move_into_cell`. Descend, quaff, and read are
/// player-only action codes the monster archetypes do not emit.
fn apply_player_action(world: &WorldHandle, ai_pool: &AiPoolHandle, action: AiAction) -> i64 {
    match action {
        AiAction::Wait => outcome::CONTINUE,
        AiAction::MoveOrMelee { tx, ty } => handle_move_into_cell(world, ai_pool, tx, ty),
        AiAction::Ranged { .. } => outcome::CONTINUE,
        AiAction::Descend => handle_descend(world),
        AiAction::Quaff => handle_quaff(world, ai_pool),
        AiAction::Read => handle_read(world, ai_pool),
    }
}

fn handle_move_into_cell(world: &WorldHandle, ai_pool: &AiPoolHandle, tx: i32, ty: i32) -> i64 {
    enum MoveAction {
        Attack(usize),
        Step(i32, i32),
        Blocked,
    }
    // Pre-fetch the target cell information without holding the
    // world lock across script dispatch.
    let (tile_id, monster_idx) = {
        let w = world.lock().unwrap();
        let tile = match w.map.get(tx, ty) {
            Tile::Floor => 0,
            Tile::Wall => 1,
            Tile::DoorClosed => 2,
            Tile::DoorOpen => 3,
            Tile::StairsDown => 4,
            Tile::Exit => 5,
        };
        let monster = w.monsters.iter().position(|m| m.x == tx && m.y == ty);
        (tile, monster)
    };
    let action_code = {
        let mut pool = ai_pool.lock().unwrap();
        pool.dispatch_move_resolve(tile_id, if monster_idx.is_some() { 1 } else { 0 })
            .unwrap_or(0)
    };
    let decision = match action_code {
        2 => MoveAction::Attack(monster_idx.unwrap_or(0)),
        1 => MoveAction::Step(tx, ty),
        _ => MoveAction::Blocked,
    };
    match decision {
        MoveAction::Attack(idx) => {
            let mut pool = ai_pool.lock().unwrap();
            let mut w = world.lock().unwrap();
            combat::player_attacks(&mut w, &mut pool, idx);
        }
        MoveAction::Step(nx, ny) => {
            {
                let mut w = world.lock().unwrap();
                w.player.x = nx;
                w.player.y = ny;
            }
            // The autopickup driver acquires its own locks
            // because it dispatches the pickup-decision script.
            autopickup(world, ai_pool);
            let mut w = world.lock().unwrap();
            // Recompute the field of view immediately so the
            // monster turns this same tick can use the updated
            // visibility bitmap for the symmetric line-of-sight
            // rule.
            w.recompute_fov();
            // Auto-descend when the player steps onto stairs or
            // the floor-one-hundred exit tile.
            match w.map.get(nx, ny) {
                crate::world::Tile::StairsDown => return outcome::DESCENDED,
                crate::world::Tile::Exit => return outcome::WON,
                _ => {}
            }
        }
        MoveAction::Blocked => {}
    }
    outcome::CONTINUE
}

fn handle_descend(world: &WorldHandle) -> i64 {
    let w = world.lock().unwrap();
    match w.map.get(w.player.x, w.player.y) {
        Tile::StairsDown => outcome::DESCENDED,
        Tile::Exit => outcome::WON,
        _ => outcome::CONTINUE,
    }
}

fn handle_quaff(world: &WorldHandle, ai_pool: &AiPoolHandle) -> i64 {
    // Read the held slot without holding the world lock during
    // virtual-machine dispatch.
    let snapshot = {
        let w = world.lock().unwrap();
        w.player
            .potion_slot
            .map(|idx| (idx, w.player.hp, w.player.max_hp))
    };
    let Some((effect_idx, hp, max_hp)) = snapshot else {
        let mut w = world.lock().unwrap();
        w.push_message(String::from("You hold no potion."));
        return outcome::CONTINUE;
    };

    let result = {
        let mut pool = ai_pool.lock().unwrap();
        pool.dispatch_potion(effect_idx as i64, hp as i64, max_hp as i64)
    };
    let Ok((hp_delta, max_hp_delta, skill_delta, status_code, _status_arg)) = result else {
        return outcome::CONTINUE;
    };

    let mut w = world.lock().unwrap();
    let bit = 1u32 << effect_idx;
    let was_identified = (w.player.identified_potions & bit) != 0;
    w.player.identified_potions |= bit;
    w.player.potion_slot = None;
    apply_player_deltas(
        &mut w,
        hp_delta as i32,
        max_hp_delta as i32,
        skill_delta as i32,
    );
    apply_potion_status(&mut w, status_code);
    let true_name = items::potion_true_name(items::POTION_EFFECTS[effect_idx as usize]);
    if was_identified {
        w.push_message(format!("You quaff the potion of {}.", true_name));
    } else {
        w.push_message(format!(
            "You drink. The potion was {}. (Identified.)",
            true_name
        ));
    }
    outcome::CONTINUE
}

fn handle_read(world: &WorldHandle, ai_pool: &AiPoolHandle) -> i64 {
    let snapshot = {
        let w = world.lock().unwrap();
        w.player.scroll_slot
    };
    let Some(effect_idx) = snapshot else {
        let mut w = world.lock().unwrap();
        w.push_message(String::from("You hold no scroll."));
        return outcome::CONTINUE;
    };

    let result = {
        let mut pool = ai_pool.lock().unwrap();
        pool.dispatch_scroll(effect_idx as i64)
    };
    let Ok((hp_delta, max_hp_delta, skill_delta, status_code, status_arg)) = result else {
        return outcome::CONTINUE;
    };

    let mut w = world.lock().unwrap();
    let bit = 1u32 << effect_idx;
    let was_identified = (w.player.identified_scrolls & bit) != 0;
    w.player.identified_scrolls |= bit;
    w.player.scroll_slot = None;
    apply_player_deltas(
        &mut w,
        hp_delta as i32,
        max_hp_delta as i32,
        skill_delta as i32,
    );
    apply_scroll_status(&mut w, status_code, status_arg);
    let true_name = items::scroll_true_name(items::SCROLL_EFFECTS[effect_idx as usize]);
    if was_identified {
        w.push_message(format!("You read the scroll of {}.", true_name));
    } else {
        w.push_message(format!(
            "The scroll crumbles. It was {}. (Identified.)",
            true_name
        ));
    }
    outcome::CONTINUE
}

fn run_one_monster_turn(world: &WorldHandle, ai_pool: &AiPoolHandle, idx: usize) {
    let snapshot = {
        let w = world.lock().unwrap();
        if idx >= w.monsters.len() {
            return;
        }
        let m = &w.monsters[idx];
        let kind = bestiary::kind(m.kind as usize);
        Some((
            m.x,
            m.y,
            w.player.x,
            w.player.y,
            monster_sees_player(&w, m.x, m.y, w.player.x, w.player.y),
            kind.ai,
            matches!(kind.ai, bestiary::AiKind::Fast),
        ))
    };
    let Some((mx, my, px, py, sees, archetype, is_fast)) = snapshot else {
        return;
    };

    let actions = if is_fast { 2 } else { 1 };
    for _ in 0..actions {
        let snapshot2 = {
            let w = world.lock().unwrap();
            if idx >= w.monsters.len() {
                return;
            }
            let m = &w.monsters[idx];
            Some((
                m.x,
                m.y,
                w.player.x,
                w.player.y,
                monster_sees_player(&w, m.x, m.y, w.player.x, w.player.y),
            ))
        };
        let Some((mx2, my2, px2, py2, sees2)) = snapshot2 else {
            return;
        };
        let action = {
            let mut pool = ai_pool.lock().unwrap();
            pool.dispatch(archetype, mx2, my2, px2, py2, sees2)
        };
        let Ok(action) = action else {
            return;
        };
        {
            let mut pool = ai_pool.lock().unwrap();
            let mut w = world.lock().unwrap();
            apply_monster_action(&mut w, &mut pool, idx, action);
            if w.player.hp <= 0 {
                return;
            }
        }
        // Suppress unused warnings for the first read.
        let _ = (mx, my, px, py, sees);
    }
}

fn tick_book_keeping(world: &WorldHandle, ai_pool: &AiPoolHandle) -> i64 {
    let snapshot = {
        let mut w = world.lock().unwrap();
        w.player.turn += 1;
        (
            w.player.turn as i64,
            w.player.hp as i64,
            w.player.max_hp as i64,
            w.player.hunger as i64,
        )
    };
    let result = {
        let mut pool = ai_pool.lock().unwrap();
        pool.dispatch_book_keeping(snapshot.0, snapshot.1, snapshot.2, snapshot.3)
    };
    let Ok((new_hp, new_hunger)) = result else {
        return outcome::CONTINUE;
    };
    let mut w = world.lock().unwrap();
    let was_hungry = w.player.hunger == 0;
    w.player.hp = new_hp as i32;
    w.player.hunger = new_hunger as i32;
    if was_hungry && w.player.turn.is_multiple_of(5) {
        w.push_message(String::from("You are starving."));
    }
    w.recompute_fov();
    if w.player.hp <= 0 {
        w.push_message(String::from("You succumb to your wounds."));
        outcome::DIED
    } else {
        outcome::CONTINUE
    }
}

fn apply_player_deltas(w: &mut World, hp_delta: i32, max_hp_delta: i32, skill_delta: i32) {
    w.player.max_hp += max_hp_delta;
    if w.player.max_hp < 1 {
        w.player.max_hp = 1;
    }
    w.player.hp += hp_delta;
    if w.player.hp > w.player.max_hp {
        w.player.hp = w.player.max_hp;
    }
    w.player.skill += skill_delta;
}

fn apply_potion_status(w: &mut World, status_code: i64) {
    if status_code == 11 {
        w.player.hp = w.player.max_hp;
        w.push_message(String::from("Vigor floods through you."));
    }
}

fn apply_scroll_status(w: &mut World, status_code: i64, status_arg: i64) {
    match status_code {
        1 => {
            for e in w.explored.iter_mut() {
                *e = true;
            }
            w.push_message(String::from("The dungeon's layout fills your mind."));
        }
        2 => {
            for _ in 0..256 {
                let rx = (w.rng_next() % crate::MAP_W) as i32;
                let ry = (w.rng_next() % crate::MAP_H) as i32;
                if w.map.is_walkable(rx, ry) && !w.monsters.iter().any(|m| m.x == rx && m.y == ry) {
                    w.player.x = rx;
                    w.player.y = ry;
                    w.push_message(String::from("The world dissolves and reforms."));
                    // Note the landing tile so the player knows
                    // when they have arrived on stairs without
                    // moving. The auto-descend path requires a
                    // movement step; teleport bypasses that, so
                    // an explicit message closes the gap.
                    match w.map.get(rx, ry) {
                        crate::world::Tile::StairsDown => {
                            w.push_message(String::from("You stand on the stairs down."));
                        }
                        crate::world::Tile::Exit => {
                            w.push_message(String::from("You stand on the exit."));
                        }
                        _ => {}
                    }
                    return;
                }
            }
            w.push_message(String::from("The teleport fizzles."));
        }
        3 => {
            w.player.identified_potions = u32::MAX;
            w.push_message(String::from("All your potions reveal their natures."));
        }
        4 => {
            let new_tier = (w.player.weapon as i64 + status_arg)
                .clamp(0, (items::WEAPONS.len() as i64) - 1) as u8;
            w.player.weapon = new_tier;
            let name = items::WEAPONS[new_tier as usize].name;
            w.push_message(format!("Your weapon shimmers. You hold the {}.", name));
        }
        5 => {
            let new_tier = (w.player.armor as i64 + status_arg)
                .clamp(0, (items::ARMORS.len() as i64) - 1) as u8;
            w.player.armor = new_tier;
            let name = items::ARMORS[new_tier as usize].name;
            w.push_message(format!("Your armor hums. You wear the {}.", name));
        }
        6 => {
            let cx = w.player.x;
            let cy = w.player.y;
            for dy in -6..=6 {
                for dx in -6..=6 {
                    let x = cx + dx;
                    let y = cy + dy;
                    if x >= 0 && y >= 0 && (x as u32) < crate::MAP_W && (y as u32) < crate::MAP_H {
                        let idx = (y as u32 * crate::MAP_W + x as u32) as usize;
                        w.explored[idx] = true;
                    }
                }
            }
            w.push_message(String::from("A bright light flares around you."));
        }
        7 => {
            let n = w.monsters.len();
            w.push_message(format!("You sense {} creatures on this floor.", n));
        }
        _ => {
            // Status codes 8 (Sleep), 9 (Confusion), and 10
            // (Remove Curse) emitted by `rogue_item_scroll.kel`
            // fall through here. The scripts produce the codes
            // correctly; implementing the host-side effects is
            // Exercise 3.7 in the manual. The placeholder
            // message is intentionally vague so a future
            // implementation can replace it without script
            // changes.
            w.push_message(String::from("The scroll's magic dissipates harmlessly."));
        }
    }
}

fn apply_monster_action(w: &mut World, pool: &mut AiPool, idx: usize, action: AiAction) {
    if idx >= w.monsters.len() {
        return;
    }
    match action {
        AiAction::Wait => {}
        AiAction::MoveOrMelee { tx, ty } => {
            let mx = w.monsters[idx].x;
            let my = w.monsters[idx].y;
            let adx = (tx - mx).abs();
            let ady = (ty - my).abs();
            if adx > 1 || ady > 1 {
                return;
            }
            if tx == w.player.x && ty == w.player.y {
                combat::monster_attacks_player(w, pool, idx);
                return;
            }
            if w.monsters
                .iter()
                .enumerate()
                .any(|(i, m)| i != idx && m.x == tx && m.y == ty)
            {
                return;
            }
            if w.map.is_walkable(tx, ty) {
                w.monsters[idx].x = tx;
                w.monsters[idx].y = ty;
            }
        }
        AiAction::Ranged { tx, ty } => {
            if tx == w.player.x && ty == w.player.y {
                combat::monster_attacks_player(w, pool, idx);
            }
        }
        AiAction::Descend | AiAction::Quaff | AiAction::Read => {
            // Monsters do not produce these action codes. Player
            // actions are not routed through this resolver. The
            // arms exist to satisfy exhaustive matching.
        }
    }
}

/// Autopickup driver. The host fetches the tile item record,
/// gathers the inputs the pickup script needs, dispatches the
/// script for the decision, and then applies the appropriate
/// world mutation.
fn autopickup(world: &WorldHandle, ai_pool: &AiPoolHandle) {
    // Pre-fetch the item, current weapon damage, current armor
    // defense, and slot occupancy. Drop the world lock before
    // dispatching the pickup script.
    let snapshot = {
        let w = world.lock().unwrap();
        let px = w.player.x;
        let py = w.player.y;
        let idx = match w.items.iter().position(|it| it.x == px && it.y == py) {
            Some(i) => i,
            None => return,
        };
        let item = w.items[idx];
        let (new_value, current_value) = match item.kind {
            ItemKind::Weapon => {
                let nv = items::WEAPONS
                    .get(item.subtype as usize)
                    .map(|w| w.damage)
                    .unwrap_or(0);
                (nv as i64, w.player.weapon_damage() as i64)
            }
            ItemKind::Armor => {
                let nv = items::ARMORS
                    .get(item.subtype as usize)
                    .map(|a| a.defense)
                    .unwrap_or(0);
                (nv as i64, w.player.armor_value() as i64)
            }
            _ => (0, 0),
        };
        let slot_full = match item.kind {
            ItemKind::Potion => w.player.potion_slot.is_some(),
            ItemKind::Scroll => w.player.scroll_slot.is_some(),
            _ => false,
        };
        Some((idx, item, new_value, current_value, slot_full))
    };
    let Some((idx, item, new_value, current_value, slot_full)) = snapshot else {
        return;
    };
    let action = {
        let mut pool = ai_pool.lock().unwrap();
        pool.dispatch_pickup(
            item.kind.as_u8() as i64,
            new_value,
            current_value,
            if slot_full { 1 } else { 0 },
        )
        .unwrap_or(0)
    };
    let mut w = world.lock().unwrap();
    if action == 0 {
        // Leave the ground item; describe what is there if it is
        // a potion or a scroll the player cannot pocket.
        match item.kind {
            ItemKind::Potion => {
                let ground_name = potion_display_name(&w, item.subtype);
                let held_name = potion_display_name(&w, w.player.potion_slot.unwrap_or(0));
                w.push_message(format!(
                    "A {} lies here, but you already hold the {}.",
                    ground_name, held_name
                ));
            }
            ItemKind::Scroll => {
                let ground_name = scroll_display_name(&w, item.subtype);
                let held_name = scroll_display_name(&w, w.player.scroll_slot.unwrap_or(0));
                w.push_message(format!(
                    "A {} lies here, but you already hold the {}.",
                    ground_name, held_name
                ));
            }
            _ => {}
        }
        return;
    }
    // action == 1 (consume) or action == 2 (scrap)
    w.items.swap_remove(idx);
    if action == 2 {
        match item.kind {
            ItemKind::Weapon => {
                let name = items::WEAPONS
                    .get(item.subtype as usize)
                    .map(|w| w.name)
                    .unwrap_or("weapon");
                w.push_message(format!("You scrap the {}.", name));
            }
            ItemKind::Armor => {
                let name = items::ARMORS
                    .get(item.subtype as usize)
                    .map(|a| a.name)
                    .unwrap_or("armor");
                w.push_message(format!("You discard the {}.", name));
            }
            _ => {}
        }
        return;
    }
    // action == 1: apply per-kind consumption.
    match item.kind {
        ItemKind::Food => {
            let hunger = w.player.hunger + 40;
            w.player.hunger = hunger.min(w.player.max_hunger);
            w.push_message(String::from("You devour the ration. Your hunger ebbs."));
        }
        ItemKind::Gold => {
            let value = item.subtype as u32;
            w.player.gold += value;
            w.push_message(format!("You scoop up {} gold pieces.", value));
        }
        ItemKind::Weapon => {
            let new_idx = item.subtype as usize;
            if new_idx < items::WEAPONS.len() {
                let name = items::WEAPONS[new_idx].name;
                w.player.weapon = item.subtype;
                w.push_message(format!("You take up the {}.", name));
            }
        }
        ItemKind::Armor => {
            let new_idx = item.subtype as usize;
            if new_idx < items::ARMORS.len() {
                let name = items::ARMORS[new_idx].name;
                w.player.armor = item.subtype;
                w.push_message(format!("You don the {}.", name));
            }
        }
        ItemKind::Potion => {
            w.player.potion_slot = Some(item.subtype);
            let name = potion_display_name(&w, item.subtype);
            w.push_message(format!("You pocket the {}.", name));
        }
        ItemKind::Scroll => {
            w.player.scroll_slot = Some(item.subtype);
            let name = scroll_display_name(&w, item.subtype);
            w.push_message(format!("You pocket the {}.", name));
        }
        ItemKind::Corpse => {
            let kind = bestiary::kind(item.subtype as usize);
            let satiation = kind.corpse_satiation();
            let hp_delta = kind.corpse_hp_delta();
            let name = kind.name;
            if satiation > 0 {
                let hunger = w.player.hunger + satiation;
                w.player.hunger = hunger.min(w.player.max_hunger);
            }
            if hp_delta != 0 {
                w.player.hp += hp_delta;
                if w.player.hp > w.player.max_hp {
                    w.player.hp = w.player.max_hp;
                }
            }
            if hp_delta < 0 {
                w.push_message(format!(
                    "You force down the {} corpse. It sickens you.",
                    name
                ));
            } else if hp_delta > 0 {
                w.push_message(format!(
                    "The {} corpse restores you somehow.",
                    name
                ));
            } else {
                w.push_message(format!("You eat the {} corpse.", name));
            }
        }
    }
}

/// Compose a display name for a potion. Returns the true
/// name if the player has identified this effect, else the
/// per-run disguised colour name.
fn potion_display_name(w: &World, effect_idx: u8) -> String {
    let idx = effect_idx as usize;
    let bit = 1u32 << effect_idx;
    let identified = (w.player.identified_potions & bit) != 0;
    if identified {
        let effect = items::POTION_EFFECTS[idx];
        format!("potion of {}", items::potion_true_name(effect))
    } else {
        let appearance = w.potion_appearance[idx] as usize;
        let bottle = items::POTION_COLORS[appearance];
        format!("{} potion", bottle)
    }
}

/// Compose a display name for a scroll. Returns the true name
/// if the player has identified this effect, else the per-run
/// disguised mock-title.
fn scroll_display_name(w: &World, effect_idx: u8) -> String {
    let idx = effect_idx as usize;
    let bit = 1u32 << effect_idx;
    let identified = (w.player.identified_scrolls & bit) != 0;
    if identified {
        let effect = items::SCROLL_EFFECTS[idx];
        format!("scroll of {}", items::scroll_true_name(effect))
    } else {
        let appearance = w.scroll_appearance[idx] as usize;
        let label = items::SCROLL_NAMES[appearance];
        format!("scroll titled {}", label)
    }
}

/// Symmetric monster line-of-sight. The host treats the player's
/// field of view as the ground truth: if the player can see the
/// monster, the monster can see the player. This is the simplest
/// symmetric rule because the player's field of view is already
/// computed through recursive shadowcasting which is itself
/// symmetric on a grid with thick walls.
///
/// Exercise for the reader. Replace this with a per-monster
/// shadowcast originating at the monster's cell. The bound is
/// the same eight-tile radius the player uses. Compare the two
/// behaviours on dungeons with pillar-like wall configurations
/// where the symmetric-by-construction rule above can disagree
/// with an independent per-monster cast.
fn monster_sees_player(w: &World, mx: i32, my: i32, _px: i32, _py: i32) -> bool {
    w.visible_at(mx, my)
}

/// Bound on the `MAX_MONSTER_COUNT`. Exposed for tests and the
/// game-tick script's loop bound rationale.
pub const fn max_monster_count() -> usize {
    MAX_MONSTER_COUNT
}

// -- Random number generation ---------------------------------------

fn register_rng(vm: &mut Vm, world: &WorldHandle) {
    let w = world.clone();
    vm.register_native_closure(
        "host::rng_u32",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("rng_u32", 0, args)?;
            let mut world = w.lock().unwrap();
            Ok(Value::Int(world.rng_next() as i64))
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::rng_range",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("rng_range", 2, args)?;
            let lo = as_i64(&args[0])?;
            let hi = as_i64(&args[1])?;
            if hi <= lo {
                return Err(VmError::NativeError(format!(
                    "host::rng_range: hi {} must be greater than lo {}",
                    hi, lo
                )));
            }
            let mut world = w.lock().unwrap();
            let span = (hi - lo) as u32;
            let r = world.rng_next() % span;
            Ok(Value::Int(lo + r as i64))
        }),
    );
}

// -- Map access -----------------------------------------------------

fn register_map(vm: &mut Vm, world: &WorldHandle) {
    let w = world.clone();
    vm.register_native_closure(
        "host::map_set",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("map_set", 3, args)?;
            let x = as_i64(&args[0])? as i32;
            let y = as_i64(&args[1])? as i32;
            let tile_id = as_i64(&args[2])?;
            let tile = tile_from_id(tile_id).ok_or_else(|| {
                VmError::NativeError(format!("host::map_set: invalid tile id {}", tile_id))
            })?;
            let mut world = w.lock().unwrap();
            world.map.set(x, y, tile);
            Ok(Value::Unit)
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::map_get",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("map_get", 2, args)?;
            let x = as_i64(&args[0])? as i32;
            let y = as_i64(&args[1])? as i32;
            let world = w.lock().unwrap();
            Ok(Value::Int(tile_id(world.map.get(x, y))))
        }),
    );

    vm.register_native_closure(
        "host::map_w",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("map_w", 0, args)?;
            Ok(Value::Int(crate::MAP_W as i64))
        }),
    );

    vm.register_native_closure(
        "host::map_h",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("map_h", 0, args)?;
            Ok(Value::Int(crate::MAP_H as i64))
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::clear_floor",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("clear_floor", 0, args)?;
            let mut world = w.lock().unwrap();
            for y in 0..crate::MAP_H as i32 {
                for x in 0..crate::MAP_W as i32 {
                    world.map.set(x, y, Tile::Wall);
                }
            }
            world.monsters.clear();
            world.items.clear();
            // Visibility resets to fresh when the player is placed.
            for v in world.visible.iter_mut() {
                *v = false;
            }
            for e in world.explored.iter_mut() {
                *e = false;
            }
            Ok(Value::Unit)
        }),
    );
}

// -- Entity spawns and player placement -----------------------------

fn register_entities(vm: &mut Vm, world: &WorldHandle) {
    let w = world.clone();
    vm.register_native_closure(
        "host::place_player",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("place_player", 2, args)?;
            let x = as_i64(&args[0])? as i32;
            let y = as_i64(&args[1])? as i32;
            let mut world = w.lock().unwrap();
            world.player.x = x;
            world.player.y = y;
            world.recompute_fov();
            Ok(Value::Unit)
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::place_stairs",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("place_stairs", 2, args)?;
            let x = as_i64(&args[0])? as i32;
            let y = as_i64(&args[1])? as i32;
            let mut world = w.lock().unwrap();
            world.map.set(x, y, Tile::StairsDown);
            Ok(Value::Unit)
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::place_exit",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("place_exit", 2, args)?;
            let x = as_i64(&args[0])? as i32;
            let y = as_i64(&args[1])? as i32;
            let mut world = w.lock().unwrap();
            world.map.set(x, y, Tile::Exit);
            Ok(Value::Unit)
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::spawn_monster",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("spawn_monster", 3, args)?;
            let kind = as_i64(&args[0])? as u8;
            let x = as_i64(&args[1])? as i32;
            let y = as_i64(&args[2])? as i32;
            if (kind as usize) >= crate::bestiary::BESTIARY.len() {
                return Err(VmError::NativeError(format!(
                    "host::spawn_monster: kind {} out of range",
                    kind
                )));
            }
            let mut world = w.lock().unwrap();
            // Reject monster spawns on non-walkable tiles, on
            // the player's cell, and on cells already occupied
            // by another monster. Room overlap in the dungeon
            // generator can produce coordinates that fall in a
            // wall band; rather than push the constraint into
            // the script, the host silently drops the spawn.
            if !world.map.is_walkable(x, y) {
                return Ok(Value::Unit);
            }
            if x == world.player.x && y == world.player.y {
                return Ok(Value::Unit);
            }
            if world.monsters.iter().any(|m| m.x == x && m.y == y) {
                return Ok(Value::Unit);
            }
            world.spawn_monster(kind, x, y);
            Ok(Value::Unit)
        }),
    );

    let w = world.clone();
    vm.register_native_closure(
        "host::spawn_item",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("spawn_item", 4, args)?;
            let kind_id = as_i64(&args[0])?;
            let subtype = as_i64(&args[1])? as u8;
            let x = as_i64(&args[2])? as i32;
            let y = as_i64(&args[3])? as i32;
            let kind = item_kind_from_id(kind_id).ok_or_else(|| {
                VmError::NativeError(format!("host::spawn_item: invalid kind id {}", kind_id))
            })?;
            let mut world = w.lock().unwrap();
            // Reject placements that would hide the stairs or
            // exit, that fall in a wall band produced by room
            // overlap in the dungeon generator, that land on
            // the player's starting cell, or that collide with
            // a cell already holding an item.
            let tile = world.map.get(x, y);
            if matches!(
                tile,
                crate::world::Tile::Wall
                    | crate::world::Tile::DoorClosed
                    | crate::world::Tile::StairsDown
                    | crate::world::Tile::Exit
            ) {
                return Ok(Value::Unit);
            }
            if x == world.player.x && y == world.player.y {
                return Ok(Value::Unit);
            }
            if world.items.iter().any(|it| it.x == x && it.y == y) {
                return Ok(Value::Unit);
            }
            world.items.push(crate::world::Item {
                kind,
                subtype,
                x,
                y,
            });
            Ok(Value::Unit)
        }),
    );
}

// -- Floor metadata -------------------------------------------------

fn register_floor(vm: &mut Vm, world: &WorldHandle) {
    let w = world.clone();
    vm.register_native_closure(
        "host::floor",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            check_arity("floor", 0, args)?;
            let world = w.lock().unwrap();
            Ok(Value::Int(world.floor as i64))
        }),
    );
}

// -- Helpers --------------------------------------------------------

/// Tile identifiers exposed to scripts. The mapping is stable
/// across this example's lifetime so scripts can hard-code the
/// numeric form.
fn tile_id(t: Tile) -> i64 {
    match t {
        Tile::Floor => 0,
        Tile::Wall => 1,
        Tile::DoorClosed => 2,
        Tile::DoorOpen => 3,
        Tile::StairsDown => 4,
        Tile::Exit => 5,
    }
}

fn tile_from_id(id: i64) -> Option<Tile> {
    Some(match id {
        0 => Tile::Floor,
        1 => Tile::Wall,
        2 => Tile::DoorClosed,
        3 => Tile::DoorOpen,
        4 => Tile::StairsDown,
        5 => Tile::Exit,
        _ => return None,
    })
}

fn item_kind_from_id(id: i64) -> Option<ItemKind> {
    Some(match id {
        0 => ItemKind::Food,
        1 => ItemKind::Gold,
        2 => ItemKind::Weapon,
        3 => ItemKind::Armor,
        4 => ItemKind::Potion,
        5 => ItemKind::Scroll,
        _ => return None,
    })
}

fn as_i64(v: &Value) -> Result<i64, VmError> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!(
            "expected Word, got {}",
            other.type_name()
        ))),
    }
}

fn check_arity(name: &str, expected: usize, args: &[Value]) -> Result<(), VmError> {
    if args.len() != expected {
        return Err(VmError::NativeError(format!(
            "host::{}: expected {} argument(s), got {}",
            name,
            expected,
            args.len()
        )));
    }
    Ok(())
}
