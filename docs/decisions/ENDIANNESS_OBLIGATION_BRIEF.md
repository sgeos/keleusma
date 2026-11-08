# Brief — the backend checks the host's endianness and the caller picks the target

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-04.**

---

## How this was reached

The word-width sentinel closed one of the backend's **two** module-level guards. Applying the finding
to the class rather than stopping at the instance, the other is `check_target_endianness`.

**It is untested**, and unlike word width **it cannot be tested here**: it is a `cfg!(target_endian)`
on the build host, so its refusal branch is unreachable on every machine this project builds on. That
is a limit on the evidence, not a gap someone forgot to close.

## The finding, which reverses the obvious reading

Its own comment says the check is on the build host and is *"NOT sufficient for cross-compilation to a
big-endian target"*. The first conclusion was that this describes a **latent** gap, since the backend
has no `TargetTriple`, no `create_target_machine`, and no way to cross-compile.

**That was wrong, and checking the public surface is what showed it.** Every public entry point is
lowering-only: they return an LLVM module and **the CALLER supplies the target machine**. Object
emission happens only in tests, and this project already emits for a non-host triple in
`linkage_symbol_census` — `thumbv8m`, which is little-endian, so harmless, but the pattern is in use.

> **So the insufficiency is LIVE FOR CALLERS, not latent.** An embedder can take `lower_module`'s
> output and emit it with a big-endian target machine. The host check passes. The shared slots are
> stored little-endian and an LLVM load on that target byte-swaps them.

**The library cannot detect this, because it never sees the target.** The obligation sits with the
caller, and nothing in the tree says so where a caller would look.

## What this increment is, and is not

**It locates and pins the obligation. It does not discharge it.** Discharging it means either taking a
target parameter or moving the check onto the `TargetMachine`, which the comment already identifies as
a change to the entry points rather than to the function.

The artifact is a **ratchet**, in the shape the `v0.2.3` line used for `Op::Len`: pin the precondition
that makes the present arrangement defensible, so that when it stops holding the test fails and names
the work.

## Prior failures this is exposed to

**A guard that agrees with itself.** The ratchet must be shown to fail under a change, or it is
decoration.

**Calling a ratchet a fix.** The `Op::Len` precedent is explicit that pinning a hazard is not
repairing it.

**Reading an absence as safety.** "No cross-compilation in the library" was true and did not mean what
it appeared to mean, because the capability lives with the caller.

## The wrong turns

**1. Do not add a target parameter as part of this.** That is the larger change and it belongs to
whoever needs cross-emission.

**2. Do not assert the refusal branch works.** It cannot run here. Claiming otherwise would be a test
that measures nothing.

**3. Do not describe the hazard as hypothetical.** A caller emitting for a big-endian target gets
byte-swapped slots today, and the reason none has is that no such target is on the committed list.
