# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-18
**Status**: Background-music hook documented in the source and surfaced as a manual exercise. No surveyed refactoring or deferred work outstanding beyond what is already framed as manual exercises.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Add `add audio processing here` comment | A ten-line block comment lands in `fn main` right after the SDL3 video subsystem init in `examples/rogue/main.rs`. The comment names the piano-roll example as the reference pattern, lists the four pieces of work (audio device open, voice-array `Arc<Mutex<_>>`, audio callback, music script driving triggers), and cross-references Exercise 2.6 in the manual. |
| Add an exercise for piano-roll-style background music | New Exercise 2.6 in the Tier two section of `docs/guide/ROGUE.md`. Spells out the piano roll's audio pipeline, the script-side work (a new `rogue_music.kel` `loop main` script), the host-side work (audio device open, audio thread, per-tick or per-floor resume), and the concurrency concerns. References the source-code hook in `main.rs`. |
| Note the source-code comment in the manual | Exercise 2.6 names the `add audio processing here` comment explicitly so a reader following the manual can grep the source to find the landing site. |

## Survey of remaining deferred work

All intentional. None block gameplay.

- **Exercise 3.7.** Sleep, Confusion, and Remove Curse scroll-effect handlers in `examples/rogue/natives.rs::apply_scroll_status` are placeholder messages. The script side produces correct status codes; the host side is the exercise.
- **Exercise 3.6.** `monster_sees_player` uses the player's field-of-view bitmap rather than a per-monster shadowcast.
- **Tier 2 exercises generally.** Coward archetype, agility potion, doors at corridor-room boundaries, reveal-all-monsters scroll, vault generator, background music.
- **Tier 3 and 4 exercises.** Ray-casting field-of-view, save and load, script-driven turn loop, sprite-sheet renderer, ranged player attacks, per-monster shadowcast, placeholder statuses, smallest-archetype-set research question, dungen worst-case execution time, bestiary-in-script, host versus script responsibility essay.
- **Tier 5 exercises.** Starvation tuning, difficulty curve calibration, loot distribution audit, combat damage scaling.

## Survey of remaining refactoring opportunities

I do not see any obvious patterns worth folding at this point. The last sweep collapsed every duplicate pattern I had surveyed. Concrete remaining candidates would need to come from a new feature pulling on the example. Examples that might surface candidates.

- **Audio integration (Exercise 2.6) would add an audio thread, a voice array, and a music-script lifecycle.** That work would likely refactor `fn main` to extract setup into a helper, which is a natural shape change rather than a deduplication. Not work to do until the feature lands.
- **Save and load (Exercise 3.2) would add serialisation paths through `World`.** Serialisation often surfaces field-grouping opportunities. Same point: wait for the feature.
- **Script-driven turn loop (Exercise 3.3) would invert the host-script dispatch direction.** A large refactor in itself; no pre-work helps.

The example reads cleanly across its host modules and its scripts. I would not recommend speculative refactoring at this point.

## Verification matrix

```bash
cargo test                                                                    # 564 tests, all pass
cargo test --features text --test rogue_scripts                               # 45 rogue script tests, all pass
cargo clippy --workspace --features sdl3-example,text -- -D warnings          # clean
cargo build --example rogue --features sdl3-example,text                      # clean
```

## Intended Next Step

Awaiting operator prompt. The natural next moves are picking up one of the placeholder scroll handlers (Exercise 3.7 sub-items), tuning starvation (Exercise 5.1), or picking up the background-music exercise (Exercise 2.6).
