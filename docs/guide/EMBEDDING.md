# Embedding

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document describes the host-facing embedding surface of Keleusma. It covers VM construction, native function registration, arena sizing, the call and resume protocol for coroutine scripts, and error recovery. The reference for this surface is `src/vm.rs`. Worked examples live in [`examples/`](../../examples).

## VM Lifecycle

A Keleusma VM is a single-threaded coroutine driver. The host owns the bytecode and the arena. The VM borrows the arena for the lifetime of its existence.

The minimal lifecycle is the following.

````rust
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};
use keleusma::Arena;

let tokens  = tokenize(SOURCE)?;
let program = parse(&tokens)?;
let module  = compile(&program)?;

let arena   = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
let mut vm  = Vm::new(module, &arena)?;
````

The four phases produce four distinct value types. `tokenize` produces a `Vec<Token>`. `parse` produces a `Program` syntax tree. `compile` produces a `Module` bytecode object. `Vm::new` consumes the module, borrows the arena, runs structural verification and resource-bounds verification, and returns a ready-to-call VM.

The VM and the module share the arena's lifetime. The host must keep the arena alive at least as long as the VM. This is enforced by the borrow checker through the `'arena` lifetime parameter on `Vm`.

### Loading Precompiled Bytecode

A host that has a precompiled `.kel.bin` file skips the lex, parse, and compile steps.

````rust
let bytes = std::fs::read("script.kel.bin")?;
let mut vm = Vm::load_bytes(&bytes, &arena)?;
````

The wire format is self-describing. The header carries magic, length, version, and target word, address, and float widths. `Vm::load_bytes` validates the framing, runs structural verification, runs resource-bounds verification, and returns the VM. Validation failure is returned as `VmError::LoadError` for framing failures or `VmError::VerifyError` for analysis failures.

## Calling the Script

The VM exposes two entry points: `Vm::call(args)` to start execution and `Vm::resume(value)` to continue after a yield. Both return `Result<VmState, VmError>`.

````rust
pub enum VmState {
    Finished(Value),
    Yielded(Value),
    Reset,
}
````

The three states correspond to the three function categories.

- **`fn` (atomic total)**. The script terminates and returns a value. `call` returns `VmState::Finished(value)`.
- **`yield` (non-atomic total)**. The script yields a value to the host. `call` returns `VmState::Yielded(value)`. The host calls `resume(value)` with a host-provided input. The script either yields again or finishes.
- **`loop` (productive divergent)**. The script yields on every iteration and resets at the end of the body. `call` returns `VmState::Yielded(value)`. The host calls `resume(value)` to drive the next yield. After the body completes, the next call returns `VmState::Reset`. Hot code swap is admissible at the reset boundary.

A typical yield-driven loop looks like the following.

````rust
let mut state = vm.call(&[Value::Int(seed)])?;
loop {
    match state {
        VmState::Yielded(out) => {
            let reply = compute_host_response(&out);
            state = vm.resume(reply)?;
        }
        VmState::Reset => {
            state = vm.resume(Value::Int(next_seed))?;
        }
        VmState::Finished(value) => {
            handle_result(value);
            break;
        }
    }
}
````

## Native Functions

Native functions are Rust functions registered with the VM that scripts may call by name. The host declares the function name, the function pointer or closure, and (optionally) the WCET and WCMU bounds.

### Ergonomic Typed Registration

The recommended path uses the marshalling layer. Any Rust function or closure of arity zero through four whose argument and return types implement `KeleusmaType` registers through `register_fn`.

````rust
vm.register_fn("math::add",      |a: i64, b: i64| -> i64 { a + b });
vm.register_fn("math::sin",      |x: f64| -> f64 { libm::sin(x) });
vm.register_fn("strings::upper", |s: String| -> String { s.to_uppercase() });
````

For functions that may fail, `register_fn_fallible` accepts `Result<R, VmError>`.

````rust
vm.register_fn_fallible("io::read_setting", |key: String| -> Result<String, VmError> {
    fetch(&key).map_err(|e| VmError::NativeError(format!("{}", e)))
});
````

The argument extraction, arity checking, and return-value wrapping happen automatically. Type mismatches at the boundary surface as `VmError::TypeError` at runtime.

### Custom Types via the Derive Macro

Host structs and enums become marshallable through the `KeleusmaType` derive.

````rust
use keleusma::KeleusmaType;

#[derive(KeleusmaType, Debug, Clone)]
struct Point {
    x: f64,
    y: f64,
}

vm.register_fn("geom::midpoint", |a: Point, b: Point| -> Point {
    Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    }
});
````

The script must declare a structurally compatible type for the host's `Point` to flow correctly across the boundary. See [TYPE_SYSTEM.md](../spec/TYPE_SYSTEM.md) for the admissible interop types.

### Lower-Level Registration

When the function must inspect the raw `Value` enum, register a function pointer that accepts `&[Value]` and returns `Result<Value, VmError>` directly.

````rust
fn first_argument(args: &[Value]) -> Result<Value, VmError> {
    args.first()
        .cloned()
        .ok_or_else(|| VmError::NativeError(String::from("missing arg")))
}
vm.register_native("debug::first_argument", first_argument);
````

A boxed closure variant `register_native_closure` captures host state. A context-aware variant `register_native_with_ctx` receives a `NativeCtx<'a>` carrying a borrow of the arena, used by natives that allocate dynamic strings into arena memory.

