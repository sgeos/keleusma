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
`register_native_closure`. It takes a boxed closure that receives the raw
`&[Value]` arguments and returns `Result<Value, VmError>`:

````rust
let voices = shared_voices.clone();
vm.register_native_closure(
    "host::silence",
    Box::new(move |args: &[Value]| -> Result<Value, VmError> {
        let channel = match args[0] {
            Value::Int(n) => n as usize,
            ref other => return Err(VmError::TypeError(
                format!("expected Int, got {:?}", other))),
        };
        voices.lock().unwrap()[channel].gate = false;
        Ok(Value::Unit)
    }),
);
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
vm.register_library(stddsl::Shell);  // shell::getenv, shell::run, ...
````

A host can register a bundle, register a subset of its own natives, or
implement the `Library` trait on its own type to ship a reusable bundle.

## What you now know

- A native function is a host Rust function a script calls by name.
- `register_fn` and `register_fn_fallible` are the ergonomic route, for
  functions whose types implement `KeleusmaType`.
- `register_native_closure` is the route for natives that capture host
  state, as every piano roll native does.
- `register_library` installs a bundled or custom set of natives.

The next chapter drives a script that yields.
