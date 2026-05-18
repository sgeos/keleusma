# Script Examples

Standalone Keleusma scripts. Each file demonstrates one feature axis. Run any of them with:

````
keleusma run examples/scripts/<file>.kel
````

| File | Topic | Feature |
|------|-------|---------|
| [`01_arithmetic.kel`](./01_arithmetic.kel) | Primitives and operators | `i64`, `f64`, `bool`, arithmetic, comparison, casts |
| [`02_struct_field.kel`](./02_struct_field.kel) | Composite types | Struct declaration, construction, field access |
| [`03_enum_match.kel`](./03_enum_match.kel) | Pattern matching | Enum declaration, variant construction, `match` |
| [`04_for_in.kel`](./04_for_in.kel) | Bounded iteration | `for` over arrays and ranges |
| [`05_pipeline.kel`](./05_pipeline.kel) | Pipeline operator | `\|>` left-to-right composition |
| [`06_multiheaded.kel`](./06_multiheaded.kel) | Function dispatch | Pattern-matched parameter heads |
| [`07_fstring.kel`](./07_fstring.kel) | String interpolation | `f"text {expr}"` desugaring |
| [`08_method_dispatch.kel`](./08_method_dispatch.kel) | Traits and impls | Receiver-style method calls |

All scripts in this directory's top level are atomic-total (`fn main`), so they run end to end through the CLI. For yield-driven and stream-driven examples, see the Rust embedding examples under [`examples/`](../).

## Example-specific scripts

The Rust embedding examples ship their own Keleusma script rosters in subdirectories of this folder. These scripts are not meant to be run standalone through `keleusma run`; they are loaded by their respective host through `include_str!` or hot reloaded from disk.

| Directory | Companion host | Description |
|-----------|---------------|-------------|
| [`piano_roll/`](./piano_roll/) | [`examples/piano_roll.rs`](../piano_roll.rs) | Ten songs (`piano_roll_0.kel` through `piano_roll_9.kel`) for the SDL3 audio piano-roll example. See [`docs/guide/PIANO_ROLL.md`](../../docs/guide/PIANO_ROLL.md). |
| [`rogue/`](./rogue/) | [`examples/rogue/main.rs`](../rogue/main.rs) | Fourteen scripts driving the SDL3 roguelike. One game-tick loop, one dungeon generator, one player artificial-intelligence script, eight monster artificial-intelligence archetypes, one combat-math script, two item-effect scripts. See [`docs/guide/ROGUE.md`](../../docs/guide/ROGUE.md). |
