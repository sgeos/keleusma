//! Renderer. Draws the HUD bar, the tile grid with field-of-view
//! shading, monster and item glyphs, the player glyph, and the
//! message bar. Visible tiles draw at full saturation. Explored-
//! but-not-currently-visible tiles draw dimmed. Unexplored tiles
//! draw black. Monsters and items only draw when the cell is in
//! the player's current field of view.

use sdl3::pixels::Color;
use sdl3::rect::Rect;
use sdl3::render::Canvas;
use sdl3::video::Window;

use crate::items::ItemKind;
use crate::tiles::{Sprite, TileAtlas};
use crate::world::World;
use crate::{HUD_PX, MAP_H, MAP_W, MSG_PX, TILE_PX};

/// RGB tint applied to the potion icon in the head-up display
/// when the player holds a potion. Indexed by the per-run
/// `potion_appearance` shuffled mapping so the same colour
/// always represents the same disguised identity within a run.
const POTION_COLORS: [(u8, u8, u8); 10] = [
    (60, 120, 220),  // blue
    (220, 60, 60),   // red
    (60, 200, 80),   // green
    (220, 180, 60),  // amber
    (180, 80, 220),  // violet
    (220, 220, 220), // milky
    (140, 240, 220), // fizzy
    (110, 110, 110), // smoky
    (180, 200, 220), // cloudy
    (220, 60, 120),  // ruby
];

/// RGB tint applied to the scroll icon when the player holds a
/// scroll. Indexed by the per-run `scroll_appearance` shuffle.
const SCROLL_COLORS: [(u8, u8, u8); 10] = [
    (240, 200, 140),
    (240, 160, 140),
    (240, 140, 200),
    (220, 140, 240),
    (160, 160, 240),
    (140, 200, 240),
    (140, 240, 200),
    (160, 240, 140),
    (240, 240, 140),
    (240, 180, 140),
];

/// Renderer state. Empty for now. Later phases cache message-log
/// scroll position here.
pub struct Renderer;

