# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-17
**Status**: Two operator directives applied to the rogue example. Monster corpses now drop with bestiary-defined frequency and effects, and the loop-main reset semantics behind game-over replay are documented in the player-facing manual.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Corpse drops with bestiary-defined effects | New `ItemKind::Corpse` variant indexed at 6. `MonsterKind` gains three shape-derived methods: `corpse_drop_chance`, `corpse_satiation`, `corpse_hp_delta`. Skeletons, ghosts, and slimes drop nothing. Larger creatures yield more meat. Serpents, insects, and mages are poisonous. The boss corpse is restorative. `combat::player_attacks` rolls against `corpse_drop_chance` on each kill and pushes a `Corpse` item onto the slain cell with `subtype = monster_kind_idx`. The autopickup driver gains a `Corpse` arm that reads the bestiary entry, applies the satiation and hit-point delta, and emits a flavoured message based on the delta's sign. The pickup decision script accepts kind 6 in the always-consume branch. The tile atlas gains a corpse silhouette. Bottleneck-luring strategy: kill poisonous monsters in open rooms so the corpse can be stepped around; corridor kills force the player to either eat the corpse or stop using that corridor. |
| Document loop-main reset semantics | New subsection `### Restarting a loop main script after game-over` added to `docs/guide/ROGUE.md`. Explains that data-segment zeroing alone is sufficient because (a) body locals do not persist past a yield, (b) the loop body re-reads world state through natives on every iteration, and (c) the Reset boundary that already separates one turn from the next handles the wrap from the body's tail back to the top. The reset is implemented by `AiPool::reset_loop_main_data` which walks the three loop-main archetypes (Boss, Tracker, Hunter) and writes zero across each data slot. No virtual-machine rebuild is required. Combined with a fresh `World` and a regenerated dungeon, the next dispatch produces a clean replay equivalent to the first run. |

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Notes

- The pickup script's per-kind ID legend in the file header was updated to include corpses. The kind table in `ROGUE.md#item-kind-identifiers` was extended.
- Corpse subtype carries the monster kind index so the consumption arm can look up `bestiary::kind(subtype)` directly. This keeps all per-kind tuning in the bestiary and out of the autopickup driver.
- The corpse-drop branch in `combat::player_attacks` rolls a fresh number against the drop chance; the slain monster is removed first so the world borrow does not overlap with the item push.

## Intended Next Step

Awaiting operator prompt. The rogue example now ships with a starvation safety valve and an explicit explanation of the replay path. Candidate next directions:

1. **Add bestiary-driven flavour to the corpse message.** Today the message reads "You eat the rat corpse." A second message tier keyed on the shape (insect, serpent, dragon) could replace "eat" with shape-appropriate verbs. Pure host-side; no script changes.
2. **Hand corpse-effect computation to a Keleusma script.** Currently the host queries the bestiary directly. Moving the lookup into `rogue_item_corpse.kel` would mirror the potion and scroll pattern and tighten the thin-client architecture.
3. **Surface the corpse-restorative outlier in the manual's exercises.** The boss corpse currently grants +8 HP because shape Boss has a positive `corpse_hp_delta`. This is a deliberate but undocumented reward for the final kill; an exercise to add a victory message reinforces the design choice.
