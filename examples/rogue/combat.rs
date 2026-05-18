//! Combat resolution. The host samples the d20 roll, dispatches
//! the combat virtual machine for the hit and damage decision,
//! and applies the result. The rules live in
//! `examples/scripts/rogue/rogue_combat.kel`. This module is
//! the thin glue between the world state and the script.

use crate::ai::AiPool;
use crate::bestiary;
use crate::items::ItemKind;
use crate::world::{Item, World};

/// Result of an attack. Used by the message-log formatter and
/// by post-attack book keeping.
#[derive(Clone, Copy, Debug)]
pub struct AttackOutcome {
    pub hit: bool,
    pub damage: i32,
    pub critical: bool,
}

/// Resolve the player attacking the monster at `monster_idx`.
/// The monster's hit points are reduced by the damage dealt; if
/// the result is zero or less, the monster is removed from the
/// world and gold equal to its score is added to the player's
/// score-as-gold counter.
pub fn player_attacks(world: &mut World, ai: &mut AiPool, monster_idx: usize) -> AttackOutcome {
    let monster_kind_idx = world.monsters[monster_idx].kind as usize;
    let kind = bestiary::kind(monster_kind_idx);
    let roll = (world.rng_next() % 20) as i64 + 1;
    let (hit_kind, damage) = ai
        .dispatch_combat(
            world.player.skill as i64,
            world.player.weapon_damage() as i64,
            kind.evasion as i64,
            kind.armor as i64,
            roll,
        )
        .unwrap_or((0, 0));
    if hit_kind == 0 {
        return AttackOutcome {
            hit: false,
            damage: 0,
            critical: false,
        };
    }
    let damage = damage as i32;
    let critical = hit_kind == 2;
    let kind_name = kind.name;
    let score = kind.score;
    let new_hp = world.monsters[monster_idx].hp - damage;
    if new_hp <= 0 {
        let mx = world.monsters[monster_idx].x;
        let my = world.monsters[monster_idx].y;
        let drop_chance = kind.corpse_drop_chance;
        world.monsters.swap_remove(monster_idx);
        world.player.gold += score;
        world.push_message(format!("You slay the {}.", kind_name));
        if drop_chance > 0 {
            let roll = (world.rng_next() % 100) as u8;
            if roll < drop_chance {
                world.items.push(Item {
                    kind: ItemKind::Corpse,
                    subtype: monster_kind_idx as u8,
                    x: mx,
                    y: my,
                });
            }
        }
    } else {
        world.monsters[monster_idx].hp = new_hp;
        let label = if critical { "critical hit" } else { "hit" };
        world.push_message(format!(
            "You {} the {} for {} damage.",
            label, kind_name, damage
        ));
    }
    AttackOutcome {
        hit: true,
        damage,
        critical,
    }
}

/// Resolve a monster at `monster_idx` attacking the player. The
/// player's hit points are reduced. The caller checks for player
/// death.
pub fn monster_attacks_player(
    world: &mut World,
    ai: &mut AiPool,
    monster_idx: usize,
) -> AttackOutcome {
    let monster_kind_idx = world.monsters[monster_idx].kind as usize;
    let kind = bestiary::kind(monster_kind_idx);
    let roll = (world.rng_next() % 20) as i64 + 1;
    let (hit_kind, damage) = ai
        .dispatch_combat(
            kind.skill as i64,
            kind.damage as i64,
            world.player.level as i64,
            world.player.armor_value() as i64,
            roll,
        )
        .unwrap_or((0, 0));
    if hit_kind == 0 {
        let name = kind.name;
        world.push_message(format!("The {} misses.", name));
        return AttackOutcome {
            hit: false,
            damage: 0,
            critical: false,
        };
    }
    let damage = damage as i32;
    let critical = hit_kind == 2;
    world.player.hp -= damage;
    let name = kind.name;
    let label = if critical { "crits" } else { "hits" };
    world.push_message(format!("The {} {} you for {} damage.", name, label, damage));
    AttackOutcome {
        hit: true,
        damage,
        critical,
    }
}
