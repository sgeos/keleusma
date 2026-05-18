//! Bestiary. One hundred monster kinds organised easy-to-hard.
//! Stats and colours load at startup from
//! `examples/scripts/rogue/rogue_bestiary.kel`; the host caches
//! the resolved table behind a `OnceLock` so subsequent
//! accesses are cheap. Monster names stay in this file as a
//! constant array because Keleusma's data segment does not
//! currently support inline strings.

use std::sync::OnceLock;

/// Visual silhouette used by the sprite renderer. The renderer
/// fills the silhouette with the kind's primary colour and accent.
/// Twelve silhouettes provide enough visual variety for the full
/// roster without requiring per-kind artwork.
#[derive(Clone, Copy, Debug)]
pub enum Shape {
    Tiny,
    Small,
    Humanoid,
    Brute,
    Serpent,
    Insect,
    Skeleton,
    Mage,
    Ghost,
    Slime,
    Dragon,
    Boss,
}

impl Shape {
    /// Decode the enum from its ordinal. Out-of-range ordinals
    /// fall back to `Tiny` so a malformed script does not panic.
    pub fn from_ord(n: i64) -> Self {
        match n {
            1 => Shape::Small,
            2 => Shape::Humanoid,
            3 => Shape::Brute,
            4 => Shape::Serpent,
            5 => Shape::Insect,
            6 => Shape::Skeleton,
            7 => Shape::Mage,
            8 => Shape::Ghost,
            9 => Shape::Slime,
            10 => Shape::Dragon,
            11 => Shape::Boss,
            _ => Shape::Tiny,
        }
    }
}

/// Artificial-intelligence archetype. The host maps each archetype
/// to a Keleusma script name. Shared archetypes across multiple
/// kinds is intentional and was confirmed during the design
/// clarification.
#[derive(Clone, Copy, Debug)]
pub enum AiKind {
    Idle,
    Wander,
    Chaser,
    Smart,
    Sleeper,
    Ranged,
    Fast,
    Boss,
    Tracker,
    Hunter,
}

impl AiKind {
    pub fn from_ord(n: i64) -> Self {
        match n {
            1 => AiKind::Wander,
            2 => AiKind::Chaser,
            3 => AiKind::Smart,
            4 => AiKind::Sleeper,
            5 => AiKind::Ranged,
            6 => AiKind::Fast,
            7 => AiKind::Boss,
            8 => AiKind::Tracker,
            9 => AiKind::Hunter,
            _ => AiKind::Idle,
        }
    }
}

/// A single bestiary entry. Stats are read-only after load.
/// Per-monster mutable state (current hit points, position,
/// archetype-specific state) lives in [`crate::world::Monster`].
pub struct MonsterKind {
    pub name: &'static str,
    pub shape: Shape,
    pub primary: (u8, u8, u8),
    pub accent: (u8, u8, u8),
    pub max_hp: i32,
    pub skill: i32,
    pub evasion: i32,
    pub damage: i32,
    pub armor: i32,
    pub ai: AiKind,
    /// Lowest floor on which this kind appears as the floor's
    /// favourite. Random-pool spawns may still draw the kind on
    /// any floor at or beyond this depth.
    pub first_floor: u32,
    /// Experience-less progression. Kills add this much to the
    /// score in addition to any gold the kind drops.
    pub score: u32,
    /// Percent chance the monster drops a corpse when slain.
    /// Derived from `shape` by the bestiary script.
    pub corpse_drop_chance: u8,
    /// Hunger restored when the player eats this corpse.
    pub corpse_satiation: i32,
    /// Hit-point delta when the player eats this corpse.
    /// Negative values indicate a poisonous corpse.
    pub corpse_hp_delta: i32,
}

/// Runtime monster table. Loaded once at startup by the host
/// calling the bestiary script. Subsequent reads through
/// [`kind`] are cheap.
static BESTIARY: OnceLock<Vec<MonsterKind>> = OnceLock::new();

/// Install the loaded bestiary into the global table. Called
/// once at startup after the host has driven the bestiary
/// script through every monster id.
pub fn install(table: Vec<MonsterKind>) {
    let _ = BESTIARY.set(table);
}

/// Convenience accessor. Returns the bestiary entry for `kind`.
/// Panics if the table has not been installed.
pub fn kind(kind: usize) -> &'static MonsterKind {
    &table()[kind]
}

/// Read-only handle to the full table. Used by call sites that
/// previously walked the static `BESTIARY` array.
pub fn table() -> &'static Vec<MonsterKind> {
    BESTIARY.get().expect("bestiary not loaded; call install() at startup")
}

/// Total number of monster kinds in the table. Mirrors
/// `MONSTER_COUNT` in `rogue_bestiary.kel`.
pub const MONSTER_COUNT: usize = 100;

// Corpse stats moved into the bestiary script. Each entry's
// corpse_drop_chance, corpse_satiation, and corpse_hp_delta
// fields are filled by `corpse_fill(state.shape)` in
// `rogue_bestiary.kel` during the per-entry load. The shipped
// shape-to-stats mapping (Tiny corpses give little meat, dragon
// corpses give a lot, etc.) is now expressed in the script's
// `fn corpse_fill(N)` heads.