impl Renderer {
    pub fn new() -> Self {
        Self
    }

    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.draw_hud(canvas, atlas, world)?;
        self.draw_map(canvas, atlas, world)?;
        self.draw_items(canvas, atlas, world)?;
        self.draw_monsters(canvas, atlas, world)?;
        self.draw_player(canvas, atlas, world)?;
        self.draw_msg(canvas, world)?;
        Ok(())
    }

    fn draw_hud(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        canvas.set_draw_color(Color::RGB(16, 16, 24));
        let _ = canvas.fill_rect(Rect::new(0, 0, MAP_W * TILE_PX, HUD_PX));
        canvas.set_draw_color(Color::RGB(80, 80, 96));
        let _ = canvas.fill_rect(Rect::new(0, (HUD_PX - 1) as i32, MAP_W * TILE_PX, 1));

        // HP gauge segments. Each pip is two pixels wide.
        let p = &world.player;
        let hp_x0 = 4_i32;
        let pip_w = 3_u32;
        let pip_h = HUD_PX - 6;
        let pips = p.max_hp.max(1) as u32;
        for i in 0..pips {
            let filled = (i as i32) < p.hp;
            canvas.set_draw_color(if filled {
                Color::RGB(220, 60, 60)
            } else {
                Color::RGB(60, 30, 30)
            });
            let _ = canvas.fill_rect(Rect::new(
                hp_x0 + (i as i32) * (pip_w as i32 + 1),
                3,
                pip_w,
                pip_h,
            ));
        }

        // Hunger gauge. Right side of the HUD bar.
        let hg_pips = 20_u32;
        let hg_filled = ((p.hunger.max(0) as u32) * hg_pips) / (p.max_hunger as u32).max(1);
        let hg_x0 = MAP_W as i32 * TILE_PX as i32 - 8 - hg_pips as i32 * (pip_w as i32 + 1);
        for i in 0..hg_pips {
            let filled = i < hg_filled;
            canvas.set_draw_color(if filled {
                Color::RGB(220, 180, 60)
            } else {
                Color::RGB(60, 40, 10)
            });
            let _ = canvas.fill_rect(Rect::new(
                hg_x0 + (i as i32) * (pip_w as i32 + 1),
                3,
                pip_w,
                pip_h,
            ));
        }

        // Floor indicator. Centre of bar, vertical bars equal to
        // floor / 5 rounded down for a coarse depth tick.
        let f_ticks = ((world.floor as i32).min(100) - 1) / 5 + 1;
        let f_x0 = (MAP_W as i32 * TILE_PX as i32) / 2 - f_ticks * 4;
        canvas.set_draw_color(Color::RGB(120, 180, 220));
        for i in 0..f_ticks {
            let _ = canvas.fill_rect(Rect::new(f_x0 + i * 4, 4, 2, pip_h));
        }

        // Weapon and armor tier indicators. Each is a small
        // icon followed by a thin vertical pip strip showing the
        // current tier on a zero through nine scale.
        let gear_y = 0_i32;
        let weapon_icon_x = 220_i32;
        self.draw_gear_indicator(
            canvas,
            atlas,
            ItemKind::Weapon,
            weapon_icon_x,
            gear_y,
            world.player.weapon as i32,
            Color::RGB(220, 80, 60),
        )?;
        let armor_icon_x = weapon_icon_x + 24 + 36;
        self.draw_gear_indicator(
            canvas,
            atlas,
            ItemKind::Armor,
            armor_icon_x,
            gear_y,
            world.player.armor as i32,
            Color::RGB(140, 180, 220),
        )?;

        // Held potion and scroll icons. Each is the regular item
        // sprite tinted by the per-run appearance colour. If the
        // slot is empty the icon is skipped entirely.
        let consumable_y = 0_i32;
        let potion_x = (MAP_W as i32 * TILE_PX as i32) / 2 + 80;
        if let Some(effect_idx) = world.player.potion_slot {
            let appearance = world.potion_appearance[effect_idx as usize] as usize;
            let color = POTION_COLORS[appearance % POTION_COLORS.len()];
            let dst = Rect::new(potion_x, consumable_y, 24, 24);
            let tex = atlas.item(ItemKind::Potion);
            tex.set_color_mod(color.0, color.1, color.2);
            canvas
                .copy(tex, None, Some(dst.into()))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        }
        let scroll_x = potion_x + 28;
        if let Some(effect_idx) = world.player.scroll_slot {
            let appearance = world.scroll_appearance[effect_idx as usize] as usize;
            let color = SCROLL_COLORS[appearance % SCROLL_COLORS.len()];
            let dst = Rect::new(scroll_x, consumable_y, 24, 24);
            let tex = atlas.item(ItemKind::Scroll);
            tex.set_color_mod(color.0, color.1, color.2);
            canvas
                .copy(tex, None, Some(dst.into()))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        }
        Ok(())
    }

    /// Draw a gear icon and its tier pip strip. `tier` is the
    /// zero-based subtype index. The pip strip shows ten thin
    /// vertical lines, filled up to and including `tier`.
    #[allow(clippy::too_many_arguments)]
    fn draw_gear_indicator(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        kind: ItemKind,
        icon_x: i32,
        icon_y: i32,
        tier: i32,
        fill_color: Color,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dst = Rect::new(icon_x, icon_y, 24, 24);
        let tex = atlas.item(kind);
        tex.set_color_mod(255, 255, 255);
        canvas
            .copy(tex, None, Some(dst.into()))
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        // Tier pips. Ten vertical bars, two pixels wide with one
        // pixel gap. The bar is filled solid when its index is
        // less than or equal to `tier`, dim otherwise.
        let pip_x0 = icon_x + 26;
        let pip_top = icon_y + 3;
        let pip_h = (HUD_PX as i32 - 6) as u32;
        for i in 0..10i32 {
            let filled = i <= tier;
            canvas.set_draw_color(if filled {
                fill_color
            } else {
                Color::RGB(40, 40, 50)
            });
            let _ = canvas.fill_rect(Rect::new(pip_x0 + i * 3, pip_top, 2, pip_h));
        }
        Ok(())
    }

    fn draw_map(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for ty in 0..MAP_H as i32 {
            for tx in 0..MAP_W as i32 {
                let visible = world.visible_at(tx, ty);
                let explored = world.explored_at(tx, ty);
                if !visible && !explored {
                    continue;
                }
                let tile = world.map.get(tx, ty);
                let sprite = Sprite::from_tile(tile);
                let dst = self.tile_rect(tx, ty);
                let tex = atlas.get(sprite);
                let mod_color = if visible {
                    Color::RGB(255, 255, 255)
                } else {
                    Color::RGB(96, 96, 110)
                };
                tex.set_color_mod(mod_color.r, mod_color.g, mod_color.b);
                canvas
                    .copy(tex, None, Some(dst.into()))
                    .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            }
        }
        Ok(())
    }

    fn draw_items(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for item in world.items.iter() {
            if !world.visible_at(item.x, item.y) {
                continue;
            }
            let dst = self.tile_rect(item.x, item.y);
            let tex = atlas.item(item.kind);
            tex.set_color_mod(255, 255, 255);
            canvas
                .copy(tex, None, Some(dst.into()))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        }
        Ok(())
    }

    fn draw_monsters(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for monster in world.monsters.iter() {
            if !world.visible_at(monster.x, monster.y) {
                continue;
            }
            let dst = self.tile_rect(monster.x, monster.y);
            let tex = atlas.monster(monster.kind as usize);
            tex.set_color_mod(255, 255, 255);
            canvas
                .copy(tex, None, Some(dst.into()))
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        }
        Ok(())
    }

    fn draw_player(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dst = self.tile_rect(world.player.x, world.player.y);
        let tex = atlas.get(Sprite::Player);
        tex.set_color_mod(255, 255, 255);
        canvas
            .copy(tex, None, Some(dst.into()))
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        Ok(())
    }

    fn draw_msg(
        &self,
        canvas: &mut Canvas<Window>,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let y0 = (HUD_PX + MAP_H * TILE_PX) as i32;
        canvas.set_draw_color(Color::RGB(16, 16, 24));
        let _ = canvas.fill_rect(Rect::new(0, y0, MAP_W * TILE_PX, MSG_PX));
        canvas.set_draw_color(Color::RGB(80, 80, 96));
        let _ = canvas.fill_rect(Rect::new(0, y0, MAP_W * TILE_PX, 1));

        // Placeholder for the message text. A bitmap-font draw
        // lands in a later phase. For now, render the latest
        // message as a coloured bar whose length scales with the
        // message length, so the host can confirm message flow at
        // a glance.
        if let Some(msg) = world.latest_message() {
            let n = msg.len() as i32;
            let bar_w = (n.min(80) * (TILE_PX as i32 / 4)) as u32;
            canvas.set_draw_color(Color::RGB(180, 200, 220));
            let _ = canvas.fill_rect(Rect::new(6, y0 + 6, bar_w, MSG_PX - 12));
        }
        Ok(())
    }

    fn tile_rect(&self, tx: i32, ty: i32) -> Rect {
        Rect::new(
            tx * TILE_PX as i32,
            HUD_PX as i32 + ty * TILE_PX as i32,
            TILE_PX,
            TILE_PX,
        )
    }
}
