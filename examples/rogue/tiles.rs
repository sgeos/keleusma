//! Procedural tile and entity atlas. The host builds every sprite
//! at startup using primitive shapes and color fills on SDL3
//! textures. No external asset pipeline. Map tiles are looked up
//! by kind. Monster and item sprites are rendered through helpers
//! that accept the entity's colour and shape selector.

use sdl3::pixels::Color;
use sdl3::rect::Rect;
use sdl3::render::{Canvas, Texture, TextureCreator};
use sdl3::video::{Window, WindowContext};

use crate::TILE_PX;
use crate::bestiary::{self, Shape};
use crate::items::{self, ItemKind};
use crate::world::Tile;

/// Static-tile sprite kinds. The map renderer indexes this set.
#[derive(Clone, Copy, Debug)]
pub enum Sprite {
    Floor,
    Wall,
    DoorClosed,
    DoorOpen,
    StairsDown,
    Exit,
    Player,
}

impl Sprite {
    pub const COUNT: usize = 7;

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_tile(t: Tile) -> Self {
        match t {
            Tile::Floor => Sprite::Floor,
            Tile::Wall => Sprite::Wall,
            Tile::DoorClosed => Sprite::DoorClosed,
            Tile::DoorOpen => Sprite::DoorOpen,
            Tile::StairsDown => Sprite::StairsDown,
            Tile::Exit => Sprite::Exit,
        }
    }
}

/// Container holding every pre-rendered sprite. Static tiles plus
/// one texture per monster kind plus six item silhouettes.
pub struct TileAtlas<'tex> {
    static_tiles: Vec<Texture<'tex>>,
    monsters: Vec<Texture<'tex>>,
    items: Vec<Texture<'tex>>,
}

impl<'tex> TileAtlas<'tex> {
    pub fn build(
        canvas: &mut Canvas<Window>,
        texture_creator: &'tex TextureCreator<WindowContext>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut static_tiles = Vec::with_capacity(Sprite::COUNT);
        for kind_idx in 0..Sprite::COUNT {
            let kind = sprite_from_index(kind_idx);
            let texture = render_static(canvas, texture_creator, kind)?;
            static_tiles.push(texture);
        }
        let mut monsters = Vec::with_capacity(bestiary::BESTIARY.len());
        for entry in bestiary::BESTIARY.iter() {
            let texture = render_monster(
                canvas,
                texture_creator,
                entry.shape,
                entry.primary,
                entry.accent,
            )?;
            monsters.push(texture);
        }
        let mut items = Vec::with_capacity(6);
        for kind_idx in 0..6 {
            let kind = item_kind_from_index(kind_idx);
            let texture = render_item(canvas, texture_creator, kind)?;
            items.push(texture);
        }
        Ok(Self {
            static_tiles,
            monsters,
            items,
        })
    }

    pub fn get(&mut self, kind: Sprite) -> &mut Texture<'tex> {
        &mut self.static_tiles[kind.index()]
    }

    pub fn monster(&mut self, kind_idx: usize) -> &mut Texture<'tex> {
        &mut self.monsters[kind_idx]
    }

    pub fn item(&mut self, kind: ItemKind) -> &mut Texture<'tex> {
        &mut self.items[kind.as_u8() as usize]
    }
}

fn sprite_from_index(i: usize) -> Sprite {
    match i {
        0 => Sprite::Floor,
        1 => Sprite::Wall,
        2 => Sprite::DoorClosed,
        3 => Sprite::DoorOpen,
        4 => Sprite::StairsDown,
        5 => Sprite::Exit,
        6 => Sprite::Player,
        _ => unreachable!("Sprite::COUNT mismatch"),
    }
}

fn item_kind_from_index(i: usize) -> ItemKind {
    match i {
        0 => ItemKind::Food,
        1 => ItemKind::Gold,
        2 => ItemKind::Weapon,
        3 => ItemKind::Armor,
        4 => ItemKind::Potion,
        5 => ItemKind::Scroll,
        _ => unreachable!("ItemKind index out of range"),
    }
}