### Bundled Natives

V0.2.0 retired the script-side text-composition machinery (the `to_string`, `concat`, `slice`, `length` utility natives and the f-string interpolation surface). The runtime ships a small bundled set:

- `keleusma::utility_natives::register_utility_natives` registers `println` (a debug primitive that is a no-op on `no_std` targets; hosts that want output override with a `register_native_closure`).
- `keleusma::audio_natives::register_audio_natives` registers `audio::midi_to_freq`, `audio::freq_to_midi`, `audio::db_to_linear`, `audio::linear_to_db`, and the `math::*` functions enumerated in [STANDARD_LIBRARY.md](../spec/STANDARD_LIBRARY.md).
- `keleusma::stddsl::Math`, `Audio`, and `Shell` register through `Vm::register_library` (see the "Standard DSL Libraries" section below).

All register through `register_fn` or `register_native` under the hood. Hosts can register all bundled natives, register a subset, or replace any function with their own implementation.

### Host-Defined String Helpers

The language is not the right vehicle for heavy string manipulation, and V0.2.0 does not ship a string standard library. **Where an application needs string work in context, register native Rust functions and let the script consume them through `use` declarations.** Rust's standard library handles formatting, splitting, regex, Unicode operations, and encoding conversion far better than anything reasonable to build inside the script.

````rust
vm.register_fn("text::upper", |s: String| -> String { s.to_uppercase() });
vm.register_fn("text::trim",  |s: String| -> String { s.trim().to_string() });
vm.register_fn_fallible(
    "text::split_first_word",
    |s: String| -> Result<String, VmError> {
        s.split_whitespace()
            .next()
            .map(|w| w.to_string())
            .ok_or_else(|| VmError::NativeError("empty input".into()))
    },
);
````

Script side:

````
use text::upper
use text::trim
use text::split_first_word

fn greet(name: Text) -> Text {
    text::upper(text::trim(name))
}
````

See [FAQ.md](./FAQ.md) for the broader framing on strings.

### Standard DSL Libraries

The `keleusma::stddsl` module ships four bundled libraries that hosts register through a single call. Each bundle is a unit struct implementing the `Library` trait. The trait's `register` method installs the bundle's native functions on the VM.

````rust
use keleusma::stddsl;

