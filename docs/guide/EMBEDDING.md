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

The script must declare a structurally compatible type for the host's `Point` to flow correctly across the boundary. See [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md) for the admissible interop types.

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

The runtime ships two convenience modules.

- `keleusma::utility_natives::register_utility_natives` registers `to_string`, `length`, `concat`, `slice`, `println`, and a few math helpers.
- `keleusma::audio_natives::register_audio_natives` registers `audio::midi_to_freq`, `audio::freq_to_midi`, `audio::db_to_linear`, `audio::linear_to_db`, and the `math::*` functions enumerated in [STANDARD_LIBRARY.md](../design/STANDARD_LIBRARY.md).

Both register through `register_fn` under the hood. Hosts can register all bundled natives, register a subset, or replace any function with their own implementation.

### WCET and WCMU Attestation

Native function calls participate in WCET and WCMU analysis. By default, native calls are attested as zero-cost in cycles and zero-bytes in heap. Hosts that need a sound bound declare per-native bounds before VM construction or, for already-constructed VMs, before calling `verify_resources`.

````rust
vm.set_native_bounds("math::sin",      cycles_per_call: 25, heap_bytes: 0)?;
vm.set_native_bounds("strings::upper", cycles_per_call: 100, heap_bytes: 256)?;
````

The bounds are the host's promise. The verifier accepts the declared values without independent measurement. The host bears responsibility for accuracy, typically through measurement or bounded-loop analysis on the native function. See [`examples/wcmu_attestation.rs`](../../examples/wcmu_attestation.rs) for a complete walkthrough.

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

## Trust-Skip Construction

Programs whose verification cost is paid at build time, not at every load, may use `Vm::new_unchecked` to skip the resource-bounds check. Structural verification still runs.

````rust
let vm = unsafe { Vm::new_unchecked(module, &arena) };
````

This is intentional misuse if used to admit programs that would fail the safe verifier. The intended use is precompiled bytecode that the host already verified once at build time. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification) for the contract.

## Cross-References

- [`examples/wcmu_basic.rs`](../../examples/wcmu_basic.rs) shows the auto-sizing pattern end to end.
- [`examples/wcmu_attestation.rs`](../../examples/wcmu_attestation.rs) shows native bound declaration.
- [`examples/wcmu_rejection.rs`](../../examples/wcmu_rejection.rs) shows the verifier rejecting an undersized arena.
- [`examples/string_ops.rs`](../../examples/string_ops.rs) shows string concatenation and slicing through utility natives.
- [`examples/yield_error.rs`](../../examples/yield_error.rs) shows error propagation through yield with a script-defined `Result`-shaped enum.
- [`examples/method_call.rs`](../../examples/method_call.rs) shows method dispatch through receiver-style syntax.
- [`examples/piano_roll.rs`](../../examples/piano_roll.rs) is a feature-gated end-to-end SDL3 audio host. It exercises bounded-step execution under a real-time audio deadline, thread-safe handoff between the Keleusma main thread and the SDL3 audio callback, multi-voice control flow through the data segment, and hot code swap between two precompiled songs (`piano_roll.kel` and `piano_roll_2.kel`). Run with `cargo run --release --example piano_roll --features sdl3-example`. Press `s` then Enter to swap; press Enter alone to quit.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the language model.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) describes the runtime model.
- [WHY_REJECTED.md](./WHY_REJECTED.md) describes verifier rejection categories.
