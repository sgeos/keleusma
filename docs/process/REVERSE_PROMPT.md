# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Five surveyed refactors all applied. Head-up display split into two rows so the hit-point bar gets the full window width and the rest of the readouts stay at fixed positions.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Refactor 1: data-slot zeroing | `zero_data_slots` is now a `pub fn` in `ai.rs`. The three call sites (startup of dungen and game virtual machines in `main.rs`, plus `AiPool::build_vm` and `AiPool::reset_loop_main_data` in `ai.rs`) all delegate to it. The `init_data_slots` free function in `main.rs` is removed. |
| Refactor 2: push-message shorthand | `push_msg(world, msg)` in `natives.rs` wraps the lock plus push pattern. Five call sites in `main.rs` (welcome message, victory message, descend message, four reload-error paths, and the reload success message) drop from two-or-three lines each to one line each. |
| Refactor 3: location of `EMBEDDED` | The `EMBEDDED` table, `SCRIPT_DIR`, `build_module`, `compile_embedded`, and `compile_disk` all moved to `ai.rs`. `main.rs` now imports the four helpers it needs. The script-list source of truth lives next to `AiModules::build` where adding a new script naturally touches both. |
| Refactor 4: collapse `embedded_source` and `disk_source` | The two intermediate functions are gone. `compile_embedded` and `compile_disk` each contain the table lookup or file read inline. Net loss of about ten lines for the same behaviour. |
| Refactor 5: render-method family | A `Renderer::blit_at(canvas, tex, tx, ty)` method and a free `blit_tex(canvas, tex, x, y, w, h)` function absorb the destination-rect plus copy plus error-format pattern. `draw_map`, `draw_items`, `draw_monsters`, `draw_player`, `draw_gear_indicator`, and the two held-consumable arms all use one or the other. |
| Head-up display layout: hit-point bar on the top row | New `HP_BAR_PX` and `INFO_BAR_PX` constants in `main.rs` each twenty-four pixels tall. `HUD_PX` is now their sum. Top row holds the hit-point pip strip across the full window width; second row holds gear icons, depth ticks, floor and gold text, held consumable icons, and the hunger gauge. The pip strip is clipped at the window width as a defensive measure for the floor one hundred run where the cap would otherwise exceed one thousand twenty-four pixels. The map and the message row shift down by twenty-four pixels; the window is correspondingly taller. |

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Line-count delta

```
 docs/guide/ROGUE.md       |   4 +-
 examples/rogue/ai.rs      |  78 +++++++++++++++++--
 examples/rogue/main.rs    | 193 +++++++++++-----------------------------------
 examples/rogue/natives.rs |   7 ++
 examples/rogue/render.rs  | 167 ++++++++++++++++++++++-----------------
 5 files changed, 222 insertions(+), 227 deletions(-)
```

Net five lines deleted. The headline number is smaller than the previous refactor round because the head-up display split added back layout code that the refactors would otherwise have removed. The structural improvements are still real: every duplicate pattern surveyed is now folded.

## Notes

- The hit-point pip strip in the top row is capped to the window width with `pips.min(max_pip_idx)`. The cap activates only past roughly two hundred fifty hit points, which corresponds to depth eighty or so. The capping is silent; the player loses the visual signal of having extra hit points beyond the window edge but the actual count is unaffected.
- The game-over panel still positions itself relative to `WINDOW_W` and `WINDOW_H`, both of which automatically follow the new `HUD_PX`. No manual adjustment needed there.
- The `blit_at` method on `Renderer` takes a `&Texture` rather than `&mut Texture` because the SDL3 `Canvas::copy` signature accepts an immutable reference. Color-mod calls before the blit still need the mutable borrow but the borrow is released before the blit.

## Intended Next Step

Awaiting operator prompt. No remaining surveyed refactors and no open bugs that I can identify. The example reads cleanly across its host modules and its scripts.