let mut vm = Vm::new(module, &arena)?;
vm.register_library(stddsl::Math);   // math::sqrt, math::floor, ...
vm.register_library(stddsl::Audio);  // audio::midi_to_freq, ...
vm.register_library(stddsl::Shell);  // shell::getenv, shell::run, shell::exit
````

`stddsl::Math` and `stddsl::Audio` require the `floats` cargo feature. `stddsl::Shell` requires the `shell` feature, which adds a `std` dependency and is therefore incompatible with `no_std` builds. The `keleusma-cli` crate enables both features and registers all three bundles by default. Hosts that want bundled text composition register a host-side `format` / `to_string` / `concat` native through `register_verified_native` (see the "Host-Defined String Helpers" section above) or implement their own `Library` bundle.

Hosts that want to ship their own reusable bundles implement the `Library` trait on a host-side type. The trait is the extensibility surface; the bundled libraries are an example of the pattern, not a closed set.

````rust
use keleusma::stddsl::Library;
use keleusma::vm::Vm;

pub struct MyDsl;

impl Library for MyDsl {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        vm.register_fn("mydsl::greet", |name: i64| -> i64 { name + 1 });
        // ... register more natives ...
    }
}

// Use site:
vm.register_library(MyDsl);
````

#### Single-file scripts

Keleusma scripts are necessarily single-file. There is no `import` or `mod` mechanism inside the language; cross-script reuse is intentionally outside the V0.2 surface. If your application's needs grow to where you find yourself wishing for modularisation, the recommended path is to roll a custom DSL library: implement `Library` on a host-side unit struct that registers the natives your scripts call, and let every script consume the same vocabulary through `use` declarations. The host-side library is the unit of reuse, not the script.

### Opaque Host Types

Hosts that need to expose Rust values to scripts without revealing their internal structure use the `HostOpaque` trait introduced in V0.2.0. The host implements the trait for its concrete Rust type; the script declares the type by name in function signatures, and the type checker resolves the name as `Type::Opaque`. Native functions produce opaque values through `host_arc` and consume them by extracting a typed reference through `dyn HostOpaque::downcast_ref`.

````rust
use keleusma::{host_arc, HostOpaque, Value};

// Newtype required to avoid violating Rust's orphan rule when
// `impl`-ing a foreign trait on a foreign type.
struct RustString(String);

impl HostOpaque for RustString {
    fn type_name(&self) -> &'static str { "RustString" }
}

vm.register_native("make_string", |args| {
    // Construct an opaque from a Rust value.
    Ok(Value::Opaque(host_arc(RustString(String::from("hello")))))
});

vm.register_native("upper_case", |args| {
    // Consume an opaque, return a new opaque.
    let opaque = match &args[0] {
        Value::Opaque(o) => o.clone(),
        other => return Err(VmError::TypeError(format!(
            "expected RustString, got {}", other.type_name()))),
    };
    let s = opaque.as_ref().downcast_ref::<RustString>().ok_or_else(|| {
        VmError::TypeError(format!(
            "expected RustString, got opaque {}", opaque.type_name()))
    })?;
    Ok(Value::Opaque(host_arc(RustString(s.0.to_uppercase()))))
});
````

Script side:

````
use make_string
use upper_case

fn main() -> RustString {
    let s = make_string();
    upper_case(s)
}
````

The opaque value is host-managed through `Arc`, so it has a lifetime independent of the arena. It may cross the yield boundary in the dialogue type and persists across arena resets. Pointer identity is the equality semantics: two opaque values compare equal only if they share the same `Arc` allocation.

Opaque values contribute zero to the script-side WCMU bound because the allocation is host-managed. For heavy work whose memory footprint matters, attach a per-native attestation through `Vm::set_native_bounds` so the verifier sees a bounded host contribution.

See [`examples/opaque_rust_string.rs`](../../examples/opaque_rust_string.rs) for a complete walkthrough that exposes `std::string::String` to scripts.

### WCET and WCMU Attestation

