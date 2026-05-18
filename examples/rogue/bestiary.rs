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
}

/// Static name table indexed by monster id. Mirrored against
/// the `rogue_bestiary.kel` ordering. Adding a new monster
/// kind requires appending a name here and a matching `fill(N)`
/// head in the script.
pub const MONSTER_NAMES: [&str; 100] = [
    "Sewer Rat",
    "Giant Centipede",
    "Cave Bat",
    "Spotted Newt",
    "Cave Spider",
    "Mongrel Cur",
    "Brown Snake",
    "Gnoll Pup",
    "Restless Skeleton",
    "Kobold Sneak",
    "Goblin Sneak",
    "Giant Ant",
    "Giant Beetle",
    "Stirge",
    "Acolyte Zombie",
    "Hobbit Thug",
    "Hyena",
    "Gnome Sapper",
    "Pixie Stinger",
    "Giant Frog",
    "Goblin Warrior",
    "Orc Whelp",
    "Imp",
    "Wraithling",
    "Dire Wolf",
    "Werecat",
    "Giant Scorpion",
    "Hobgoblin Captain",
    "Ogre Whelp",
    "Carrion Crawler",
    "Gnoll Warlord",
    "Orc Captain",
    "Bugbear",
    "Werewolf",
    "Brown Mold",
    "Yellow Mold",
    "Black Pudding",
    "Gelatinous Cube",
    "Ogre",
    "Cave Bear",
    "Troll",
    "Young Hydra",
    "Iron Statue",
    "Salamander",
    "Cyclops",
    "Manticore",
    "Wyvern",
    "Lich Apprentice",
    "Wraith",
    "Specter",
    "Vampire Spawn",
    "Dire Bear",
    "Greater Imp",
    "Pit Fiend Whelp",
    "Stone Giant",
    "Cloud Giant Adept",
    "Frost Salamander",
    "Fire Drake",
    "Black Drake",
    "Green Drake",
    "White Drake",
    "Red Drake",
    "Iron Golem",
    "Lesser Demon Lord",
    "Death Knight",
    "Lich",
    "Storm Giant",
    "Beholder",
    "Fire Giant",
    "Frost Giant",
    "Young White Dragon",
    "Young Black Dragon",
    "Young Green Dragon",
    "Young Blue Dragon",
    "Young Red Dragon",
    "Bone Devil",
    "Mind Flayer",
    "Adult Black Dragon",
    "Adult Green Dragon",
    "Adult Blue Dragon",
    "Adult Red Dragon",
    "Adult White Dragon",
    "Ancient White Dragon",
    "Demon Lord",
    "Pit Fiend",
    "Beholder Tyrant",
    "Ancient Black Dragon",
    "Ancient Green Dragon",
    "Ancient Blue Dragon",
    "Ancient Red Dragon",
    "Tarrasque",
    "Empyrean",
    "Solar",
    "Lich King",
    "Ancient Wyrm",
    "Avatar of Destruction",
    "World Devourer",
    "Cosmic Horror",
    "Elder God Spawn",
    "Balrog Lord",
];

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

impl MonsterKind {
    /// Percent chance the monster drops a corpse when slain.
    /// Skeletons, ghosts, and slimes never leave a corpse.
    /// Dragons and bosses almost always do.
    pub fn corpse_drop_chance(&self) -> u8 {
        match self.shape {
            Shape::Tiny => 50,
            Shape::Small => 60,
            Shape::Humanoid => 55,
            Shape::Brute => 70,
            Shape::Serpent => 55,
            Shape::Insect => 45,
            Shape::Skeleton => 0,
            Shape::Mage => 40,
            Shape::Ghost => 0,
            Shape::Slime => 0,
            Shape::Dragon => 90,
            Shape::Boss => 100,
        }
    }

    /// Hunger restored when the player eats this corpse. Larger
    /// monsters carry more meat.
    pub fn corpse_satiation(&self) -> i32 {
        match self.shape {
            Shape::Tiny => 8,
            Shape::Small => 15,
            Shape::Humanoid => 25,
            Shape::Brute => 40,
            Shape::Serpent => 12,
            Shape::Insect => 6,
            Shape::Skeleton => 0,
            Shape::Mage => 20,
            Shape::Ghost => 0,
            Shape::Slime => 0,
            Shape::Dragon => 60,
            Shape::Boss => 80,
        }
    }

    /// Hit-point delta from eating this corpse. Negative values
    /// indicate the corpse is poisonous or rotten; the player
    /// can still eat it for the satiation, taking the damage as
    /// the cost. Cautious players lure such monsters away from
    /// chokepoints. Desperate players take the hit anyway.
    pub fn corpse_hp_delta(&self) -> i32 {
        match self.shape {
            Shape::Serpent => -4,
            Shape::Insect => -3,
            Shape::Mage => -1,
            Shape::Boss => 8,
            _ => 0,
        }
    }
}
