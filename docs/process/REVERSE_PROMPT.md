# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-17
**Status**: Root cause of the disconnected-room artifact fixed. `carve_room` no longer writes walls over the new room's rectangle before carving the interior. Overlapping rooms now merge their floors into one connected region instead of subdividing each other.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Corpse drops with bestiary-defined effects | Commit `f39e7c4`. |
| Document loop-main reset semantics | Commit `f39e7c4`. |
| Spanning-tree corridors with connectivity flags | Commit `8450abc`. Reduces but does not eliminate the donut-pocket failure mode. |
| Carve floor only, no destructive wall-fill | Current commit. The first line of the prior `carve_room`, `fill_rect(x0, y0, x1, y1, 1)`, is removed. Rooms now rely on the solid wall left by `host::clear_floor` for their outlines. Two overlapping rooms merge their interiors into one connected floor region. The chain-corridor failure mode that this fix targets, in which a later room's wall fill subdivides an earlier room's floor, is no longer possible because the wall fill no longer happens. |

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Notes

- The fix is a one-line deletion in the dungeon generator script. The host natives are unchanged.
- The spanning-tree corridor pass from commit `8450abc` is retained. It is no longer strictly necessary for connectivity but it improves visual variety and makes the generator robust to future changes that might reintroduce destructive carving.
- The integration tests pass on the new shape. The `dungen_runs_floor_1` test exercises `map_set` more than one hundred times, which still holds because the corridor pass writes floor along the L-shapes regardless of whether the rooms wrote walls first.

## Intended Next Step

Awaiting operator prompt. With the root cause fixed, the remaining backlog around the dungeon generator is purely aesthetic or efficiency-oriented:

1. **Optional. Remove the spanning-tree pass.** The chain `R[i] -> R[i+1]` is now correct because overlapping rooms no longer subdivide each other. The spanning-tree pass is a defence-in-depth measure with a small data-segment cost. Keeping it gives more varied corridor topology; removing it simplifies the script back to roughly its prior shape.
2. **Optional. Smarter corridor routing.** Bend the L-shape's horizontal-or-vertical-first choice randomly so the L direction varies. Cheap; one extra `rng_range` call per corridor.
3. **Optional. Door tile placement at corridor-room boundaries.** Identify where a corridor enters a room and place a `DoorClosed` tile at that cell. Adds visual interest and gives the player something to interact with.