Native function calls participate in WCET and WCMU analysis. By default, native calls are attested as zero-cost in cycles and zero-bytes in heap. Hosts that need a sound bound declare per-native bounds before VM construction or, for already-constructed VMs, before calling `verify_resources`.

````rust
vm.set_native_bounds("math::sin",      cycles_per_call: 25, heap_bytes: 0)?;
vm.set_native_bounds("strings::upper", cycles_per_call: 100, heap_bytes: 256)?;
````

The bounds are the host's promise. The verifier accepts the declared values without independent measurement. The host bears responsibility for accuracy, typically through measurement or bounded-loop analysis on the native function. See [`examples/wcmu_attestation.rs`](../../examples/wcmu_attestation.rs) for a complete walkthrough.

### Calibrated WCET in CPU cycles

The bundled `NOMINAL_COST_MODEL` returns per-opcode pipelined-cycle estimates suitable for relative ordering on a single platform. The values are not measured for any specific host CPU; they assign 1 to data movement, 2 to arithmetic, 3 to division, 5 to composite construction, 10 to function calls. Hosts that want WCET in actual CPU cycles for the deployment target consume a `MEASURED_COST_MODEL` generated by the `keleusma-bench` workspace member.

The wiring is `include!` of a measured-model fragment from [`keleusma-bench/measured_cost_models/`](../../keleusma-bench/measured_cost_models/), then a call to the `_with_cost_model` variant of the WCET API:

```rust
include!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs"));

use keleusma::verify::wcet_stream_iteration_with_cost_model;

let cycles = wcet_stream_iteration_with_cost_model(chunk, &MEASURED_COST_MODEL)?;
```

