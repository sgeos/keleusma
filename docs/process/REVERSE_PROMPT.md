# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-17
**Status**: Hunger cadence halved, two pieces of dead code removed, manual updated for the new defaults. Survey of deferred work and remaining refactoring opportunities included below.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Corpse drops with bestiary-defined effects | Commit `f39e7c4`. |
| Document loop-main reset semantics | Commit `f39e7c4`. |
| Spanning-tree corridors | Commit `8450abc`. Defence-in-depth, retained. |
| Carve floor only, drop destructive wall-fill | Commit `4587871`. Root-cause fix. |
| Food lasts twice as long | Current commit. `rogue_book_keeping.kel` now decrements hunger only on even-numbered turns. Starting hunger of one hundred lasts two hundred turns of fed state, double the previous run length. Starvation damage cadence is unchanged because the change targets the fed phase, not the starving phase. |
| Dead-code removal | Current commit. `World::new_demo` (about ninety lines of phase-zero scaffolding) is removed because the dungen script supersedes it. `tiles::_items_referenced` (a workaround for an unused-import warning) is removed and the import is narrowed from `use crate::items::{self, ItemKind}` to `use crate::items::ItemKind`. |
| Manual currency | The Items section, the `host::tick_book_keeping` description, the Default tuning parameters table, and Exercise 1.2 in the manual all updated to describe the new hunger cadence. Exercise 1.2's task is no longer the shipped behaviour, so the exercise is reframed as a tuning experiment. |

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Remaining deferred work

The intentional deferrals are documented as manual exercises. They are not bugs and not on the V0.1.0 critical path.

- **Exercise 3.7.** Sleep, Confusion, and Remove Curse status codes have placeholder messages in `examples/rogue/natives.rs::apply_scroll_status`. The script-side dispatch produces the correct codes; the host-side application is left for the reader.
- **Exercise 3.6.** `monster_sees_player` uses the player's field-of-view bitmap as the ground truth. The note in the source acknowledges this is symmetric-by-construction and labels the per-monster shadowcast as an exercise.
- **Exercises 2.x and 3.x.** Doors at corridor-to-room boundaries, ranged attacks for the player, vault generation, save and load, sprite-sheet renderer, alternative artificial-intelligence archetypes. None block gameplay.

## Remaining refactoring opportunities

Surveyed but not applied this round. Each is a small, contained improvement that could be picked up in a follow-up if desired.

1. **Reload-and-startup script-loading duplication.** `examples/rogue/main.rs::reload_scripts` and `fn main` both construct an `AiModules` value from script sources. The startup path uses `include_str!` constants; the reload path reads from disk. Both then call `build_module` and assemble the same struct. A shared helper that takes a closure from name to source could deduplicate the construction step. The cost-benefit is borderline because the lists are static.
2. **Position-coupled name list in `reload_scripts`.** The `names` array and the `drain.next().unwrap()` chain that populates `AiModules` must stay in lockstep. A named-pair iteration would catch ordering bugs at compile time. Several patterns work, including a const array of `(name, fn(&mut AiModules, Module))` setters.
3. **The `dispatch_*` methods on `AiPool` share a six-line shape.** Build the args array, call the named virtual machine, map the error string, unpack the result. The `unpack_finished_*` helpers already factor the result-decoding step. A `call_pure(&mut Vm, name, args)` wrapper could absorb the remaining boilerplate. Marginal; risks over-abstraction.
4. **`pickup_with_flag` and `random_with_flag` in `rogue_dungen.kel` share scratch slots in the data segment.** The current invariant is "strictly sequential calls". A future refactor that interleaves the helpers would silently corrupt state. Adding per-helper scratch slots would be cheap insurance.
5. **The 4587871 commit deletes the wall-fill but the spanning-tree pass from 8450abc is still in the generator.** With the wall-fill gone, the connectivity-flag array is no longer load-bearing for connectivity. It still adds layout variety but the data-segment cost and the three helpers may not be worth the variety. Deciding to roll back to a simpler chain is reasonable.

## Intended Next Step

Awaiting operator prompt. The example is in a clean, playable state. The corpse mechanism gives the player a fallback against starvation; the halved hunger cadence further extends viable run length; the dungeon generator no longer produces unreachable pockets. Candidate next moves:

1. **Pick up Exercise 3.7's Remove Curse to add a real consequence to cursed gear.** Adds one host-side flag, one swap-rejection check, and a status code 10 handler. About thirty lines of code.
2. **Implement Exercise 2.3's doors at corridor-room crossings.** Visually informative; gives the door tile a non-cosmetic role.
3. **Roll back the spanning-tree pass.** With the carve-floor-only fix, the chain shape is correct. Removing the spanning-tree pass simplifies the script and removes the scratch-slot reentrancy footgun. Trade off less corridor variety.
