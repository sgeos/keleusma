//! Item tables. Host-owned read-only definitions for the weapons,
//! armor, potions, and scrolls that drop on dungeon floors. Each
//! table is indexed by subtype identifier. The per-run shuffled
//! display names for potions and scrolls live on `World` because
//! they depend on the run seed.

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
        }
    }
}

// -- Weapons ---------------------------------------------------------

pub struct Weapon {
    pub name: &'static str,
    pub damage: i32,
}

pub const WEAPONS: &[Weapon] = &[
    Weapon {
        name: "fists",
        damage: 2,
    },
    Weapon {
        name: "rusty dagger",
        damage: 4,
    },
    Weapon {
        name: "short sword",
        damage: 7,
    },
    Weapon {
        name: "battle axe",
        damage: 10,
    },
    Weapon {
        name: "war hammer",
        damage: 14,
    },
    Weapon {
        name: "claymore",
        damage: 18,
    },
    Weapon {
        name: "halberd",
        damage: 22,
    },
    Weapon {
        name: "flamberge",
        damage: 26,
    },
    Weapon {
        name: "rune sword",
        damage: 30,
    },
    Weapon {
        name: "vorpal blade",
        damage: 36,
    },
];

// -- Armor -----------------------------------------------------------

pub struct Armor {
    pub name: &'static str,
    pub defense: i32,
}

pub const ARMORS: &[Armor] = &[
    Armor {
        name: "rags",
        defense: 0,
    },
    Armor {
        name: "padded jacket",
        defense: 1,
    },
    Armor {
        name: "leather armor",
        defense: 2,
    },
    Armor {
        name: "studded leather",
        defense: 3,
    },
    Armor {
        name: "ring mail",
        defense: 4,
    },
    Armor {
        name: "chain mail",
        defense: 5,
    },
    Armor {
        name: "scale mail",
        defense: 6,
    },
    Armor {
        name: "splint mail",
        defense: 7,
    },
    Armor {
        name: "plate mail",
        defense: 8,
    },
    Armor {
        name: "rune plate",
        defense: 10,
    },
];

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
