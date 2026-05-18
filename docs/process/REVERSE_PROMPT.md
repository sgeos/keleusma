# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Bestiary migrated from host-side Rust constant to a Keleusma script. The host runs the script once per monster id at startup and reads the data segment. Resolved entries cache in a `OnceLock`-backed `Vec<MonsterKind>` so runtime accesses remain plain reads.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Move the bestiary into a Keleusma script | New `rogue_bestiary.kel`. The script declares a sixteen-slot data segment matching the `MonsterKind` numeric fields and one hundred multi-headed `fill(N)` functions that write the constants for entry `N` into the data segment. The `fn main(n)` entry point resolves negative indices to `MONSTER_COUNT + n` and dispatches to the matching head. |
| Negative-index discovery convention | Calling `fn main(-1)` writes the last entry; reading slot zero (the resolved `id`) yields `MONSTER_COUNT - 1`. The host uses this on first call to learn the table size and asserts it matches the host-side `MONSTER_NAMES` array length. |
| Loader function fills a `.data` struct | The script's data segment is the `.data` struct. The host writes input via the function argument (Keleusma's `fn main` already accepts integer arguments) and reads outputs via `vm.get_data(slot)`. No language change was required because the data segment is already accessible to the host through the existing `get_data`/`set_data` boundary. |
| Host-side refactor | `bestiary.rs` shrinks from 1552 lines to 299. The `BESTIARY: [MonsterKind; 100]` static is replaced by a `OnceLock<Vec<MonsterKind>>`. The `kind(idx)` accessor stays. Two new helpers, `Shape::from_ord` and `AiKind::from_ord`, decode ordinals. A parallel `MONSTER_NAMES: [&str; 100]` constant holds names because Keleusma's data segment does not currently support inline strings. |
| Bestiary loading at startup | `main.rs::load_bestiary` runs the script once with `-1` for discovery, asserts the count matches `MONSTER_NAMES.len()`, then runs it once per id and calls `bestiary::install(table)`. |

## Line-count delta

| Category | Before | After | Delta |
|----------|-------:|------:|------:|
| Host (Rust) | 5932 | 4761 | -1171 |
| Scripts (Keleusma) | 1383 | 1633 | +250 |
| **Total** | **7315** | **6394** | **-921** |
| Script share | 18.9% | 25.5% | +6.6pp |

The migration deletes nine hundred and twenty-one net lines. The single biggest source of savings is the per-entry density: the prior Rust struct literal cost fourteen lines per entry; the Keleusma `fill(N)` form fits on one line per entry. With one hundred entries the savings are immediate.

## Verification matrix

```bash
cargo test                                                                    # 567 tests, all pass (added 3 bestiary tests)
cargo test --features text --test rogue_scripts                               # 48 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Three new tests

- `bestiary_compiles` confirms the script passes the verifier.
- `bestiary_negative_one_returns_last_entry` verifies the discovery convention.
- `bestiary_entry_zero_is_sewer_rat_stats` cross-checks one entry's numeric fields against the prior table.

## Notes

- No language change was required. The user anticipated one (`fn main` accepting a `.data` struct); in practice Keleusma's data segment is already accessible to the host via `get_data`/`set_data`, so the script's data segment serves as the I/O struct directly. The argument to `fn main` carries the monster index; the data-segment slots carry the output fields.
- Names stay in Rust as a constant array because there is no clean way to ship one hundred static string literals out of a script's data segment today. Exercise 4.3 in the manual notes this as a deferred research question.
- The bestiary is loaded once at startup and not hot-reloadable. The `OnceLock` cannot be re-set. A future change could swap the cache for a `RwLock<Vec<MonsterKind>>` to support F5 reload of the bestiary, but that is not required for the demonstration.

## Intended Next Step

Awaiting operator prompt. Candidate next moves: pick up one of the placeholder scroll handlers (Exercise 3.7), tune starvation (Exercise 5.1), pick up the background-music exercise (Exercise 2.6), or invest in support for shipping static string tables out of a Keleusma script so names could move too.
