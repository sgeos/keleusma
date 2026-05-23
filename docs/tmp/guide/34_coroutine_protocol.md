# Chapter 34. The Coroutine Protocol from the Host Side

> Part IX, Embedding Keleusma in a Rust Program. Chapter 34 of 40.
> Previous: [Chapter 33, Registering Native Functions](./33_registering_natives.md).
> Next: [Chapter 35, Sizing the Arena and Reading the Bounds](./35_arena_sizing.md).

## Goal

By the end of this chapter you will be able to drive a `yield` or `loop`
script from the host, and recover from a runtime error.

## call, resume, and VmState

The host has two entry points into a VM. `call(&[Value])` starts
execution. `resume(Value)` continues it after a yield. Both return
`Result<VmState, VmError>`, where `VmState` has three variants:

````rust
pub enum VmState {
    Finished(Value),
    Yielded(Value),
    Reset,
}
````

The variants correspond to the three function categories of Chapter 15.

- An atomic `fn` script runs to completion: `call` returns
  `Finished(value)`.
- A `yield` script hands a value out: `call` returns `Yielded(value)`,
  and the host calls `resume(input)` to continue.
- A `loop` script yields every cycle and resets at the end of its body.
  `call` returns `Yielded`, `resume` drives the next yield, and once a
  body completes the next call returns `Reset`.

## The drive loop

A host drives a yielding script with a match on `VmState`:

````rust
let mut state = vm.call(&[Value::Int(seed)])?;
loop {
    match state {
        VmState::Yielded(out) => {
            let reply = host_response(&out);
            state = vm.resume(reply)?;
        }
        VmState::Reset => {
            state = vm.resume(Value::Int(next_input))?;
        }
        VmState::Finished(value) => {
            handle_result(value);
            break;
        }
    }
}
````

The piano roll's tick loop is this pattern. Once per sixteenth-note tick
it calls `resume` with the current tick number, the script runs one
cycle and yields, and the host sleeps until the next tick boundary. When
the script's body completes a cycle, the state is `Reset`, which is the
boundary where a hot swap may happen, the subject of Chapter 37.

## The dialogue type

The value passed to `resume` and the value carried by `Yielded` are the
two halves of the script's dialogue, introduced in Chapter 16. The host
and the script must agree on these two types. The agreement is not
checked by the Rust compiler, because both are carried as the runtime
`Value` enum; it is the host author's responsibility to supply resume
values of the type the script expects.

## Error recovery

A runtime error from `call` or `resume` returns `Err(VmError)`. The VM is
left in an intermediate state. The host has two choices.

- Discard the VM and reconstruct it. Constructing a new VM against the
  arena resets the arena.
- Call `vm.reset_after_error()`, which clears the operand stack, the call
  frames, and the arena, while preserving the data segment.

````rust
match vm.call(&[arg]) {
    Ok(state) => handle_state(state),
    Err(VmError::TypeError(msg)) => {
        eprintln!("script error: {}", msg);
        vm.reset_after_error();
    }
    Err(other) => return Err(other.into()),
}
````

`VmError` enumerates the runtime conditions: `StackUnderflow`,
`TypeError`, `DivisionByZero`, `IndexOutOfBounds`, `FieldNotFound`,
`NoMatch`, `NativeError`, `InvalidBytecode`, `Trap`, `VerifyError`, and
`LoadError`. Of these, `VerifyError` and `LoadError` fire at construction,
before any script code runs; the rest fire during execution.

## What you now know

- `call` starts a VM, `resume` continues it after a yield.
- `VmState` is `Finished`, `Yielded`, or `Reset`, matching the three
  function categories.
- A host drives a yielding script with a match-and-resume loop.
- The dialogue types are the host's responsibility to honor.
- `reset_after_error` recovers a VM while preserving the data segment.

The next chapter sizes the arena.
