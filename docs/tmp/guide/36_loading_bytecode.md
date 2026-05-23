# Chapter 36. Loading Precompiled and Signed Bytecode

> Part IX, Embedding Keleusma in a Rust Program. Chapter 36 of 40.
> Previous: [Chapter 35, Sizing the Arena and Reading the Bounds](./35_arena_sizing.md).
> Next: [Chapter 37, Hot Code Swap from the Host](./37_hot_swap_host.md).

## Goal

By the end of this chapter you will be able to load precompiled bytecode,
load signed bytecode against a trust matrix, and understand the
trust-skip constructor.

## Loading precompiled bytecode

A host with a precompiled `.kel.bin` file, produced by `keleusma compile`
or by a build pipeline, skips the lex, parse, and compile phases:

````rust
let bytes  = std::fs::read("script.kel.bin")?;
let mut vm = Vm::load_bytes(&bytes, &arena)?;
````

`Vm::load_bytes` validates the wire-format framing, runs structural
verification, runs resource-bounds verification, and returns the VM. A
framing failure is `VmError::LoadError`; an analysis failure is
`VmError::VerifyError`. The verification is the same as for a
source-compiled module; loading bytecode does not skip the safety
checks, only the compilation.

## Loading signed bytecode

A module compiled from a `signed` entry function carries a
`FLAG_REQUIRES_SIGNATURE` bit, and `Vm::load_bytes` refuses it, directing
the caller to the signed path. A signed module loads through
`Vm::load_signed_bytes`, which takes a slice of trusted public keys:

````rust
let pub_bytes: [u8; 32] = std::fs::read("pub.bin")?.try_into().unwrap();
let key = ed25519_dalek::VerifyingKey::from_bytes(&pub_bytes)?;
let mut vm = Vm::load_signed_bytes(&signed_bytes, &arena, &[key])?;
````

The slice is the trust matrix. The module loads if its signature verifies
against any key in the slice. An empty slice rejects every signed module.
The matrix is copied onto the constructed VM, so later signed hot-swap
loads inherit the same keys.

A host that boots from an unsigned baseline and accepts only signed
updates afterward registers keys after construction:

````rust
let mut vm = Vm::new(unsigned_baseline_module, &arena)?;
vm.register_verifying_key(operator_key);
````

## The trust-skip constructor

`Vm::new_unchecked` skips the resource-bounds verification. Structural
verification still runs, because the execution loop depends on it for
memory safety.

````rust
let vm = unsafe { Vm::new_unchecked(module, &arena) };
````

It is marked `unsafe` to capture a trust contract: the caller attests
that the bytecode's resource bounds were verified earlier, at build time.
The intended use is a build pipeline that verifies once and ships
bytecode that need not be re-verified on every load. Using it to admit a
program that would fail the safe verifier is intentional misuse outside
the language's guarantees, and it is documented as such. The bounded-time
and bounded-memory guarantees of Part V hold under `Vm::new` and weaken
to host attestation under `Vm::new_unchecked`.

## What you now know

- `Vm::load_bytes` loads a precompiled `.kel.bin`, verifying it the same
  as a source-compiled module.
- `Vm::load_signed_bytes` loads signed bytecode against a slice of
  trusted public keys.
- `register_verifying_key` adds keys to a VM after construction.
- `Vm::new_unchecked` skips the resource-bounds check under an explicit,
  `unsafe` trust contract.

The next chapter replaces a running module with a new one.
