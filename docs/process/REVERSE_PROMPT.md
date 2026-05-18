# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Tier five game-balance exercises added to the manual, including a candid note that the starvation fix overshot. Survey of remaining deferred work and small refactoring candidates included below.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Add Tier five game balance to the exercises | New `### Tier five: game balance` section at the end of the exercises chain. Four exercises. 5.1 explicitly states that starvation was a problem, the fix combined a halved hunger cadence with corpse drops, and the combination overshot so starvation is no longer a real threat; the exercise asks the reader to find a configuration that puts the player back in the danger zone without making early-game starvation inevitable. 5.2 asks for difficulty-curve calibration across the bestiary's `first_floor` boundaries. 5.3 audits the loot distribution. 5.4 calibrates the weapon-damage progression against monster hit points. |

## Remaining deferred work

All intentional. None block gameplay.

- **Exercise 3.7.** Sleep, Confusion, and Remove Curse scroll-effect handlers in `examples/rogue/natives.rs::apply_scroll_status` are still placeholder messages. The script-side dispatch produces correct status codes; the host-side application is the exercise.
- **Exercise 3.6.** `monster_sees_player` reads the player's field-of-view bitmap rather than running a per-monster shadowcast.
- **Tier-two-and-up exercises generally.** Doors at corridor-to-room boundaries, ranged player attacks, vault generation, save and load, sprite-sheet renderer, alternative archetypes, bestiary-in-script. None are bugs.

## Remaining refactoring candidates

Each is small (single-digit line savings). None are urgent. The example is at a point where further refactoring needs a concrete reason rather than mechanical deduplication.

1. **Data-slot zeroing.** `init_data_slots` in `main.rs`, the data-zero loop in `ai::build_vm`, and the data-zero loop in `ai::reset_loop_main_data` all do the same three lines. A shared free function would save about six lines and tighten the invariant.
2. **`world.lock().unwrap().push_message(...)` shorthand.** Eight sites in `main.rs` follow this pattern. A small `push_msg(world, msg)` free function would save four to six lines and read more cleanly at the error-handling sites.
3. **Location of `EMBEDDED`.** The nineteen-row script table lives in `main.rs` because that is where `include_str!` resolves the relative paths. Moving it into `ai.rs` alongside `AiModules::build` would localise the script-list source of truth. The cost is coupling `ai.rs` to filenames and the loss of `include_str!` resolution against the entry-point directory.
4. **`embedded_source` and `disk_source` shape.** Both return `Result<X, Box<dyn Error>>` with a `format!`-built message. A small `with_context(name, op)` helper could parallelise the error-context construction. Marginal.
5. **`render.rs::draw_*` family.** Ten draw methods on `Renderer` with similar signatures `(&self, &mut Canvas, ...)`. A trait or a method-chain refactor could compress the call sites but at the cost of clarity. Not recommended without a concrete pull from a new feature.

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Intended Next Step

Awaiting operator prompt. The example is in a clean state. Candidate next directions:

1. **Pick up the strongest balance exercise (5.1) as a real change.** A combination of slightly faster hunger cadence (one per turn? one per one and a half turns through a `turn % 3 < 2` predicate?) and slightly lower corpse satiation would put starvation back as a real threat. The operator could direct one specific tuning combination and ship it as the new default.
2. **Pick up one of the placeholder scroll handlers from Exercise 3.7.** Remove Curse and Sleep are each about thirty host-side lines.
3. **Apply one of the small refactors above.** None are urgent; pick by taste.
