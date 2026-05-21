# Keleusma Cookbook

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Recipes are working patterns for embedding Keleusma in larger systems. Each recipe states the problem it solves, the constraint it respects, and a minimal working example. Recipes link to the bundled examples where they instantiate the pattern at production scale; the linked sections are the place to read deeper.

## Index

| Recipe | Use it when |
|--------|-------------|
| [Working with `Text`](#working-with-text) | The host or scripts need to handle strings. |
| [Auto-sizing the arena from the module](#auto-sizing-the-arena-from-the-module) | The host wants exact `WCMU`-bounded arena sizing instead of a hardcoded capacity. |
| [The data-loader pattern](#the-data-loader-pattern) | The host needs read-only configuration data that benefits from script-side editing. |
| [Narrow-runtime type alias](#narrow-runtime-type-alias) | The host targets a sub-64-bit native runtime (16-bit or 8-bit signed word). |
| [Distributing signed bytecode](#distributing-signed-bytecode) | The host delivers compiled modules over an untrusted channel and needs origin authenticity. |

---

## Working with `Text`

### Problem

The host or a script needs to handle strings. Names, log messages, error reports, configuration values, identifiers from the outside world. Keleusma is not a value-add for string processing, but real applications routinely need some string work at the boundary.

### Solution

Two rules.

**One. Use string literals for static text.** Source-level string literals compile to `Value::StaticStr` and live in the bytecode's read-only constant pool. They are immutable, fixed-size handles, and admissible in function arguments, return values, and `yield` payloads. The script's surface type is `Text`; the runtime preserves the static-versus-dynamic distinction internally.

```keleusma
fn label() -> Text {
    "ready"
}
```

**Two. Register Rust functions for every text operation beyond literals.** V0.2.0 retired the bundled `concat`, `to_string`, `slice`, and `length` utility natives along with f-string interpolation. Script-side string composition flows through host-registered functions. Hosts that need formatting, splitting, regular expressions, Unicode operations, or encoding conversion register a Rust function and the script imports it through `use`.

```rust
vm.register_fn("text::upper", |s: String| -> String { s.to_uppercase() });
vm.register_fn("text::trim",  |s: String| -> String { s.trim().to_string() });
```

```keleusma
use text::upper
use text::trim

fn greet(name: Text) -> Text {
    text::upper(text::trim(name))
}
```

Host-produced dynamic strings reside in the arena heap as `Value::KStr` (arena-handled). They are admissible on the stack and in local bindings but cannot cross a `yield` boundary; the verifier rejects programs that would carry an arena-resident `KStr` across the host-VM boundary.

### Why this works for an RTOS or embedded target

Static strings live in the read-only data section and cost no allocation. A script that returns names or log labels through static strings consumes zero arena. Host-produced dynamic strings cost arena heap that the host attests through `register_verified_native(name, fn, wcet, wcmu_bytes)`; the verifier folds the per-call WCMU into the iteration budget. There is no path by which string work can grow unbounded; either it goes through a fixed-size static-string handle, or it counts against a verifier-bounded heap allocation, or it never compiles.

### Cross-references

- [FAQ.md, Strings](./FAQ.md#strings) covers the surface caveats and the static-string escape table.
- [TYPE_SYSTEM.md, Text Types](../spec/TYPE_SYSTEM.md#text-types) is the type-system specification.
- The rogue example's bestiary script returns monster names through this pattern.

---

## Auto-sizing the arena from the module

### Problem

Every Keleusma `Vm` needs an arena. The host picks the capacity. Pick too small and the verifier rejects the module at `Vm::new` with `VerifyError`; pick too large and the host wastes memory it does not need. Embedded targets in particular want exact sizing because they may not have a heap at all (the arena runs from a static `[u8; N]` buffer in `.bss`).

### Solution

Use `keleusma::vm::auto_arena_capacity_for(&module, native_wcmu)` to compute the minimum-required capacity from the compiled module before constructing the VM. The function walks the module's Stream chunks, sums each chunk's stack and heap WCMU, and returns the largest total. The result is the smallest capacity that admits the module under the supplied native attestations.

```rust
use keleusma::vm::{auto_arena_capacity_for, Vm};
use keleusma::Arena;

let cap = auto_arena_capacity_for(&module, &[])?;
let arena = Arena::with_capacity(cap);
let vm = Vm::new(module, &arena)?;
```

The second argument is a slice of per-native heap-allocation attestations. Pass an empty slice when no native allocates from the arena. Pass the appropriate `u32` values when the host has registered heap-allocating natives.

```rust
// Script that uses host-registered text or buffer natives. The
// host's per-call attestations flow through the slice in the
// same order as the module's `native_names` table.
let native_wcmu = &[upper_wcmu, trim_wcmu];
let cap = auto_arena_capacity_for(&module, native_wcmu)?;
```

### When to use which arena-sizing option

The library offers three patterns.

| Option | Use it when |
|--------|-------------|
| `Arena::with_capacity(DEFAULT_ARENA_CAPACITY)` | Hosted development and quick prototyping; a generous default capacity is acceptable. |
| `auto_arena_capacity_for` | Production hosts that want the smallest correct capacity, especially when running many VMs or when host memory is tight. |
| `Arena::from_static_buffer` | Bare-metal targets with no heap. The host owns a fixed-size buffer in `.bss` and hands its pointer to the arena. |

The auto-sizing option composes with the static-buffer option. Compute the capacity at compile time (if the module is `const`-loadable through `include_bytes!`) or at build time (running the host once to print the value), then declare the static buffer at that size.

### Failure mode

If the chosen capacity is below the module's WCMU, `Vm::new` returns `VmError::VerifyError` before any code runs. This is detected at construction time, not at execution time, so the failure is observable up front rather than in the middle of a run.

### Cross-references

- [EMBEDDING.md, Arena Sizing](./EMBEDDING.md#arena-sizing) is the embedding-guide reference.
- The bundled `examples/wcmu_basic.rs` shows the full auto-sizing pattern end to end.

---

## The data-loader pattern

### Problem

The host needs a table of configuration data. The data is structurally homogeneous (a fixed-shape record per entry) but designer-tunable (game balance, look-up tables, content). Storing the table in Rust source means designers must rebuild the host to retune. Storing it in a script file lets designers edit a `.kel` file and reload at runtime.

Keleusma does not currently support module-scope `const` declarations for arrays of records, inline string tables, or runtime allocation of growable structures. The pattern below works inside those constraints.

### Solution

Encode the table as a Keleusma script with three pieces.

1. **A data segment** declared on the script side, holding one field per output column of the record. The data segment is the host-script I/O struct.
2. **A multi-headed dispatcher** with one head per entry. Each head writes the per-entry constants into the data segment.
3. **A loader function** that resolves the index (including the negative-index convention) and chains into the dispatcher.

The host runs the script once per entry at startup, reads the data segment after each call, and caches the result in a regular Rust container (`Vec<T>`, `HashMap<K, T>`, or similar). After the cache is warm, runtime reads go through the Rust cache; the script is touched again only when the host wants to reload.

The pattern admits runtime hot reload because the table is in script form. A host that re-compiles the script, re-runs the loader, and atomically replaces the cache can swap data without restarting. A host that caches once at startup still benefits because the table moves out of Rust source and into a file that designers can edit.

### Three component techniques

The pattern composes three techniques that are individually known but compose well.

**Multi-headed dispatch encoding a constant table.** Keleusma admits multi-headed function definitions with integer-pattern parameters. One head per entry, each body assigning the entry's fields, is functionally equivalent to a constant array. The encoding is verifier-friendly because every body is straight-line code. Prolog facts and Erlang or Elixir pattern matching are close analogues.

**Data segment as host-script I/O struct.** The data segment is normally the place where a `loop main` script preserves state across resumes. Repurposing it for one-shot pure functions as an output struct works because `get_data` and `set_data` are already part of the host boundary. The script reads the input through its function argument and writes outputs through `state.field = ...` assignments.

**Negative-index size discovery.** The loader resolves negative indices to `count + n` (Python sequence convention). Calling `fn main(-1)` writes the last entry's fields, including an `id` slot equal to `count - 1`. The host reads the `id` slot to learn the table size with one call, sizes its cache from that, and asserts the value against any parallel host-side constant. This avoids hard-coding the count in the Rust source.

### Minimal example

A table of three colours, each with red, green, blue channels.

```keleusma
// colours.kel
data state {
    id: Word,
    r: Word, g: Word, b: Word,
}

fn main(n: Word) -> Word {
    let count = 3;
    let i = if n < 0 { count + n } else { n };
    fill(i);
    0
}

fn fill(0) -> Word { state.id = 0; state.r = 255; state.g =   0; state.b =   0; 0 }  // red
fn fill(1) -> Word { state.id = 1; state.r =   0; state.g = 255; state.b =   0; 0 }  // green
fn fill(2) -> Word { state.id = 2; state.r =   0; state.g =   0; state.b = 255; 0 }  // blue
fn fill(_n: Word) -> Word { 0 }
```

Host side, with the cache discovered from the script.

```rust
use std::sync::OnceLock;

pub struct Colour { pub r: u8, pub g: u8, pub b: u8 }

static COLOURS: OnceLock<Vec<Colour>> = OnceLock::new();

pub fn colours() -> &'static [Colour] {
    COLOURS.get().expect("colours not loaded")
}

fn load_colours(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    // Discover the count by calling with -1.
    vm.call(&[Value::Int(-1)])?;
    let count = read_int(vm, 0)? as usize + 1;
    let mut table = Vec::with_capacity(count);
    for i in 0..count {
        vm.call(&[Value::Int(i as i64)])?;
        table.push(Colour {
            r: read_int(vm, 1)? as u8,
            g: read_int(vm, 2)? as u8,
            b: read_int(vm, 3)? as u8,
        });
    }
    let _ = COLOURS.set(table);
    Ok(())
}

fn read_int(vm: &Vm, slot: usize) -> Result<i64, Box<dyn std::error::Error>> {
    match vm.get_data(slot)? {
        Value::Int(n) => Ok(*n),
        other => Err(format!("expected Int at slot {}, got {:?}", slot, other).into()),
    }
}
```

### Variations

**Multiple tables in one script.** If two tables share the same data-segment shape, dispatch on a leading `table` argument. `fn main(table, tier)` dispatches to one of two per-table inner functions based on `table`. Each table is independently discoverable via `-1`.

```keleusma
fn main(table: Word, tier: Word) -> Word {
    let count = 20;
    let i = if tier < 0 { count + tier } else { tier };
    if table == 0 { weapon(i); }
    else { if table == 1 { armor(i); } };
    0
}

fn weapon(0) -> Word { ... }
fn armor(0) -> Word { ... }
```

**Chained dispatchers.** When some output fields are derived from others, chain two dispatchers in the loader. The first dispatcher sets the keying field; the second reads it from the data segment and sets the derived fields. The host receives a fully populated entry from a single call.

```keleusma
fn main(n: Word) -> Word {
    let count = 100;
    let i = if n < 0 { count + n } else { n };
    fill(i);
    corpse_fill(state.shape);
    0
}
```

**Names through the return value.** Keleusma's data segment does not currently accept string fields in source. When entries have a name, encode it as a third multi-headed dispatcher returning `Text` and call it as the last expression in `fn main`. The host receives the string as the return value while the data segment carries the numeric fields. The host can leak the returned static string once at startup to obtain a `&'static str`.

### When to use

The pattern fits when all of the following hold.

- The table has more than about ten entries. Below that, the script overhead exceeds the savings.
- Each entry is a small struct of integers or enum ordinals. Strings, floats with quirky precision, or variable-size payloads need workarounds.
- The data benefits from being designer-editable without a host rebuild. If only the Rust author ever touches the table, leave it in Rust.
- Runtime hot reload is desirable, even if the initial implementation caches once. The pattern keeps the path open.

### When not to use

- The data is already dense in Rust (one line per entry with no per-entry struct boilerplate). The migration adds script-loading overhead without compressing the storage.
- The data has lifecycle hooks (constructors, drop). Keleusma cannot carry those. Keep them in Rust.
- The data is keyed on a type that the script cannot represent. Strings, floats with specific precision requirements, or compound keys all push the pattern out of fit.

### Examples in this repository

The rogue example uses this pattern for its bestiary and equipment tables; see [ROGUE.md, *Reading the bestiary script*](./ROGUE.md#reading-the-bestiary-script).

---

## Narrow-runtime type alias

### Problem

The host targets a sub-64-bit native runtime. A 16-bit microcontroller, a retro-class 8-bit machine, a 32-bit embedded core. The default `Vm<'a, 'arena>` is `GenericVm<'a, 'arena, i64, u64, f64>`. Carrying 64-bit values on a 16-bit native target wastes memory and forces software arithmetic on machine operands the hardware does not natively support. The host wants the runtime's word, address, and float widths to match the target.

### Solution

The `Vm` shape is generic over three trait parameters that mirror the bytecode header's `word_bits_log2`, `addr_bits_log2`, and `float_bits_log2` declared widths. Instantiate `GenericVm<W, A, F>` directly with the host's chosen widths and define a type alias for the ergonomic call sites.

```rust
use keleusma::vm::GenericVm;

// 16-bit signed word, 16-bit unsigned address, 32-bit float.
type NarrowVm<'a, 'arena> = GenericVm<'a, 'arena, i16, u16, f32>;

// 8-bit signed word, 16-bit unsigned address, 32-bit float
// (6502-class retro target with floats kept for future opcodes).
type RetroVm<'a, 'arena> = GenericVm<'a, 'arena, i8, u16, f32>;
```

Bytecode for the narrow target is produced through `compile_with_target`. The `embedded_16` preset rejects floating-point opcodes; use a custom `Target` if floats are wanted at a narrower width.

```rust
use keleusma::Arena;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;

let module = {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile_with_target(&program, &Target::embedded_16()).expect("compile")
};

let arena = Arena::with_capacity(4096);
let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("verify");
```

### Host functions speak Rust's natural types

The marshall layer (`KeleusmaType`, `IntoNativeFn`, `IntoFallibleNativeFn`) is parametric over `(W, F)`, with universal impls for `i64`, `f64`, `bool`, `()`, `Option<T>`, fixed arrays, and tuples (arities 2 to 5). The universal `KeleusmaType<W, F> for i64` impl bridges through `Word::to_i64` and `Word::from_i64_wrap`; the universal `KeleusmaType<W, F> for f64` impl bridges through `Float::to_f64` and `Float::from_f64`.

The host author writes `i64` and `f64` in closure signatures regardless of the script's narrower word and float types. The runtime truncates at the boundary.

```rust
vm.register_fn("host::triple", |x: i64| -> i64 { x * 3 });
```

On a `NarrowVm`, the script-side `i16` argument widens to `i64` for the host closure; the `i64` return truncates back to `i16` through `Word::from_i64_wrap`. Hosts that want native-width Rust types (a closure body that takes `i16` directly to avoid widening) can add their own `KeleusmaType<i16, f32> for i16` impl in their crate.

### Standard library bundles work on narrow runtimes

The three `stddsl` bundles implement `Library<W, A, F>` universally and register on any admissible runtime shape. `Math` and `Audio` carry their inner closures in `f64`; on a runtime whose `F` is `f32`, every closure argument and return value passes through `Float::from_f64` and `Float::to_f64` at the marshall boundary, narrowing intermediates and constants. The narrowing is mathematically defined and silent. `Shell` has no floating-point surface and so quantifies over `F` without precision implications.

```rust
let mut vm: NarrowVm<'_, '_> = NarrowVm::new(module, &arena).expect("verify");
vm.register_library(keleusma::stddsl::Math);
vm.register_library(keleusma::stddsl::Audio);
```

Programs that require full `f64` precision should declare a runtime whose `F` is `f64` rather than relying on the silent narrowing. The narrow-float runtime is the appropriate choice when the target's FPU is single-precision and the script does not need the extra mantissa.

### Word-width arithmetic discipline

Script-side arithmetic on a narrow runtime wraps at the runtime's word boundary, not at 64 bits. The `Word` trait's `wrapping_add`, `wrapping_sub`, `wrapping_mul`, `wrapping_div`, `wrapping_rem`, and `wrapping_neg` methods drive every arithmetic dispatch site. On `NarrowVm`, `30_000 + 10_000` produces `-25_536` rather than `40_000`. Programs that depend on wider arithmetic should declare a wider word, or perform the operation host-side through a registered native that takes the natural Rust type.

### Cross-references

- `examples/narrow_runtime.rs` is the worked demonstrator.
- `tests/narrow_vm.rs` is the integration test that pins the pattern.
- [`docs/decisions/BACKLOG.md`, B16](../decisions/BACKLOG.md) records the architectural rationale for the parametric shape.
- The `Word`, `Address`, and `Float` traits live in `src/word.rs`, `src/address.rs`, and `src/float.rs`. Custom impls are admissible; the bundled impls cover the standard widths.

---

## Distributing signed bytecode

### Problem

The host application loads compiled bytecode that arrives from an untrusted source — over a comms link, from disk, from a content-delivery channel — and needs to refuse modules that were not produced by an authorised signer. The threat model is multi-party: one or more known mothership identities are trusted to sign modules; everything else is rejected.

### Solution

Three steps. The cargo feature `signatures` is off by default; turn it on for both the producer and the consumer.

**One. Generate a keypair.** The `keleusma keygen --seed seed.bin --public pub.bin` subcommand writes a 32-byte Ed25519 seed and the matching public key to separate files. On Unix the seed file is mode `0o600`; existing files are not overwritten. Treat the seed as a long-lived secret kept on the signing system. The public key is freely distributable and is what the consumer trusts.

**Two. Declare the requirement and sign.** The source script declares the signing requirement with the `signed` modifier on the entry function:

```keleusma
signed loop main(input: Word) -> Word {
    let next = yield input * 2;
    next
}
```

`keleusma compile script.kel --signing-key seed.bin -o script.kel.bin` produces signed bytecode. The compiler emits `FLAG_REQUIRES_SIGNATURE` in the framing header and the signer appends an Ed25519 signature.

**Three. Verify at the consumer.** The host populates a trust matrix with the public keys it accepts, then loads through the signature-aware entry points:

```rust
let key = ed25519_dalek::VerifyingKey::from_bytes(&public_key_bytes)?;
let mut vm = Vm::load_signed_bytes(&signed_bytes, &arena, &[key])?;
```

For hot-swap delivery (mothership/daughtership pattern), the host constructs the VM from an unsigned baseline, registers the trust matrix, and accepts signed updates over the comm link:

```rust
let mut vm = Vm::new(baseline_module, &arena)?;
vm.register_verifying_key(mothership_key);
// later, after receiving an update over the wire:
vm.replace_module_from_bytes(&update_bytes, initial_data)?;
```

The signature is verified before the new bytecode is decoded; an invalid signature rejects the swap and the current module continues to run.

### Why this works for embedded targets

The verification path uses `ed25519-dalek` under `no_std + alloc`. The `examples/rtos` demonstrator builds with `--features keleusma-signatures` and verifies a built-in signed fixture at boot before entering the scheduler loop. Ed25519 verification on a Cortex-M33 at 600 MHz runs in low milliseconds; the cost is paid at each module load or hot-swap, not at every yield.

### Cross-references

- `R42` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) is the design rationale.
- [`docs/spec/WIRE_FORMAT.md`](../spec/WIRE_FORMAT.md) documents the header extension layout.
- [`docs/guide/EMBEDDING.md`, Signed Modules](./EMBEDDING.md#signed-modules) covers the embedding-side API in more depth.
- [`examples/scripts/11_signed.kel`](../../examples/scripts/11_signed.kel) is a worked signed script.
