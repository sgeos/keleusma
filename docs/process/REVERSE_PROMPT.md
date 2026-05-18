# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-17
**Status**: Connectivity refactor applied to the rogue example's dungeon generator. The rigid `R[i] -> R[i+1]` chain is replaced by a spanning-tree growth loop driven by per-room connectivity flags. The change addresses the operator-reported "small disconnected room" artifact observed under magic mapping.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Corpse drops with bestiary-defined effects | Implemented in commit `f39e7c4`. See prior reverse prompt entry. |
| Document loop-main reset semantics | Implemented in commit `f39e7c4`. See prior reverse prompt entry. |
| Spanning-tree corridors with connectivity flags | The dungeon generator script gains a `connected: [Word; 8]` array plus two scratch slots in the data segment. Three helper functions, `count_with_flag`, `pick_with_flag`, and `random_with_flag`, walk the flag array. The `fn main` body resets all flags to zero, flags room zero, and then runs seven iterations of the spanning-tree growth loop. Each iteration draws a corridor from a uniformly chosen connected source to a uniformly chosen unconnected target and flags the target. After seven iterations every room is reachable from room zero. The change ships as a script-only refactor; the host natives are unchanged. The `dungen_runs_floor_1` and `dungen_runs_floor_100_places_exit` integration tests both pass against the new shape. |

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Notes

- The diagnosis of the operator-reported disconnected-room artifact is that random room placement allows overlap. When a later room is laid down, its perimeter wall can carve a wall line through an earlier room's floor, subdividing the earlier room into a connected island around the new stored centre and a separated pocket. The chain corridor breaches the new wall at exactly one cell (the stored centre), so the pocket can remain unreachable from the player. The spanning-tree refactor does not eliminate the underlying overlap; it spreads breaches across a random topology so the probability of any single pocket being isolated is reduced. A complete fix would also reject overlapping placements at room-generation time or run a connectivity check at end of generation and dig extra corridors to reach unflagged regions. Both options are tractable follow-ups if the artifact persists in practice.
- The `connected` flag is stored in the data segment. Because the dungeon generator is `fn main` and the data segment persists across calls, the flags must be explicitly reset at the top of every call. The script does this in the room-placement loop so the cost is hidden in an existing pass.
- The `random_with_flag(0)` and `random_with_flag(1)` helpers use the same scratch slots `state.scratch_idx` and `state.scratch_pick`. They are not reentrant. The current call shape is sequential, so this is safe; a future refactor that interleaves the helpers must allocate distinct scratch slots.

## Intended Next Step

Awaiting operator prompt. Candidate next directions:

1. **Add a connectivity post-pass to the dungeon generator.** Flood-fill from room zero's centre at the end of generation. For any unreached floor region, dig a straight corridor from a reached cell to the region. This is a defensive measure against rare overlap configurations that the spanning-tree shape still permits.
2. **Reject overlapping room placements.** Augment `place_room` with a rejection sampler that retries up to N times before accepting. Requires no host-side changes. Eliminates the donut-pocket artifact entirely at the cost of slightly less variable layouts.
3. **Surface corpse messaging variety.** The current "You eat the rat corpse" message reads the same for every kind. A second message tier keyed on the bestiary shape could replace "eat" with shape-appropriate verbs ("gnaw on the insect", "tear off a chunk of the dragon", "swallow the slime essence").
