//! Host-owned world model. Map, player, monsters, items, message
//! log, field-of-view buffers, identification state, and the
//! per-run randomised display mappings for potions and scrolls.

use std::collections::VecDeque;

use crate::bestiary;
use crate::fov;
use crate::items::{self, ItemKind};
use crate::{MAP_H, MAP_W};

/// Field-of-view radius in tiles. Matches the design default.
pub const FOV_RADIUS: i32 = 8;

/// Static tile classes the renderer knows how to draw. Variants
/// beyond the scaffold demo are declared up front so the renderer
/// and the sprite atlas can be wired to the full set in this
/// phase. Subsequent phases populate maps with the additional
/// tile kinds during dungeon generation.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Floor,
    Wall,
    DoorClosed,
    DoorOpen,
    StairsDown,
    Exit,
}

/// Two-dimensional tile grid, stored row-major.
pub struct Map {
    cells: Vec<Tile>,
}

impl Map {
    pub fn new() -> Self {
        let total = (MAP_W * MAP_H) as usize;
        Self {
            cells: vec![Tile::Wall; total],
        }
    }

    pub fn get(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
            return Tile::Wall;
        }
        self.cells[(y as u32 * MAP_W + x as u32) as usize]
    }

    pub fn set(&mut self, x: i32, y: i32, t: Tile) {
        if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
            return;
        }
        self.cells[(y as u32 * MAP_W + x as u32) as usize] = t;
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        matches!(
            self.get(x, y),
            Tile::Floor | Tile::DoorOpen | Tile::StairsDown | Tile::Exit
        )
    }

    pub fn is_transparent(&self, x: i32, y: i32) -> bool {
        matches!(
            self.get(x, y),
            Tile::Floor | Tile::DoorOpen | Tile::StairsDown | Tile::Exit
        )
    }
}

/// Sentinel kind value indicating the player rather than a
/// bestiary monster kind.
pub const PLAYER_KIND: i32 = -1;

/// Unified actor record. Player and monster share this struct.
/// Fields that only matter for one role are zero or defaulted
/// for the other. Memory overhead per monster is roughly eighty
/// bytes; the example caps active monsters at twenty-four, so
/// the waste is negligible. The conceptual win is symmetry:
/// player and monsters travel through the same code paths with
/// the same field names.
#[derive(Clone)]
pub struct Actor {
    /// `PLAYER_KIND` for the player, otherwise an index into
    /// `bestiary::BESTIARY`.
    pub kind: i32,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub skill: i32,
    /// Equipped weapon tier. Unused for monsters.
    pub weapon: u8,
    /// Equipped armor tier. Unused for monsters.
    pub armor: u8,
    pub level: u32,
    pub hunger: i32,
    pub max_hunger: i32,
    pub gold: u32,
    pub potion_slot: Option<u8>,
    pub scroll_slot: Option<u8>,
    pub turn: u64,
    /// One bit per potion effect index, set when the effect has
    /// been identified. Player-only.
    pub identified_potions: u32,
    pub identified_scrolls: u32,
    /// Artificial-intelligence-archetype state slot for
    /// monsters. Unused for the player; the player's intent
    /// comes from the keyboard.
    pub state: u32,
}

impl Actor {
    pub fn new_player(x: i32, y: i32) -> Self {
        Self {
            kind: PLAYER_KIND,
            x,
            y,
            hp: 12,
            max_hp: 12,
            skill: 0,
            weapon: 1,
            armor: 0,
            level: 1,
            hunger: 100,
            max_hunger: 100,
            gold: 0,
            potion_slot: None,
            scroll_slot: None,
            turn: 0,
            identified_potions: 0,
            identified_scrolls: 0,
            state: 0,
        }
    }

    pub fn new_monster(kind: u8, x: i32, y: i32, max_hp: i32) -> Self {
        Self {
            kind: kind as i32,
            x,
            y,
            hp: max_hp,
            max_hp,
            skill: 0,
            weapon: 0,
            armor: 0,
            level: 0,
            hunger: 0,
            max_hunger: 0,
            gold: 0,
            potion_slot: None,
            scroll_slot: None,
            turn: 0,
            identified_potions: 0,
            identified_scrolls: 0,
            state: 0,
        }
    }

    pub fn is_player(&self) -> bool {
        self.kind == PLAYER_KIND
    }

