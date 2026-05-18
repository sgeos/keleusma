# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Weapons and armor expanded to twenty tiers. All three previously surveyed host-to-script migrations applied. The example now ships twenty-two Keleusma scripts versus nineteen before.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Expand weapons and armor to twenty tiers | Ten new entries appended to each table in `examples/rogue/items.rs`. Weapons run from `fists` at two damage to `last word` at one hundred and eighteen damage. Armors run from `rags` at zero defense to `last guard` at forty defense. The dungeon generator's tier-scaling formula changes from `(f / 10) + 2` capped at nine to `(f / 5) + 1` capped at nineteen so high-tier gear actually appears on deep floors. The head-up display's tier-pip strip widens from ten to twenty pips and the armor icon shifts right to accommodate. The compact `Weapon { name, damage }` struct-literal syntax keeps `items.rs` shorter than before despite the doubled row count. |
| Migration 3a: descend level-up into a script | New `rogue_descend.kel`. The script takes the player's `(level, max_hp, hp, skill, floor)` and returns the post-descent five-tuple. The host applies the returns. The shipped progression keeps the prior arithmetic (hp+3, skill+1, level+1, floor+1) so gameplay is unchanged. `descend_floor` in `main.rs` is now a snapshot, dispatch, apply sequence rather than inline arithmetic. |
| Migration 3b: starvation message flag | `rogue_book_keeping.kel` now returns a three-tuple `(new_hp, new_hunger, starving_msg)`. The script computes the starving-message flag from the input `hunger` and `turn` values. The host pushes the message when the flag is set rather than carrying the `was_hungry` plus `turn.is_multiple_of(5)` check itself. |
| Migration 2: autopickup consumption into a script | New `rogue_consume.kel`. After `rogue_pickup.kel` returns `consume`, the host dispatches `rogue_consume.kel` with the item kind and subtype. The script dispatches to one of seven fine-grained natives: `host::consume_food`, `host::take_gold`, `host::equip_weapon`, `host::equip_armor`, `host::stash_potion`, `host::stash_scroll`, `host::eat_corpse`. Each native applies one item kind's world mutation and pushes its message. The autopickup driver's per-kind switch is gone from `natives.rs`. |
| Migration 1: scroll-status application into a script | New `rogue_scroll_apply.kel`. After `rogue_item_scroll.kel` returns its `(status_code, status_arg)` pair, the host dispatches `rogue_scroll_apply.kel`. The script dispatches to one of eight fine-grained natives: `host::set_explored_all`, `host::set_explored_radius`, `host::teleport_player_random`, `host::identify_all_potions`, `host::change_weapon_tier`, `host::change_armor_tier`, `host::sense_monsters`, `host::scroll_placeholder`. The host's `apply_scroll_status` ninety-line match is gone. The placeholder native handles the unimplemented Sleep, Confusion, and Remove Curse status codes. |
| Manual currency | The architecture diagram now lists twenty-two virtual machines. A new section, *Reading the consume and descend scripts*, summarises the three new scripts. The status-action paragraph in the item-effect section now notes that the dispatch itself runs in script. The tier-pip-scale text changes from "zero through nine" to "zero through nineteen". Exercise 5.4 updates the weapon-damage range. |

## Line-count delta

```
 docs/guide/ROGUE.md                           |  26 +-
 examples/rogue/ai.rs                          |  74 ++++-
 examples/rogue/items.rs                       | 120 +++----
 examples/rogue/main.rs                        |  42 ++-
 examples/rogue/natives.rs                     | 442 ++++++++++++++++----------
 examples/rogue/render.rs                      |  13 +-
 examples/scripts/rogue/rogue_book_keeping.kel |  10 +-
 examples/scripts/rogue/rogue_dungen.kel       |   8 +-
 8 files changed, 448 insertions(+), 287 deletions(-)
```

Plus three new script files (`rogue_consume.kel` 53 lines, `rogue_descend.kel` 26 lines, `rogue_scroll_apply.kel` 62 lines).

Final totals.

| Category | LOC before | LOC after | Delta |
|----------|-----------:|----------:|------:|
| Host (Rust) | 5789 | 5932 | +143 |
| Scripts (Keleusma) | 1238 | 1383 | +145 |
| **Total** | **7027** | **7315** | **+288** |
| Script share | 17.6% | 18.9% | +1.3pp |

The total grows. This is the inherent cost of the migration. Each new fine-grained native adds host code (typically ten to fifteen lines), and each new script adds script code (typically thirty to fifty lines), so a migration that collapses one Rust match with eight arms into a script dispatching to eight natives net adds lines. The value is architectural rather than LOC-positive. Modders can now retune scroll status mapping, item consumption flavour, and per-floor level-up curves in script without touching the host.

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Notes

- The `register_unit_native` and `register_int_native` helpers absorb the boilerplate that would otherwise dominate the new natives. Without them the natives.rs growth would be twice as large.
- The consume and scroll-apply scripts compile against the embedded source table at startup and pick up disk-edited copies on `F5`. Their fine-grained natives are registered against each virtual machine in `AiPool::new` and re-registered in `AiPool::reload`.
- The host now imports `bestiary` and `items` directly in `natives.rs` for the per-kind consumption arms; the prior code reached through `World` accessors only. This is a small expansion of `natives.rs`'s dependency surface but localises the bestiary lookup to the eat-corpse arm where it semantically belongs.

## Intended Next Step

Awaiting operator prompt. The natural next moves are picking up one of the placeholder scroll handlers (Exercise 3.7), tuning starvation (Exercise 5.1), or picking up the background-music exercise (Exercise 2.6).
