# Chapter 37. Hot Code Swap from the Host

## Goal

By the end of this chapter you will be able to replace a running module
with a new one, from the host, at a reset boundary.

## The swap point

Hot code swap is host-driven. A `loop` script does not replace itself.
The host replaces it, and only at one point: a `VmState::Reset`, the
boundary between two iterations of the loop body. At that boundary the
script's operand stack is empty, and the private data segment is the only
live script-owned state, which is what makes the swap safe. Shared data is
the host-owned buffer of Chapter 34, not script-owned state, so it is not
part of the swap.

The host watches for `Reset` in its drive loop and calls
`Vm::replace_module`:

```rust
match vm.resume(input)? {
    VmState::Reset => {
        let new_module = load_new_version()?;
        let slot_count = new_module_private_slot_count;
        let initial_data = vec![Value::Int(0); slot_count];
        vm.replace_module(new_module, initial_data)?;
        vm.call(&[Value::Int(next_input)])?;
    }
    other => { /* ... */ }
}
```

`replace_module` takes the new module and an initial private-data vector
whose length must match the new module's declared `private data` slot
count; pass an empty vector for a module with no private data. Shared data
is not a hot-swap input; it stays in the host's own buffer across the swap.
After the swap, the VM's coroutine state is cleared, so the new module is
driven from its entry point with `call`, not with `resume`.

## What survives a swap

Three things must be understood about what crosses a swap.

- The dialogue type must stay stable. The new module must yield and
  resume the same types as the old one, because the host keeps driving
  the conversation without a break.
- The private data segment is handed in fresh. The host may pass the old
  values forward, re-initialize them to zero, or run migration code to fit
  a new schema. The piano roll passes a freshly zeroed vector, so each
  incoming song's init block runs against a clean slate.
- Native function registrations live on the VM, not on the module, so
  they persist across the swap. The new module sees the same natives the
  old one did.

## Signed updates

When a swap installs a module delivered as signed bytecode, the host uses
`Vm::replace_module_from_bytes`. It verifies the signature against the
trust matrix the VM carries, the matrix registered in Chapter 36, before
installing the module. A signed hot swap is the mechanism behind the
multi-party delivery scenario: a baseline device receives a signed update
over a link and installs it only if the signature checks out.

## The piano roll's swap

The piano roll is the worked example. Its stdin thread turns a keypress
into a swap request. The main loop, on the next `VmState::Reset`, calls
`replace_module` with the next song's module and an empty private-data
vector, resizes and re-zeroes the shared-data buffer for the new song,
resets its host-owned voice state, and calls the new module's entry point.
The audible song change is exactly this code path.

## What you now know

- Hot swap is host-driven and happens only at a `VmState::Reset`.
- `replace_module` installs a new module and a fresh private data segment;
  the new module is then driven from `call`.
- The dialogue type must stay stable; native registrations persist; the
  private data segment is the host's to carry, reset, or migrate, while
  shared data stays in the host's buffer across the swap.
- `replace_module_from_bytes` installs a signed update against the trust
  matrix.

The next chapter measures execution cost.
