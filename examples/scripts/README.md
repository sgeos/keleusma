# Script Examples

Standalone Keleusma scripts. Each file demonstrates one feature axis. Run any of them with:

````
keleusma run examples/scripts/<file>.kel
````

| File | Topic | Feature |
|------|-------|---------|
| [`01_arithmetic.kel`](./01_arithmetic.kel) | Primitives and operators | `Word`, `Float`, `bool`, arithmetic, comparison, casts |
| [`02_struct_field.kel`](./02_struct_field.kel) | Composite types | Struct declaration, construction, field access |
| [`03_enum_match.kel`](./03_enum_match.kel) | Pattern matching | Enum declaration, variant construction, `match` |
| [`04_for_in.kel`](./04_for_in.kel) | Bounded iteration | `for` over arrays and ranges |
| [`05_pipeline.kel`](./05_pipeline.kel) | Pipeline operator | `\|>` left-to-right composition |
| [`06_multiheaded.kel`](./06_multiheaded.kel) | Function dispatch | Pattern-matched parameter heads |
| [`07_refinement.kel`](./07_refinement.kel) | Refinement types | `newtype Name = Underlying where predicate;` with compile-time literal elision and runtime construction check |
| [`08_method_dispatch.kel`](./08_method_dispatch.kel) | Traits and impls | Receiver-style method calls |
| [`09_big_numbers.kel`](./09_big_numbers.kel) | Big-number arithmetic | Pattern-matched checked arms binding `(high, low)` halves of an `i128` intermediate |
| [`10_multbyte.kel`](./10_multbyte.kel) | Byte-typed arithmetic | Multiplication on `Byte` operands |
| [`11_signed.kel`](./11_signed.kel) | Signed compiled module | `signed` modifier on the entry function, Ed25519 signature flow through the CLI |
| [`12_sensor_window.kel`](./12_sensor_window.kel) | Per-iteration composite, confined | A struct built once per `for` iteration and consumed inside it |
| [`13_telemetry_stream.kel`](./13_telemetry_stream.kel) | Per-iteration composite, yielded | `loop main` streaming a struct built inside a `for`. **Not runnable through `keleusma run`** — see below |
| [`14_frame_log.kel`](./14_frame_log.kel) | Per-iteration composite, stored | A struct copied into a `private data` slot each iteration, surviving the stream's `Reset`. **Diverges under `keleusma run`** — see below |

**The scripts below exist to WITNESS OPCODES, not to demonstrate a language feature.** They are the
`v0.3.X` line's, added so the native-lowering censuses have something that emits each instruction;
several are deliberately inadmissible or deliberately refused, and that is their purpose rather than
a defect. Each carries a machine-checked `// WITNESSES:` line that `witness_integrity.rs` verifies
against what the file actually emits.

| File | Topic | Feature |
|------|-------|---------|
| [`opcode_witness.kel`](./opcode_witness.kel) | Opcode witness, lowered | Emits opcodes no application emits, all of which the native backend lowers |
| [`refused_witness.kel`](./refused_witness.kel) | Opcode witness, refused | Concentrates the opcodes the backend still refuses, so their refusal does not exempt an otherwise-lowering module. **Cannot be given an arena at all** while `len_witness` is present — deliberate |
| [`float_witness.kel`](./float_witness.kel) | Float opcodes | Split out because one float constant refused a whole module and took four unrelated opcodes' witnesses with it |
| [`fixed_arithmetic.kel`](./fixed_arithmetic.kel) | Fixed-point arithmetic | `Fixed<N>` multiply, divide and the `Word` conversions |
| [`fixed_conversions.kel`](./fixed_conversions.kel) | Fixed-point conversions | `WordToFixed` and `FixedToWord` across widths |
| [`external_native_witness.kel`](./external_native_witness.kel) | External native call | `CallExternalNative`, which needs host registration rather than a verified native |

Scripts `01` through `12` are atomic-total (`fn main`) and run end to end through the CLI.

**`13` and `14` are `loop main` stream programs and neither terminates under `keleusma run`.** They are here because the analyses that read this directory need the shape, not because they are runnable demonstrations:

- **`13_telemetry_stream.kel` is refused outright.** The command requires a `loop main` to yield `Word`, and this one yields a composite — which is the whole point of the example. Drive it from a host with `call_with_shared` / `resume_with_shared`.
- **`14_frame_log.kel` runs forever**, because a `loop` function is productively divergent by design. Interrupt it, or drive it from a host.

**This sentence used to say every top-level script was atomic-total, and adding `13` and `14` made that false.** It is corrected rather than quietly left, because a reader takes an invariant like that at face value.

For further yield-driven and stream-driven examples, see the Rust embedding examples under [`examples/`](../).

## Example-specific scripts

The Rust embedding examples ship their own Keleusma script rosters in subdirectories of this folder. These scripts are not meant to be run standalone through `keleusma run`; they are loaded by their respective host through `include_str!` or hot reloaded from disk.

| Directory | Companion host | Description |
|-----------|---------------|-------------|
| [`piano_roll/`](./piano_roll/) | [`examples/piano_roll.rs`](../piano_roll.rs) | Ten songs (`piano_roll_0.kel` through `piano_roll_9.kel`) for the SDL3 audio piano-roll example. See [`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md). |
| [`rogue/`](./rogue/) | [`examples/rogue/main.rs`](../rogue/main.rs) | Nineteen scripts driving the SDL3 roguelike. Game-tick loop, dungeon generator, player artificial intelligence, combat math, book-keeping, autopickup decision, movement resolution, ten monster artificial-intelligence archetypes including three `loop main` archetypes (Boss, Tracker, Hunter), two item-effect scripts. See [`book/src/ROGUE.md`](../../book/src/ROGUE.md). |