The cookbook section [Calibrated WCET with a measured cost model](./COOKBOOK.md#calibrated-wcet-with-a-measured-cost-model) is the recipe walkthrough. [`examples/measured_wcet.rs`](../../examples/measured_wcet.rs) is the minimal working example. [`keleusma-bench/measured_cost_models/README.md`](../../keleusma-bench/measured_cost_models/README.md) catalogues the pre-generated fragments and the capture workflow for new targets.

## Arena Sizing

The arena holds the operand stack at the bottom and dynamic strings on the top. Total bytes used during a Stream-to-Reset iteration is bounded by the WCMU analysis. The host has three options.

**Option A. Use the default capacity.** `DEFAULT_ARENA_CAPACITY` is sixty-four kilobytes. Sufficient for most scripts.

````rust
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
````

**Option B. Compute the capacity from the module before VM construction.** The function `auto_arena_capacity_for` walks the module and returns the bound.

````rust
let cap   = keleusma::vm::auto_arena_capacity_for(&module, &[])?;
let arena = Arena::with_capacity(cap);
let vm    = Vm::new(module, &arena)?;
````

The empty slice argument represents per-native heap attestations. Pass attested values when the script calls heap-allocating natives. See [`examples/wcmu_basic.rs`](../../examples/wcmu_basic.rs) for the auto-sizing pattern.

**Option C. Provide a static buffer.** The arena can run from a host-owned buffer in `.bss` for embedded targets without a heap.

````rust
static mut ARENA_BUFFER: [u8; 16 * 1024] = [0; 16 * 1024];
let arena = unsafe {
    Arena::from_static_buffer(core::ptr::addr_of_mut!(ARENA_BUFFER))
};
````

If the chosen capacity is below the analyzed WCMU, `Vm::new` returns `VmError::VerifyError`. The error is surfaceable before any code runs.

## Error Recovery

Errors during `call` or `resume` return `Err(VmError)`. The VM is not automatically reset; volatile state may remain on the operand stack and in the arena. Two paths exist.

**Path 1. Discard the VM.** Drop and reconstruct. The arena resets when the new VM is constructed against it.

**Path 2. Recover and continue.** Call `Vm::reset_after_error` to clear volatile state while preserving the data segment.

````rust
match vm.call(&[arg]) {
    Ok(state)              => handle_state(state),
    Err(VmError::TypeError(msg)) => {
        eprintln!("script error: {}", msg);
        vm.reset_after_error();
    }
    Err(other) => return Err(other.into()),
}
````

The data segment, if declared, persists across error events. Long-running streams that accumulate state should rely on the data segment rather than local bindings for state that must survive errors.

### Error Variants

`VmError` enumerates the runtime error conditions.

| Variant | Condition |
|---------|-----------|
| `StackUnderflow` | Empty operand stack on pop |
| `TypeError(msg)` | Operand type does not match the operation |
| `DivisionByZero` | Integer or modulo by zero |
| `IndexOutOfBounds(idx, len)` | Array or tuple index out of range |
| `FieldNotFound(struct, field)` | Field access on a struct that does not declare the field |
| `NoMatch(value)` | No match arm or multiheaded function head matched |
| `NativeError(msg)` | Native function returned `Err` |
| `InvalidBytecode(msg)` | Bytecode shape unexpected at runtime |
| `Trap(msg)` | Script halted by a `Trap` instruction |
| `VerifyError(msg)` | Structural or resource-bounds verification failed at construction |
| `LoadError(msg)` | Wire-format framing failed during `load_bytes` |

`VerifyError` is the only variant that fires before any script code executes. The other variants fire during execution. See [WHY_REJECTED.md](./WHY_REJECTED.md) for `VerifyError` interpretation.

## Hot Code Swapping

The VM supports replacing the loaded module at the reset boundary of a `loop` script. The host calls `Vm::replace_module` after observing `VmState::Reset` and starts the new module's entry point with `Vm::call`. The signature takes the new module and an initial data-segment vector whose length must match the new module's declared schema.

````rust
match vm.resume(input)? {
    VmState::Reset => {
        let new_module = recompile_or_load_new_version()?;
        // Re-initialise the data segment. Length must match the
        // new module's declared `data` block size; preserve or
        // migrate values as appropriate.
        let initial_data = vec![Value::Int(0); new_module_data_slot_count];
        vm.replace_module(new_module, initial_data)?;
        // The swap clears coroutine state. Drive the new module
        // from the entry point, not via `resume`.
        vm.call(&[Value::Int(next_seed)])?;
    }
    other => { /* ... */ }
}
````

The dialogue type, the yielded type and the resume type, must remain stable across swaps. The data segment may carry forward (pass current values), may be re-initialized to the new schema, or may be replaced by host migration code. Native function registrations live on the VM, not on the module, and persist across swaps. See [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) for the full hot-swap specification, and [`examples/piano_roll.rs`](../../examples/piano_roll.rs) for a runnable end-to-end demonstration.

## Signed Modules

The optional `signatures` cargo feature enables Ed25519 signing of compiled bytecode. Source scripts declare the requirement with the `signed` modifier on the entry function (`signed fn main`, `signed yield main`, `signed loop main`); the compiler emits `FLAG_REQUIRES_SIGNATURE` in the framing header. The runtime refuses to load such a module unless its signature verifies against a trust matrix the host populates before the load.

### Signing at build time

The host (or a build pipeline) takes a 32-byte Ed25519 seed and uses `wire_format::module_to_signed_wire_bytes` to produce signed bytes. The CLI exposes this through `keleusma compile --signing-key seed.bin -o out.bin`. The `keleusma keygen --seed seed.bin --public pub.bin` subcommand generates a fresh keypair from the OS RNG; the seed file is written with `0o600` permissions on Unix and existing files are not overwritten.

````rust
let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed_bytes);
let signed = keleusma::wire_format::module_to_signed_wire_bytes(&module, &signing_key)?;
std::fs::write("script.kel.bin", &signed)?;
````

### Verifying and loading

The host loads a signed module through `Vm::load_signed_bytes(bytes, arena, &keys)`. The keys slice carries one or more public keys; the first matching key admits the module. An empty slice rejects every signed module with `LoadError::InvalidSignature`. The matrix is also copied onto the constructed VM so subsequent `Vm::replace_module_from_bytes` calls inherit the same keys.

