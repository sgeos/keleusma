# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Cookbook section created with the data-loader pattern as the first recipe. Weapons and armors migrated into `rogue_gear.kel`. The example now ships twenty-four Keleusma scripts.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Create a cookbook section | New `docs/guide/COOKBOOK.md`. The data-loader recipe explains the problem, three component techniques, a minimal three-entry example with host code, two variations (multiple tables in one script, chained dispatchers), when to use, when not to use, and pointers at the production examples in this repository. Linked from `docs/guide/README.md` and the quick-reference table in `docs/README.md`. Cross-referenced from the bestiary section of `ROGUE.md`. |
| Note runtime hot-swap | The cookbook explicitly calls out that the pattern admits runtime hot reload even when the host caches. The Rogue example caches once at startup and does not reload the bestiary or gear scripts today, but the pattern itself does not preclude reload. An end-of-recipe note flags lifting these scripts into the F5 reload chain as a future iteration. |
| Migrate the equipment scripts | New `rogue_gear.kel` with twenty weapons and twenty armors in two dispatchers sharing one data segment. `items.rs` keeps the `Weapon` and `Armor` structs and gains `WEAPON_NAMES` and `ARMOR_NAMES` parallel constant arrays. `WEAPONS` and `ARMORS` become `OnceLock<Vec<_>>` populated by `items::install_weapons` and `items::install_armors`. New `items::weapons()` and `items::armors()` accessor functions replace direct `WEAPONS[idx]` access at eight call sites. The host's `main.rs::load_gear` function runs the script with `(table, -1)` to discover each table's count then iterates each table to populate the cache. |

## Why this round mattered

The cookbook is the principal artifact. The bestiary and gear migrations were already in place; the cookbook lifts the pattern out of the rogue manual and into a general recipe other embedders can use. It also positions the pattern as one of a future set of recipes, with the cookbook directory ready to accept additions.

The equipment migration is the second worked instance of the pattern in this codebase. It demonstrates the "multiple tables in one script" variation that the cookbook calls out. Modders who want to retune weapon damages or armor defenses can now edit a single `.kel` file rather than `items.rs`; for a designer-focused workflow this is the right partition.

## Line-count delta

| Category | Before | After | Delta |
|----------|-------:|------:|------:|
| Host (Rust) | 4724 | 4774 | +50 |
| Scripts (Keleusma) | 1655 | 1738 | +83 |
| **Total** | **6379** | **6512** | **+133** |

The migration is LOC-positive, as predicted. The weapons and armors tables were already at one line per entry in Rust, so moving them to a script could not compress further. The added cost is the script's dispatchers and the host's loader. Plus the cookbook itself adds a couple hundred lines to the docs tree.

This is the trade documented in the cookbook's "when not to use" section. The migration was undertaken anyway because the user explicitly valued modder-side accessibility and a second worked example of the pattern.

## Verification matrix

```bash
cargo test                                                                    # 571 tests, all pass (added 3 gear tests)
cargo test --features text --test rogue_scripts                               # 52 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Three new tests

- `gear_compiles` confirms the script passes the verifier.
- `gear_weapon_zero_is_fists_damage_two` cross-checks one weapon entry.
- `gear_armor_negative_one_is_last_guard_defense_forty` exercises the discovery convention against the second table.

## Intended Next Step

Awaiting operator prompt. Candidate next moves:

1. **Lift the bestiary and gear scripts into the F5 hot-reload path.** The pattern admits it; the example does not yet exercise it. The cookbook recipe explicitly notes this as a deferred iteration.
2. **Add a second cookbook recipe.** Candidates: the loop-main reset pattern (already covered prose-side in `ROGUE.md`), the natives-as-fine-grained-primitives pattern that `rogue_consume.kel` and `rogue_scroll_apply.kel` exemplify, or the spanning-tree corridor pattern with connectivity flags. Each could be a stand-alone recipe.
3. **Pick up one of the placeholder scroll handlers (Exercise 3.7)**, tune starvation (Exercise 5.1), or the background-music exercise (Exercise 2.6).
