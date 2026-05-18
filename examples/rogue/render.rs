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

use sdl3::render::Texture;

use crate::items::ItemKind;
use crate::text;
use crate::tiles::{Sprite, TileAtlas};
use crate::world::World;
use crate::{HP_BAR_PX, HUD_PX, INFO_BAR_PX, MAP_H, MAP_W, MSG_PX, TILE_PX};

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

/// Outcome of a finished run. Passed to the game-over overlay
/// so the panel can show the appropriate title and stats.
#[derive(Clone, Copy, Debug)]
pub enum GameOver {
    Died,
    Won,
}

/// Renderer state. Empty for now. Later phases cache message-log
/// scroll position here.
pub struct Renderer;

impl Renderer {
    pub fn new() -> Self {
        Self
    }

    /// Render the game-over overlay on top of the existing
    /// frame. The panel is centred and shows the outcome title,
    /// the final floor reached, the gold collected, and the
    /// turn counter.
    pub fn draw_game_over(
        &mut self,
        canvas: &mut Canvas<Window>,
        world: &World,
        outcome: GameOver,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let win_w = (MAP_W * TILE_PX) as i32;
        let win_h = (HUD_PX + MAP_H * TILE_PX + MSG_PX) as i32;
        let panel_w = 480_i32;
        let panel_h = 240_i32;
        let panel_x = (win_w - panel_w) / 2;
        let panel_y = (win_h - panel_h) / 2;

        // Backdrop.
        canvas.set_draw_color(Color::RGB(8, 8, 16));
        let _ = canvas.fill_rect(Rect::new(panel_x, panel_y, panel_w as u32, panel_h as u32));
        canvas.set_draw_color(Color::RGB(180, 180, 200));
        let _ = canvas.fill_rect(Rect::new(panel_x, panel_y, panel_w as u32, 2));
        let _ = canvas.fill_rect(Rect::new(panel_x, panel_y + panel_h - 2, panel_w as u32, 2));
        let _ = canvas.fill_rect(Rect::new(panel_x, panel_y, 2, panel_h as u32));
        let _ = canvas.fill_rect(Rect::new(panel_x + panel_w - 2, panel_y, 2, panel_h as u32));

        // Title.
        let (title, title_color) = match outcome {
            GameOver::Died => ("YOU DIED", Color::RGB(220, 60, 60)),
            GameOver::Won => ("VICTORY", Color::RGB(220, 200, 60)),
        };
        let title_scale = 6;
        let title_w = text::text_width(title, title_scale);
        text::draw_text(
            canvas,
            panel_x + (panel_w - title_w) / 2,
            panel_y + 24,
            title,
            title_color,
            title_scale,
        );

        // Stats.
        let stats_scale = 3;
        let stat_y0 = panel_y + 120;
        let stats = [
            format!("FLOOR  {}", world.floor),
            format!("GOLD   {}", world.player.gold),
            format!("TURNS  {}", world.player.turn),
        ];
        for (i, line) in stats.iter().enumerate() {
            let line_w = text::text_width(line, stats_scale);
            text::draw_text(
                canvas,
                panel_x + (panel_w - line_w) / 2,
                stat_y0 + i as i32 * 32,
                line,
                Color::RGB(220, 220, 230),
                stats_scale,
            );
        }

        // Prompt.
        let prompt = "R RESTART  Q QUIT";
        let prompt_scale = 2;
        let prompt_w = text::text_width(prompt, prompt_scale);
        text::draw_text(
            canvas,
            panel_x + (panel_w - prompt_w) / 2,
            panel_y + panel_h - 28,
            prompt,
            Color::RGB(160, 200, 240),
            prompt_scale,
        );
        Ok(())
    }

    pub fn draw(
        &mut self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.draw_hp_bar(canvas, world)?;
        self.draw_info_bar(canvas, atlas, world)?;
        self.draw_map(canvas, atlas, world)?;
        self.draw_items(canvas, atlas, world)?;
        self.draw_monsters(canvas, atlas, world)?;
        self.draw_player(canvas, atlas, world)?;
        self.draw_msg(canvas, world)?;
        Ok(())
    }