fn render_static<'tex>(
    canvas: &mut Canvas<Window>,
    texture_creator: &'tex TextureCreator<WindowContext>,
    kind: Sprite,
) -> Result<Texture<'tex>, Box<dyn std::error::Error>> {
    let mut texture = texture_creator.create_texture_target(None, TILE_PX, TILE_PX)?;
    canvas.with_texture_canvas(&mut texture, |c| {
        c.set_draw_color(Color::RGB(8, 8, 12));
        c.clear();
        match kind {
            Sprite::Floor => draw_floor(c),
            Sprite::Wall => draw_wall(c),
            Sprite::DoorClosed => draw_door(c, false),
            Sprite::DoorOpen => draw_door(c, true),
            Sprite::StairsDown => draw_stairs_down(c),
            Sprite::Exit => draw_exit(c),
            Sprite::Player => draw_player(c),
        }
    })?;
    Ok(texture)
}

fn render_monster<'tex>(
    canvas: &mut Canvas<Window>,
    texture_creator: &'tex TextureCreator<WindowContext>,
    shape: Shape,
    primary: (u8, u8, u8),
    accent: (u8, u8, u8),
) -> Result<Texture<'tex>, Box<dyn std::error::Error>> {
    let mut texture = texture_creator.create_texture_target(None, TILE_PX, TILE_PX)?;
    canvas.with_texture_canvas(&mut texture, |c| {
        draw_floor(c);
        let p = Color::RGB(primary.0, primary.1, primary.2);
        let a = Color::RGB(accent.0, accent.1, accent.2);
        match shape {
            Shape::Tiny => draw_tiny(c, p, a),
            Shape::Small => draw_small(c, p, a),
            Shape::Humanoid => draw_humanoid(c, p, a),
            Shape::Brute => draw_brute(c, p, a),
            Shape::Serpent => draw_serpent(c, p, a),
            Shape::Insect => draw_insect(c, p, a),
            Shape::Skeleton => draw_skeleton(c, p, a),
            Shape::Mage => draw_mage(c, p, a),
            Shape::Ghost => draw_ghost(c, p, a),
            Shape::Slime => draw_slime(c, p, a),
            Shape::Dragon => draw_dragon(c, p, a),
            Shape::Boss => draw_boss(c, p, a),
        }
    })?;
    Ok(texture)
}

fn render_item<'tex>(
    canvas: &mut Canvas<Window>,
    texture_creator: &'tex TextureCreator<WindowContext>,
    kind: ItemKind,
) -> Result<Texture<'tex>, Box<dyn std::error::Error>> {
    let mut texture = texture_creator.create_texture_target(None, TILE_PX, TILE_PX)?;
    canvas.with_texture_canvas(&mut texture, |c| {
        draw_floor(c);
        match kind {
            ItemKind::Food => draw_food(c),
            ItemKind::Gold => draw_gold(c),
            ItemKind::Weapon => draw_weapon(c),
            ItemKind::Armor => draw_armor(c),
            ItemKind::Potion => draw_potion(c),
            ItemKind::Scroll => draw_scroll(c),
        }
    })?;
    Ok(texture)
}

// -- Map tile draws --------------------------------------------------

fn draw_floor(c: &mut Canvas<Window>) {
    c.set_draw_color(Color::RGB(36, 32, 28));
    let _ = c.fill_rect(Rect::new(0, 0, TILE_PX, TILE_PX));
    c.set_draw_color(Color::RGB(72, 68, 60));
    let third = (TILE_PX / 3) as i32;
    for ix in 0..3 {
        for iy in 0..3 {
            let cx = third / 2 + ix * third;
            let cy = third / 2 + iy * third;
            let _ = c.fill_rect(Rect::new(cx - 1, cy - 1, 2, 2));
        }
    }
}

fn draw_wall(c: &mut Canvas<Window>) {
    c.set_draw_color(Color::RGB(96, 88, 72));
    let _ = c.fill_rect(Rect::new(0, 0, TILE_PX, TILE_PX));
    c.set_draw_color(Color::RGB(160, 152, 132));
    let _ = c.fill_rect(Rect::new(0, 0, TILE_PX, 3));
    c.set_draw_color(Color::RGB(48, 40, 32));
    let _ = c.fill_rect(Rect::new(0, (TILE_PX - 3) as i32, TILE_PX, 3));
}

fn draw_door(c: &mut Canvas<Window>, open: bool) {
    draw_floor(c);
    if open {
        c.set_draw_color(Color::RGB(96, 64, 32));
        let _ = c.fill_rect(Rect::new(2, 4, 3, TILE_PX - 8));
        let _ = c.fill_rect(Rect::new((TILE_PX - 5) as i32, 4, 3, TILE_PX - 8));
    } else {
        c.set_draw_color(Color::RGB(120, 80, 40));
        let _ = c.fill_rect(Rect::new(4, 2, TILE_PX - 8, TILE_PX - 4));
        c.set_draw_color(Color::RGB(200, 160, 80));
        let mid_x = (TILE_PX / 2) as i32;
        let mid_y = (TILE_PX / 2) as i32;
        let _ = c.fill_rect(Rect::new(mid_x + 2, mid_y - 1, 3, 3));
    }
}

