# Chapter 33. Registering Native Functions

> Part IX, Embedding Keleusma in a Rust Program. Chapter 33 of 40.
> Previous: [Chapter 32, Constructing a VM and Running a Module](./32_constructing_a_vm.md).
> Next: [Chapter 34, The Coroutine Protocol from the Host Side](./34_coroutine_protocol.md).

## Goal

By the end of this chapter you will be able to register host functions
that scripts call, by both the ergonomic and the lower-level routes.

## What a native function is

A native function is a Rust function the host registers with the VM under
a name. A script calls it by that name, after a `use` declaration. Native
functions are the bridge: the script decides what should happen, the
native does it. The piano roll's `host::play` is a native; the script
calls it, and the Rust side updates the audio voice state.

Native functions are registered after `Vm::new` and before the script
runs.

## The ergonomic route: register_fn

The recommended route is `register_fn`. It accepts any Rust function or
closure of arity zero through four whose argument and return types
implement the `KeleusmaType` marshalling trait, which the primitive types
already do:

````rust
vm.register_fn("math::add", |a: i64, b: i64| -> i64 { a + b });
vm.register_fn("math::sin", |x: f64| -> f64 { libm::sin(x) });
````

Argument extraction, arity checking, and return-value wrapping are
handled automatically. For a function that may fail, `register_fn_fallible`
accepts a `Result<R, VmError>` return:

````rust
vm.register_fn_fallible("io::read_setting", |key: String| -> Result<String, VmError> {
    fetch(&key).map_err(|e| VmError::NativeError(format!("{}", e)))
});
````

A host struct or enum crosses the boundary by deriving `KeleusmaType`:

````rust
#[derive(KeleusmaType, Debug, Clone)]
struct Point { x: f64, y: f64 }
````

## The lower-level route: register_native_closure

`register_fn` cannot capture host state, because its argument is a plain
function shape. When a native must read or write state the host owns, use
`register_native_closure`. It takes a closure that receives the raw
`&[Value]` arguments and returns `Result<Value, VmError>`:

````rust
let voices = shared_voices.clone();
vm.register_native_closure("host::silence", move |args: &[Value]| {
    let channel = match args[0] {
        Value::Int(n) => n as usize,
        ref other => return Err(VmError::TypeError(
            format!("expected Int, got {:?}", other))),
    };
    voices.lock().unwrap()[channel].gate = false;
    Ok(Value::Unit)
});
````

This is the route the piano roll uses for every one of its natives,
because each one captures the shared `Arc<Mutex<[Voice; 8]>>` voice
table. The closure owns its captured clone, and inspecting the raw
`Value` lets the native validate its arguments explicitly. A plain
function pointer with no captured state can instead use `register_native`.

## Bundled libraries

The `keleusma::stddsl` module ships three bundles of natives, registered
in one call each:

````rust
vm.register_library(stddsl::Math);   // math::sqrt, math::pow, ...
vm.register_library(stddsl::Audio);  // audio::midi_to_freq, ...
vm.register_library(stddsl::Shell);  // shell::getenv, shell::run, shell::sleep_ms,
                                      // shell::read_file, shell::write_file, ...
````

The Shell bundle in V0.2.1 covers environment access (`getenv`,
`has_env`, `setenv`), subprocess execution (`run`, `run_full`,
`run_checked`, `run_timeout`), process termination (`exit`), timing (`sleep_ms`,
`now_unix_ms`), file I/O (`read_file`, `write_file`, `append_file`,
`file_exists`), stderr output (`write_err`, `writeln_err`), and host
metadata (`pid`, `hostname`, `arg_count`, `arg`, `pwd`, `cd`). See
[`STANDARD_LIBRARY.md`](../spec/STANDARD_LIBRARY.md) for the full list.

Each bundle exposes a public `SIGNATURES` constant containing
source-form `use` declarations. Hosts that want compile-time type and
arity validation prepend the constant to the script source before
parsing; the bundled `keleusma-cli` does this for all three bundles. A
custom bundle can implement the `Library` trait and expose a parallel
`SIGNATURES` constant to participate in the same validation flow.

## What you now know

- A native function is a host Rust function a script calls by name.
- `register_fn` and `register_fn_fallible` are the ergonomic route, for
  functions whose types implement `KeleusmaType`.
- `register_native_closure` is the route for natives that capture host
  state, as every piano roll native does.
- `register_library` installs a bundled or custom set of natives.
- A fallible native's failure is handled on the script side with the
  native-error construct `native(args) { ok(v) => ..., error(code) =>
  ... }`. The host reports the `Word` error code by returning an error
  built with the `KeleusmaError` derive, which maps a fieldless enum's
  variants to their discriminants. See [Chapter 23](./23_big_numbers.md).

The next chapter drives a script that yields.
