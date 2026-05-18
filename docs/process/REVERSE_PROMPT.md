# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Monster names migrated into the bestiary script via a `Text`-returning dispatcher. The host extracts the name from the script's return value, leaks it once at startup, and stores `&'static str` in `MonsterKind::name`. Cookbook trimmed of example-specific narrative.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Trim cookbook of example-specific details | `docs/guide/COOKBOOK.md` is reduced to a generic recipe. The minimal example renames from `rogue_colours.kel` to `colours.kel`. Production-example pointers are reduced to a single sentence at the bottom of the recipe linking to `ROGUE.md`. The variation notes drop their references to specific scripts. |
| Migrate monster names into the bestiary script | New `fn name(N) -> Text` dispatcher in `rogue_bestiary.kel`, one head per monster. The script's `fn main` changes return type from `Word` to `Text` and ends with `name(i)` so the call's `Finished(StaticStr)` payload carries the name. The host's `load_bestiary` extracts the static string from the `VmState` payload, leaks it once for `&'static str`, and threads it into `read_bestiary_entry`. The `MONSTER_NAMES: [&str; 100]` constant is removed from `bestiary.rs`. |
| Cookbook variation: names through the return | The cookbook's variations section gains a new variation, *Names through the return value*, describing the pattern in generic terms. |

## Why the names migration worked

Keleusma's surface grammar does not yet expose `StaticStr` as a data-segment field type. The data segment is therefore not the right place for entry names. But the language already admits string literals in expression position and admits `Text` as a function return type (the f-string example demonstrates this). A multi-headed dispatcher returning `Text` is the natural workaround. The host receives the string as the call's payload, leaks it, and caches a `&'static str`.

The leak is bounded by the table size. The bestiary holds one hundred names averaging roughly ten bytes each. Leaking once at startup costs a few kilobytes and yields a `'static` lifetime suitable for the existing `MonsterKind::name: &'static str` field. No language change was required.

## Line-count delta

| Category | Before | After | Delta |
|----------|-------:|------:|------:|
| Host (Rust) | 4774 | 4684 | -90 |
| Scripts (Keleusma) | 1738 | 1844 | +106 |
| **Total** | **6512** | **6528** | **+16** |

The host shrinks by ninety lines (the deleted name array, the simplified `read_bestiary_entry` signature, the swap from `MONSTER_NAMES.len()` to `MONSTER_COUNT`). The script grows by one hundred and six lines (one hundred `name(N)` heads plus the default plus the `name(i)` call in `fn main` plus a few lines of documentation). The total grows by sixteen lines.

The increase is purely the cost of the per-name header line in script form. Each `name(N)` head is one line for a one-line value, comparable to the prior `MONSTER_NAMES` array which was also one line per entry. The migration is LOC-neutral within noise; the architectural gain is single source of truth.

## Verification matrix

```bash
cargo test                                                                    # 572 tests, all pass (added 1 name test)
cargo test --features text --test rogue_scripts                               # 53 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Notes

- The leaked-string pattern is intentional. The alternative would be to change `MonsterKind::name` to `String` (owned), which would force a clone or borrow at every call site. Leaking at startup keeps the existing `&'static str` API and pays a one-time cost of a few kilobytes.
- Exercise 4.3 is updated. The remaining open question is the weapon and armor names in `rogue_gear.kel`. Applying the same pattern there is mechanical given the bestiary precedent.

## Intended Next Step

Awaiting operator prompt. Candidate next moves:

1. **Migrate weapon and armor names too.** Apply the same return-value-as-name pattern to `rogue_gear.kel`. The mechanical work is the same; the gain is removing `WEAPON_NAMES` and `ARMOR_NAMES` from `items.rs`.
2. **Lift the bestiary and gear scripts into the F5 hot-reload path.** The pattern admits it; the example does not yet exercise it.
3. **Add a second cookbook recipe.** Candidates noted in the prior reverse prompt.
4. **Pick up an open exercise from the manual.**