    pub fn weapon_damage(&self) -> i32 {
        items::WEAPONS[self.weapon as usize].damage
    }

    pub fn armor_value(&self) -> i32 {
        items::ARMORS[self.armor as usize].defense
    }
}

/// Type alias kept so historical call sites that reference
/// `Player` and `Monster` read naturally. The underlying record
/// is the same.
pub type Player = Actor;
pub type Monster = Actor;

/// Per-item world record. Subtype indexes into the table that
/// matches `kind`. Gold uses `subtype` as the pile value.
#[derive(Clone, Copy)]
pub struct Item {
    pub kind: ItemKind,
    pub subtype: u8,
    pub x: i32,
    pub y: i32,
}

/// Top-level world container.
pub struct World {
    pub map: Map,
    pub player: Player,
    pub monsters: Vec<Monster>,
    pub items: Vec<Item>,
    pub floor: u32,
    pub messages: VecDeque<String>,

    /// Per-cell visibility flag for this turn. Sized to the map.
    pub visible: Vec<bool>,
    /// Per-cell cumulative explored flag. Sized to the map.
    pub explored: Vec<bool>,

    /// Per-run mapping from potion-effect index to display-colour
    /// index. Shuffled at run start to disguise effect identity.
    pub potion_appearance: [u8; items::POTION_EFFECTS.len()],
    /// Per-run mapping from scroll-effect index to display-name
    /// index. Shuffled at run start.
    pub scroll_appearance: [u8; items::SCROLL_EFFECTS.len()],

    /// Simple xorshift RNG state. Host-owned so deterministic
    /// reseeding is possible per floor.
    rng_state: u32,
}

impl World {
    /// Build an empty world. The map is solid wall, the player is
    /// at a sentinel position, no monsters or items exist. The
    /// dungeon-generation script is expected to populate the
    /// world on the first turn.
    ///
    /// The random number generator is seeded from the current
    /// system time so each run produces a different dungeon and
    /// a different potion or scroll appearance shuffle. The
    /// seed is mixed with a golden-ratio constant to keep the
    /// xorshift state nonzero even when the time component is
    /// small.
    pub fn new() -> Self {
        let total = (MAP_W * MAP_H) as usize;
        let mut world = Self {
            map: Map::new(),
            player: Actor::new_player(1, 1),
            monsters: Vec::new(),
            items: Vec::new(),
            floor: 1,
            messages: VecDeque::new(),
            visible: vec![false; total],
            explored: vec![false; total],
            potion_appearance: identity_appearance(items::POTION_EFFECTS.len()),
            scroll_appearance: identity_appearance(items::SCROLL_EFFECTS.len()),
            rng_state: seed_from_time(),
        };
        world.shuffle_appearances();
        world
    }

    pub fn new_demo() -> Self {
        let total = (MAP_W * MAP_H) as usize;
        let mut map = Map::new();
        let room_x0 = 6_i32;
        let room_y0 = 4_i32;
        let room_x1 = 30_i32;
        let room_y1 = 18_i32;
        for y in room_y0..=room_y1 {
            for x in room_x0..=room_x1 {
                let on_edge = x == room_x0 || x == room_x1 || y == room_y0 || y == room_y1;
                map.set(x, y, if on_edge { Tile::Wall } else { Tile::Floor });
            }
        }
        map.set(room_x1 - 1, room_y1 - 1, Tile::StairsDown);

        // A second room east of the first, connected by a corridor.
        let r2_x0 = 40_i32;
        let r2_y0 = 8_i32;
        let r2_x1 = 60_i32;
        let r2_y1 = 16_i32;
        for y in r2_y0..=r2_y1 {
            for x in r2_x0..=r2_x1 {
                let on_edge = x == r2_x0 || x == r2_x1 || y == r2_y0 || y == r2_y1;
                map.set(x, y, if on_edge { Tile::Wall } else { Tile::Floor });
            }
        }
        // Corridor at row 12.
        for x in room_x1..=r2_x0 {
            map.set(x, 12, Tile::Floor);
        }
        // Doors at corridor entry and exit.
        map.set(room_x1, 12, Tile::DoorClosed);
        map.set(r2_x0, 12, Tile::DoorClosed);

        let mut world = Self {
            map,
            player: Actor::new_player(room_x0 + 2, room_y0 + 2),
            monsters: Vec::new(),
            items: Vec::new(),
            floor: 1,
            messages: VecDeque::new(),
            visible: vec![false; total],
            explored: vec![false; total],
            potion_appearance: identity_appearance(items::POTION_EFFECTS.len()),
            scroll_appearance: identity_appearance(items::SCROLL_EFFECTS.len()),
            rng_state: seed_from_time(),
        };
        world.shuffle_appearances();

        // Demo monsters for visual testing.
        world.spawn_monster(0, 14, 8);
        world.spawn_monster(2, 20, 14);
        world.spawn_monster(8, 48, 11);
        world.spawn_monster(13, 55, 13);

        // Demo items for visual testing.
        world.items.push(Item {
            kind: ItemKind::Food,
            subtype: 0,
            x: 12,
            y: 10,
        });
        world.items.push(Item {
            kind: ItemKind::Potion,
            subtype: 0,
            x: 18,
            y: 6,
        });
        world.items.push(Item {
            kind: ItemKind::Scroll,
            subtype: 0,
            x: 22,
            y: 14,
        });
        world.items.push(Item {
            kind: ItemKind::Weapon,
            subtype: 3,
            x: 45,
            y: 12,
        });
        world.items.push(Item {
            kind: ItemKind::Armor,
            subtype: 2,
            x: 52,
            y: 14,
        });
        world.items.push(Item {
            kind: ItemKind::Gold,
            subtype: 25,
            x: 50,
            y: 10,
        });

        world.push_message(String::from("Welcome to the dungeon."));
        world.recompute_fov();
        world
    }

