# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Corpse stats moved into the bestiary script as a second multi-headed dispatcher keyed on shape. The manual gains a section on the data-loader pattern that the bestiary script demonstrates.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Discuss whether the data-loader pattern is novel | Partial agreement, noted in the user-facing reply. The three component techniques (multi-headed dispatch encoding a constant table, data segment as host-script I/O struct, negative-index size discovery) are individually known. The composition is well-suited to Keleusma's constraints and worth documenting as a Keleusma idiom; whether it is "novel" in the wider sense is hard to judge without a literature review. |
| Document the data-loader pattern in the manual | New subsection *The data-loader pattern* under *Reading the bestiary script*. Names the three techniques, explains why each fits Keleusma, and points at the corpse-data migration as a second worked instance of the pattern. |
| Migrate any other data that can move | Corpse stats per shape now live in `rogue_bestiary.kel` as a twelve-head `corpse_fill(N)` dispatcher. The bestiary's `fn main(n)` calls `fill(i)` to set base stats including shape, then `corpse_fill(state.shape)` to derive the corpse fields. `MonsterKind` gains three direct fields (`corpse_drop_chance: u8`, `corpse_satiation: i32`, `corpse_hp_delta: i32`) and loses the three methods that previously matched on shape. Callers in `combat.rs` and `natives.rs` swap from method calls to field access. |
| Decline other migrations | The weapons and armors tables in `items.rs` are already as dense as their Keleusma equivalent would be (one line per entry). Migrating them is LOC-negative without architectural gain; I left them alone. The reverse prompt records this decision. |

## Why corpse data was a good migration

Before: three methods on `MonsterKind`, each matching on `Shape` with twelve arms returning a constant. Around fifty lines of host code for three twelve-entry tables.

After: one twelve-head `corpse_fill(N)` dispatcher in the bestiary script. The script-side encoding is denser, and the corpse data sits next to the bestiary entries it describes. Modders edit the corpse table in the same file as the monster table.

The migration also strengthens the data-loader pattern's documentation: the bestiary script now contains two dispatchers keyed on different axes (one hundred entries by monster id, twelve entries by shape), chained inside `fn main`.

## Line-count delta

| Category | Before | After | Delta |
|----------|-------:|------:|------:|
| Host (Rust) | 4761 | 4724 | -37 |
| Scripts (Keleusma) | 1633 | 1655 | +22 |
| **Total** | **6394** | **6379** | **-15** |

Modest LOC reduction. The migration was about architectural placement rather than line count. The bestiary script grew by twenty-two lines (twelve corpse-fill heads plus three new data slots plus the dispatcher call in `fn main`). `bestiary.rs` shrank by about fifty lines (three methods × roughly fifteen lines each) plus the three new struct fields. Callers updated from method calls to field access.

## Verification matrix

```bash
cargo test                                                                    # 568 tests, all pass (added 1 corpse-data test)
cargo test --features text --test rogue_scripts                               # 49 rogue script tests, all pass
cargo clippy --workspace --tests --features sdl3-example,text -- -D warnings  # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Notes

- The data-loader pattern subsection in the manual is the principal artifact of this round. The corpse-data migration is the second worked example that the section references; it strengthens the pattern's documentation without being load-bearing on its own.
- Exercise 4.3 in the manual is reframed. The original asked whether the bestiary could be moved to a script; that question is now answered. The remaining open question is whether monster names could move too, which would require a Keleusma string-pool feature or a static-string-handle native.

## Intended Next Step

Awaiting operator prompt. Candidate next moves: pick up one of the placeholder scroll handlers (Exercise 3.7), tune starvation (Exercise 5.1), pick up the background-music exercise (Exercise 2.6), or invest in the string-pool feature that would let monster names move into the bestiary script.