    /// Top row of the head-up display. The hit-point pip strip
    /// claims the full window width. At high floors the strip
    /// runs across the entire row; lower floors leave the right
    /// side dark.
    fn draw_hp_bar(
        &self,
        canvas: &mut Canvas<Window>,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        canvas.set_draw_color(Color::RGB(16, 16, 24));
        let _ = canvas.fill_rect(Rect::new(0, 0, MAP_W * TILE_PX, HP_BAR_PX));

        let p = &world.player;
        let hp_x0 = 4_i32;
        let pip_w = 3_u32;
        let pip_h = HP_BAR_PX - 6;
        let pips = p.max_hp.max(1) as u32;
        let max_pip_idx = (MAP_W * TILE_PX - hp_x0 as u32) / (pip_w + 1);
        for i in 0..pips.min(max_pip_idx) {
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
        Ok(())
    }

    /// Second row of the head-up display. Carries gear icons,
    /// floor ticks, floor and gold text, held consumable icons,
    /// and the hunger gauge.
    fn draw_info_bar(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let row_y = HP_BAR_PX as i32;
        canvas.set_draw_color(Color::RGB(16, 16, 24));
        let _ = canvas.fill_rect(Rect::new(0, row_y, MAP_W * TILE_PX, INFO_BAR_PX));
        canvas.set_draw_color(Color::RGB(80, 80, 96));
        let _ = canvas.fill_rect(Rect::new(0, (HUD_PX - 1) as i32, MAP_W * TILE_PX, 1));

        let p = &world.player;
        let pip_w = 3_u32;
        let pip_h = INFO_BAR_PX - 6;

        // Hunger gauge. Right side of the row.
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
                row_y + 3,
                pip_w,
                pip_h,
            ));
        }

        // Floor depth ticks at the centre of the row.
        let f_ticks = ((world.floor as i32).min(100) - 1) / 5 + 1;
        let f_x0 = (MAP_W as i32 * TILE_PX as i32) / 2 - f_ticks * 4;
        canvas.set_draw_color(Color::RGB(120, 180, 220));
        for i in 0..f_ticks {
            let _ = canvas.fill_rect(Rect::new(f_x0 + i * 4, row_y + 4, 2, pip_h));
        }

        // Weapon and armor tier indicators.
        let weapon_icon_x = 8_i32;
        self.draw_gear_indicator(
            canvas,
            atlas,
            ItemKind::Weapon,
            weapon_icon_x,
            row_y,
            world.player.weapon as i32,
            Color::RGB(220, 80, 60),
        )?;
        let armor_icon_x = weapon_icon_x + 24 + 36;
        self.draw_gear_indicator(
            canvas,
            atlas,
            ItemKind::Armor,
            armor_icon_x,
            row_y,
            world.player.armor as i32,
            Color::RGB(140, 180, 220),
        )?;

        // Floor and gold text right of the floor ticks.
        let stats_x = (MAP_W as i32 * TILE_PX as i32) / 2 + f_ticks * 4 + 12;
        let stats_y = row_y + 6;
        text::draw_text(
            canvas,
            stats_x,
            stats_y,
            &format!("F{:02}", world.floor),
            Color::RGB(120, 180, 220),
            2,
        );
        text::draw_text(
            canvas,
            stats_x + text::text_width("F00", 2) + 8,
            stats_y,
            &format!("G{}", world.player.gold),
            Color::RGB(220, 180, 60),
            2,
        );

        // Held potion and scroll icons, tinted by per-run colour.
        let potion_x = (MAP_W as i32 * TILE_PX as i32) / 2 + 220;
        if let Some(effect_idx) = world.player.potion_slot {
            let appearance = world.potion_appearance[effect_idx as usize] as usize;
            let (r, g, b) = POTION_COLORS[appearance % POTION_COLORS.len()];
            let tex = atlas.item(ItemKind::Potion);
            tex.set_color_mod(r, g, b);
            blit_tex(canvas, tex, potion_x, row_y, 24, 24)?;
        }
        let scroll_x = potion_x + 28;
        if let Some(effect_idx) = world.player.scroll_slot {
            let appearance = world.scroll_appearance[effect_idx as usize] as usize;
            let (r, g, b) = SCROLL_COLORS[appearance % SCROLL_COLORS.len()];
            let tex = atlas.item(ItemKind::Scroll);
            tex.set_color_mod(r, g, b);
            blit_tex(canvas, tex, scroll_x, row_y, 24, 24)?;
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
        let tex = atlas.item(kind);
        tex.set_color_mod(255, 255, 255);
        blit_tex(canvas, tex, icon_x, icon_y, 24, 24)?;
        // Tier pips. Ten vertical bars, two pixels wide with one
        // pixel gap. The bar is filled solid when its index is
        // less than or equal to `tier`, dim otherwise.
        let pip_x0 = icon_x + 26;
        let pip_top = icon_y + 3;
        let pip_h = (INFO_BAR_PX as i32 - 6) as u32;
        for i in 0..10i32 {
            canvas.set_draw_color(if i <= tier {
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
                let sprite = Sprite::from_tile(world.map.get(tx, ty));
                let tex = atlas.get(sprite);
                let (r, g, b) = if visible {
                    (255, 255, 255)
                } else {
                    (96, 96, 110)
                };
                tex.set_color_mod(r, g, b);
                self.blit_at(canvas, tex, tx, ty)?;
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
            let tex = atlas.item(item.kind);
            tex.set_color_mod(255, 255, 255);
            self.blit_at(canvas, tex, item.x, item.y)?;
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
            let tex = atlas.monster(monster.kind as usize);
            tex.set_color_mod(255, 255, 255);
            self.blit_at(canvas, tex, monster.x, monster.y)?;
        }
        Ok(())
    }

    fn draw_player(
        &self,
        canvas: &mut Canvas<Window>,
        atlas: &mut TileAtlas,
        world: &World,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tex = atlas.get(Sprite::Player);
        tex.set_color_mod(255, 255, 255);
        self.blit_at(canvas, tex, world.player.x, world.player.y)
    }

    /// Copy a texture into the map cell at `(tx, ty)` using the
    /// renderer's tile rectangle.
    fn blit_at(
        &self,
        canvas: &mut Canvas<Window>,
        tex: &Texture,
        tx: i32,
        ty: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dst = self.tile_rect(tx, ty);
        canvas
            .copy(tex, None, Some(dst.into()))
            .map_err(|e| e.to_string().into())
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
        if let Some(msg) = world.latest_message() {
            let upper = msg.to_uppercase();
            text::draw_text(canvas, 8, y0 + 5, &upper, Color::RGB(220, 220, 230), 2);
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

/// Copy a texture into a free-form rectangle. Used by head-up
/// display elements that draw at absolute coordinates rather
/// than aligned to the tile grid.
fn blit_tex(
    canvas: &mut Canvas<Window>,
    tex: &Texture,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    canvas
        .copy(tex, None, Some(Rect::new(x, y, w, h).into()))
        .map_err(|e| e.to_string().into())
}