fn draw_stairs_down(c: &mut Canvas<Window>) {
    draw_floor(c);
    c.set_draw_color(Color::RGB(40, 80, 160));
    let step = (TILE_PX / 6) as i32;
    for i in 0..5 {
        let y = step * (i + 1);
        let w = TILE_PX - 2 * (i as u32 + 1) * step as u32;
        let x = (i + 1) * step;
        if w > 0 {
            let _ = c.fill_rect(Rect::new(x, y, w, step as u32));
        }
    }
}

fn draw_exit(c: &mut Canvas<Window>) {
    draw_floor(c);
    c.set_draw_color(Color::RGB(220, 200, 60));
    let half = (TILE_PX / 2) as i32;
    let _ = c.fill_rect(Rect::new(half - 6, 4, 12, TILE_PX - 8));
    c.set_draw_color(Color::RGB(255, 240, 140));
    let _ = c.fill_rect(Rect::new(half - 2, 4, 4, TILE_PX - 8));
}

fn draw_player(c: &mut Canvas<Window>) {
    draw_floor(c);
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(240, 240, 240));
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 8, 6, 6));
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 2, 8, 7));
    let _ = c.fill_rect(Rect::new(cx - 4, cy + 5, 3, 4));
    let _ = c.fill_rect(Rect::new(cx + 1, cy + 5, 3, 4));
}

// -- Monster silhouette draws ---------------------------------------

fn draw_tiny(c: &mut Canvas<Window>, p: Color, _a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 3, 6, 6));
}

fn draw_small(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 4, 10, 9));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 3, 2, 2));
    let _ = c.fill_rect(Rect::new(cx + 2, cy - 3, 2, 2));
}

fn draw_humanoid(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 8, 6, 5));
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 3, 8, 7));
    let _ = c.fill_rect(Rect::new(cx - 4, cy + 4, 3, 5));
    let _ = c.fill_rect(Rect::new(cx + 1, cy + 4, 3, 5));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 2, cy - 6, 1, 1));
    let _ = c.fill_rect(Rect::new(cx + 1, cy - 6, 1, 1));
}

fn draw_brute(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 8, 8, 6));
    let _ = c.fill_rect(Rect::new(cx - 7, cy - 2, 14, 8));
    let _ = c.fill_rect(Rect::new(cx - 4, cy + 6, 3, 4));
    let _ = c.fill_rect(Rect::new(cx + 1, cy + 6, 3, 4));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 2, cy - 6, 1, 2));
    let _ = c.fill_rect(Rect::new(cx + 1, cy - 6, 1, 2));
}

fn draw_serpent(c: &mut Canvas<Window>, p: Color, a: Color) {
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(3, 14, 4, 4));
    let _ = c.fill_rect(Rect::new(7, 10, 4, 4));
    let _ = c.fill_rect(Rect::new(11, 8, 4, 4));
    let _ = c.fill_rect(Rect::new(15, 6, 4, 4));
    let _ = c.fill_rect(Rect::new(19, 5, 4, 4));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(20, 6, 1, 1));
}

fn draw_insect(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 3, 10, 6));
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 6, 6, 3));
    c.set_draw_color(a);
    // Legs.
    for off in &[-5i32, -2, 1, 4] {
        let _ = c.fill_rect(Rect::new(cx + off, cy + 3, 1, 4));
        let _ = c.fill_rect(Rect::new(cx + off, cy - 7, 1, 4));
    }
}

fn draw_skeleton(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 8, 6, 5));
    let _ = c.fill_rect(Rect::new(cx - 1, cy - 3, 2, 8));
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 1, 10, 1));
    let _ = c.fill_rect(Rect::new(cx - 3, cy + 5, 2, 4));
    let _ = c.fill_rect(Rect::new(cx + 1, cy + 5, 2, 4));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 2, cy - 6, 1, 1));
    let _ = c.fill_rect(Rect::new(cx + 1, cy - 6, 1, 1));
}

fn draw_mage(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 9, 6, 4));
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 5, 10, 12));
    let _ = c.fill_rect(Rect::new(cx - 3, cy + 7, 6, 2));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 1, cy - 11, 2, 3));
    let _ = c.fill_rect(Rect::new(cx - 1, cy + 1, 2, 4));
}

