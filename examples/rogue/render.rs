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

use crate::tiles::{Sprite, TileAtlas};
use crate::world::World;
use crate::{HUD_PX, MAP_H, MAP_W, MSG_PX, TILE_PX};

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
        self.draw_hud(canvas, world)?;
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