    pub fn recompute_fov(&mut self) {
        fov::compute(
            &self.map,
            self.player.x,
            self.player.y,
            FOV_RADIUS,
            &mut self.visible,
        );
        for i in 0..self.visible.len() {
            if self.visible[i] {
                self.explored[i] = true;
            }
        }
    }

    pub fn visible_at(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
            return false;
        }
        self.visible[(y as u32 * MAP_W + x as u32) as usize]
    }

    pub fn explored_at(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
            return false;
        }
        self.explored[(y as u32 * MAP_W + x as u32) as usize]
    }

    pub fn spawn_monster(&mut self, kind: u8, x: i32, y: i32) {
        let entry = bestiary::kind(kind as usize);
        self.monsters
            .push(Actor::new_monster(kind, x, y, entry.max_hp));
    }

    pub fn push_message<S: Into<String>>(&mut self, s: S) {
        self.messages.push_back(s.into());
        while self.messages.len() > 32 {
            self.messages.pop_front();
        }
    }

    pub fn latest_message(&self) -> Option<&str> {
        self.messages.back().map(String::as_str)
    }

    fn shuffle_appearances(&mut self) {
        // Fisher-Yates shuffle.
        for i in (1..self.potion_appearance.len()).rev() {
            let j = (self.rng_next() as usize) % (i + 1);
            self.potion_appearance.swap(i, j);
        }
        for i in (1..self.scroll_appearance.len()).rev() {
            let j = (self.rng_next() as usize) % (i + 1);
            self.scroll_appearance.swap(i, j);
        }
    }

    pub fn rng_next(&mut self) -> u32 {
        let mut s = self.rng_state;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.rng_state = s;
        s
    }
}

fn identity_appearance(n: usize) -> [u8; items::POTION_EFFECTS.len()] {
    let mut out = [0u8; items::POTION_EFFECTS.len()];
    debug_assert_eq!(items::POTION_EFFECTS.len(), items::SCROLL_EFFECTS.len());
    for i in 0..n.min(out.len()) {
        out[i] = i as u8;
    }
    out
}

/// Build an xorshift seed from the current system time. The
/// seed combines the nanosecond and second components of the
/// Unix epoch through an exclusive-or and mixes in a
/// golden-ratio constant to keep the resulting state nonzero
/// even when one component is small. Falls back to the
/// golden-ratio constant on the unlikely error path so the
/// generator never starts at zero.
fn seed_from_time() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    const GOLDEN: u32 = 0x9E37_79B9;
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => {
            let nanos = d.subsec_nanos();
            let secs = d.as_secs() as u32;
            let mut s = nanos ^ secs.rotate_left(13) ^ GOLDEN;
            if s == 0 {
                s = GOLDEN;
            }
            s
        }
        Err(_) => GOLDEN,
    }
}