fn draw_ghost(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 7, 10, 12));
    let _ = c.fill_rect(Rect::new(cx - 5, cy + 5, 2, 2));
    let _ = c.fill_rect(Rect::new(cx - 1, cy + 5, 2, 2));
    let _ = c.fill_rect(Rect::new(cx + 3, cy + 5, 2, 2));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 4, 2, 2));
    let _ = c.fill_rect(Rect::new(cx + 1, cy - 4, 2, 2));
}

fn draw_slime(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 7, cy - 1, 14, 9));
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 4, 10, 4));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 3, cy + 1, 2, 2));
    let _ = c.fill_rect(Rect::new(cx + 1, cy + 1, 2, 2));
}

fn draw_dragon(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    // Body.
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 2, 9, 6));
    // Head.
    let _ = c.fill_rect(Rect::new(cx + 5, cy - 5, 5, 5));
    // Wings.
    let _ = c.fill_rect(Rect::new(cx - 7, cy - 8, 6, 3));
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 11, 6, 3));
    // Tail.
    let _ = c.fill_rect(Rect::new(cx - 9, cy + 1, 5, 3));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx + 8, cy - 3, 1, 1));
}

fn draw_boss(c: &mut Canvas<Window>, p: Color, a: Color) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(p);
    let _ = c.fill_rect(Rect::new(cx - 9, cy - 9, 18, 18));
    c.set_draw_color(a);
    let _ = c.fill_rect(Rect::new(cx - 7, cy - 7, 4, 4));
    let _ = c.fill_rect(Rect::new(cx + 3, cy - 7, 4, 4));
    let _ = c.fill_rect(Rect::new(cx - 6, cy + 3, 12, 3));
}

// -- Item silhouettes -----------------------------------------------

fn draw_food(c: &mut Canvas<Window>) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(200, 140, 60));
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 4, 10, 8));
    c.set_draw_color(Color::RGB(80, 40, 20));
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 3, 2, 2));
    let _ = c.fill_rect(Rect::new(cx + 2, cy - 3, 2, 2));
}

fn draw_gold(c: &mut Canvas<Window>) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(220, 180, 40));
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 4, 8, 8));
    c.set_draw_color(Color::RGB(255, 220, 80));
    let _ = c.fill_rect(Rect::new(cx - 3, cy - 3, 2, 2));
}

fn draw_weapon(c: &mut Canvas<Window>) {
    let cx = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(200, 200, 220));
    let _ = c.fill_rect(Rect::new(cx - 1, 3, 2, TILE_PX - 8));
    c.set_draw_color(Color::RGB(120, 80, 40));
    let _ = c.fill_rect(Rect::new(cx - 3, (TILE_PX as i32) - 7, 6, 3));
}

fn draw_armor(c: &mut Canvas<Window>) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(160, 160, 200));
    let _ = c.fill_rect(Rect::new(cx - 6, cy - 4, 12, 10));
    c.set_draw_color(Color::RGB(80, 80, 120));
    let _ = c.fill_rect(Rect::new(cx - 6, cy + 2, 12, 2));
}

fn draw_potion(c: &mut Canvas<Window>) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(80, 60, 40));
    let _ = c.fill_rect(Rect::new(cx - 2, cy - 7, 4, 3));
    c.set_draw_color(Color::RGB(40, 180, 220));
    let _ = c.fill_rect(Rect::new(cx - 4, cy - 4, 8, 9));
}

fn draw_scroll(c: &mut Canvas<Window>) {
    let cx = (TILE_PX / 2) as i32;
    let cy = (TILE_PX / 2) as i32;
    c.set_draw_color(Color::RGB(220, 200, 160));
    let _ = c.fill_rect(Rect::new(cx - 6, cy - 4, 12, 8));
    c.set_draw_color(Color::RGB(80, 40, 20));
    let _ = c.fill_rect(Rect::new(cx - 5, cy - 2, 10, 1));
    let _ = c.fill_rect(Rect::new(cx - 5, cy, 8, 1));
    let _ = c.fill_rect(Rect::new(cx - 5, cy + 2, 10, 1));
}

// Suppress an unused-import warning when items::POTION_EFFECTS and
// related constants are referenced indirectly via the renderer.
#[allow(dead_code)]
fn _items_referenced() -> usize {
    items::WEAPONS.len() + items::ARMORS.len()
}