````rust
let pub_bytes: [u8; 32] = std::fs::read("pub.bin")?.try_into().unwrap();
let key = ed25519_dalek::VerifyingKey::from_bytes(&pub_bytes)?;
let mut vm = Vm::load_signed_bytes(&signed, &arena, &[key])?;
````

Hosts that bootstrap from an unsigned baseline and only accept signed bytecode at hot-swap construct the VM normally, register keys post-construction, and hot-swap signed updates:

````rust
let mut vm = Vm::new(unsigned_baseline_module, &arena)?;
vm.register_verifying_key(mothership_key);
// ... later, after receiving a signed update over the comm link ...
vm.replace_module_from_bytes(&update_bytes, initial_data)?;
````

`Vm::load_bytes` refuses signed modules with a diagnostic redirecting the caller to `Vm::load_signed_bytes`. Without the `signatures` feature, the variant returned is `LoadError::SignaturesUnsupported` so the operator sees that the build cannot verify, not just that the path is wrong.

The signing message convention is the full framed buffer with the signature payload bytes and the CRC trailer bytes zeroed. The verifier reconstructs the same view by zeroing both regions on its private copy before the cryptographic operation. The CRC trailer covers the full file including the real signature, so framing-level tamper is caught by the CRC alone; signature mutation is caught by the cryptographic check after CRC repair.

See `R42` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the design rationale and [`docs/spec/WIRE_FORMAT.md`](../spec/WIRE_FORMAT.md) for the header layout.

## Trust-Skip Construction

Programs whose verification cost is paid at build time, not at every load, may use `Vm::new_unchecked` to skip the resource-bounds check. Structural verification still runs.

````rust
let vm = unsafe { Vm::new_unchecked(module, &arena) };
````

This is intentional misuse if used to admit programs that would fail the safe verifier. The intended use is precompiled bytecode that the host already verified once at build time. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification) for the contract. `Vm::new_unchecked` also skips the signed-module flag check; the caller attests that any signature verification was performed at build time.

## Cross-References

- [`examples/wcmu_basic.rs`](../../examples/wcmu_basic.rs) shows the auto-sizing pattern end to end.
- [`examples/wcmu_attestation.rs`](../../examples/wcmu_attestation.rs) shows native bound declaration.
- [`examples/wcmu_rejection.rs`](../../examples/wcmu_rejection.rs) shows the verifier rejecting an undersized arena.
- [`examples/string_ops.rs`](../../examples/string_ops.rs) shows host-registered text concatenation and slicing natives.
- [`examples/yield_error.rs`](../../examples/yield_error.rs) shows error propagation through yield with a script-defined `Result`-shaped enum.
- [`examples/method_call.rs`](../../examples/method_call.rs) shows method dispatch through receiver-style syntax.
- [`examples/piano_roll.rs`](../../examples/piano_roll.rs) is a feature-gated end-to-end SDL3 audio host. It exercises bounded-step execution under a real-time audio deadline, thread-safe handoff between the Keleusma main thread and the SDL3 audio callback, multi-voice control flow through the data segment, and hot code swap across a roster of precompiled songs (`piano_roll_<N>.kel`, currently `piano_roll_0.kel`, `piano_roll_1.kel`, and `piano_roll_2.kel`). Run with `cargo run --release --example piano_roll --features sdl3-example`. Press `s` to cycle to the next song, `r` to restart the current song, a digit to select a song by index, or Enter alone to quit. The long-form manual is [PIANO_ROLL.md](./PIANO_ROLL.md), which covers writing songs, lifting the host loop into another application, and using the example as an architectural reference for embedding Keleusma in other control-loop domains.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the language model.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) describes the runtime model.
- [WHY_REJECTED.md](./WHY_REJECTED.md) describes verifier rejection categories.
