//! Item tables. Weapon and armor stats load from
//! `rogue_gear.kel` at startup; names stay here as constant
//! parallel arrays. Potions and scrolls are static because
//! their effects are dispatched in `rogue_item_*.kel` scripts
//! and the host only carries identification metadata. The
//! per-run shuffled display names for potions and scrolls
//! live on `World` because they depend on the run seed.

use std::sync::OnceLock;

/// Top-level item category. Stored as `u8` on items in the world
/// to keep the item record narrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Food,
    Gold,
    Weapon,
    Armor,
    Potion,
    Scroll,
    Corpse,
}

impl ItemKind {
    pub fn as_u8(self) -> u8 {
        match self {
            ItemKind::Food => 0,
            ItemKind::Gold => 1,
            ItemKind::Weapon => 2,
            ItemKind::Armor => 3,
            ItemKind::Potion => 4,
            ItemKind::Scroll => 5,
            ItemKind::Corpse => 6,
        }
    }
}

// -- Weapons ---------------------------------------------------------

pub struct Weapon {
    pub name: &'static str,
    pub damage: i32,
}

pub struct Armor {
    pub name: &'static str,
    pub defense: i32,
}

/// Weapon names indexed by tier. The matching damage values
/// load from `rogue_gear.kel` at startup. Adding a tier needs
/// a name here and a `fn weapon(N)` head in the script.
pub const WEAPON_NAMES: [&str; 20] = [
    "fists", "rusty dagger", "short sword", "battle axe", "war hammer",
    "claymore", "halberd", "flamberge", "rune sword", "vorpal blade",
    "moonblade", "dragonbone spear", "starforged axe", "adamant flail",
    "voidshard", "demonbane", "soulrender", "god-piercer", "world-ender",
    "last word",
];

/// Armor names indexed by tier.
pub const ARMOR_NAMES: [&str; 20] = [
    "rags", "padded jacket", "leather armor", "studded leather", "ring mail",
    "chain mail", "scale mail", "splint mail", "plate mail", "rune plate",
    "mithril mail", "elven cuirass", "dragonscale", "dwarven plate",
    "celestial mail", "bulwark of ages", "adamantine plate",
    "aegis of the host", "starshield", "last guard",
];

static WEAPONS: OnceLock<Vec<Weapon>> = OnceLock::new();
static ARMORS: OnceLock<Vec<Armor>> = OnceLock::new();

pub fn install_weapons(damages: &[i32]) {
    let vec: Vec<Weapon> = WEAPON_NAMES
        .iter()
        .zip(damages.iter())
        .map(|(n, d)| Weapon {
            name: n,
            damage: *d,
        })
        .collect();
    let _ = WEAPONS.set(vec);
}

pub fn install_armors(defenses: &[i32]) {
    let vec: Vec<Armor> = ARMOR_NAMES
        .iter()
        .zip(defenses.iter())
        .map(|(n, d)| Armor {
            name: n,
            defense: *d,
        })
        .collect();
    let _ = ARMORS.set(vec);
}

pub fn weapons() -> &'static [Weapon] {
    WEAPONS.get().expect("weapons not loaded; call load_gear at startup")
}

pub fn armors() -> &'static [Armor] {
    ARMORS.get().expect("armors not loaded; call load_gear at startup")
}

// -- Potions ---------------------------------------------------------

/// Underlying potion effect. Hidden until first quaff. The display
/// name shown to the player is one of [`POTION_COLORS`], shuffled
/// per run.
#[derive(Clone, Copy, Debug)]
pub enum PotionEffect {
    Healing,
    GreaterHealing,
    Restoration,
    Poison,
    Acid,
    Strength,
    Skill,
    Speed,
    Levitation,
    SeeInvisible,
}

pub const POTION_EFFECTS: &[PotionEffect] = &[
    PotionEffect::Healing,
    PotionEffect::GreaterHealing,
    PotionEffect::Restoration,
    PotionEffect::Poison,
    PotionEffect::Acid,
    PotionEffect::Strength,
    PotionEffect::Skill,
    PotionEffect::Speed,
    PotionEffect::Levitation,
    PotionEffect::SeeInvisible,
];

/// Display colours used to disguise potions until identification.
/// One per [`POTION_EFFECTS`] entry. The host shuffles the mapping
/// at run start.
pub const POTION_COLORS: &[&str] = &[
    "blue", "red", "green", "amber", "violet", "milky", "fizzy", "smoky", "cloudy", "ruby",
];

pub fn potion_true_name(effect: PotionEffect) -> &'static str {
    match effect {
        PotionEffect::Healing => "healing",
        PotionEffect::GreaterHealing => "greater healing",
        PotionEffect::Restoration => "restoration",
        PotionEffect::Poison => "poison",
        PotionEffect::Acid => "acid",
        PotionEffect::Strength => "strength",
        PotionEffect::Skill => "skill",
        PotionEffect::Speed => "speed",
        PotionEffect::Levitation => "levitation",
        PotionEffect::SeeInvisible => "see invisible",
    }
}

// -- Scrolls ---------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum ScrollEffect {
    Identify,
    MagicMapping,
    Teleport,
    EnchantWeapon,
    EnchantArmor,
    RemoveCurse,
    Light,
    DetectMonsters,
    Sleep,
    Confusion,
}

pub const SCROLL_EFFECTS: &[ScrollEffect] = &[
    ScrollEffect::Identify,
    ScrollEffect::MagicMapping,
    ScrollEffect::Teleport,
    ScrollEffect::EnchantWeapon,
    ScrollEffect::EnchantArmor,
    ScrollEffect::RemoveCurse,
    ScrollEffect::Light,
    ScrollEffect::DetectMonsters,
    ScrollEffect::Sleep,
    ScrollEffect::Confusion,
];

pub const SCROLL_NAMES: &[&str] = &[
    "ZELGO MER",
    "JUYED AWK YACC",
    "NR 9",
    "XIXAXA XOXAXA XUXAXA",
    "PRATYAVAYAH",
    "DAIYEN FOOELS",
    "LEP GEX VEN ZEA",
    "PRIRUTSENIE",
    "ELBIB YLOH",
    "VERR YED HORRE",
];

pub fn scroll_true_name(effect: ScrollEffect) -> &'static str {
    match effect {
        ScrollEffect::Identify => "identify",
        ScrollEffect::MagicMapping => "magic mapping",
        ScrollEffect::Teleport => "teleport",
        ScrollEffect::EnchantWeapon => "enchant weapon",
        ScrollEffect::EnchantArmor => "enchant armor",
        ScrollEffect::RemoveCurse => "remove curse",
        ScrollEffect::Light => "light",
        ScrollEffect::DetectMonsters => "detect monsters",
        ScrollEffect::Sleep => "sleep",
        ScrollEffect::Confusion => "confusion",
    }
}
