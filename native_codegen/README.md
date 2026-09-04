# keleusma-native

> **Navigation**: [Documentation Root](../docs/README.md) | [V0.3.x Roadmap](../docs/roadmap/V0_3_X_ROADMAP.md)

LLVM native code generation for verified Keleusma bytecode. V0.3.x Workstream A.

**Status, measured 2026-09-03: 63 of the instruction set's 66 opcodes lower**, over the 74-module
corpus, reported by `isa_lowering_census`. The differential oracle was in place before the subset
widened, so every extension has been checked against the virtual machine from the start.

> ⚠ **THE PARAGRAPH HERE PREVIOUSLY SAID "28 of 66" AND NAMED THE DATA SEGMENT AND COMPOSITES AS THE
> NEXT INCREMENTS. BOTH WERE LONG DONE.** It understated the backend by more than half and pointed a
> reader at finished work. **A hand-maintained opcode list is what drifted**, so this file no longer
> keeps one — run the census, which computes it.

**None of the three remaining opcodes is missing support**, and reading `63 of 66` as three opcodes to
implement is the specific error the census now prints a disposition table to prevent:

| opcode | disposition |
|---|---|
| `Reset` | **accepted**, by a route the census does not instrument — consumed by the degenerate-stream shape match |
| `IsStruct` | **no verdict available**; no corpus witness and no producer found by a bounded search |
| `Len` | **refusing is correct.** The machine returns `InvalidBytecode` for it on a flat array, so lowering it would compute a length where the reference traps |

**A LOWERS verdict is not a correctness claim.** It says the backend emitted code, not that the code is
right; that is the differential's question. Sensitivity — whether a defect in a lowering would be
*detected* — is measured separately by `tools/mutation_sweep.py`, and
[`docs/decisions/NATIVE_MUTATION_CENSUS.md`](../docs/decisions/NATIVE_MUTATION_CENSUS.md) is **stale
and expensive to un-stale**: a round-one re-run was abandoned after 12h51m on the 5th of 25 mutations.

Scoping notes are in
[`docs/decisions/NATIVE_LOWERING_INVENTORY.md`](../docs/decisions/NATIVE_LOWERING_INVENTORY.md).

## What it does today

Takes a `Module` from the Rust reference compiler, lowers a chunk to LLVM IR,
and produces either a JIT-executed function or a native object file. Correctness
is established by executing the same bytecode on the VM and requiring identical
results.

**Supported opcodes are not listed here.** The list is computed, and a copy kept by hand is what went
stale before:

    cd native_codegen && cargo test --test isa_lowering_census -- --nocapture

Anything unsupported is **refused** with `LowerError::UnsupportedOp` rather than lowered to something
plausible. Straight-line arithmetic, structured conditionals, counted loops, the data segment,
composites, native calls, and `f32`/`f64` floats all lower.

Only 64-bit word width (`word_bits_log2 == 6`) is accepted.

## Why this package is detached

It is a standalone package with its own `[workspace]`, like `compiler/` and
`examples/rtos/`. For those, detachment keeps a half-built subproject from
destabilising the released workspace. Here it does that **and** one more thing:
this package needs an LLVM development install, and as a workspace member it
would make LLVM a hard build dependency of the entire repository, for every
developer and for CI.

The parent's `cargo test --workspace` therefore does not build this, and
`scripts/release-gate.sh` runs it as a separate step that **skips loudly** when
LLVM is absent.

## Requirements

- LLVM **22.1** development install, with headers and libraries.
- The binding is `inkwell` 0.9 over `llvm-sys` 221.

`inkwell` 0.8 does **not** support LLVM 22; its maximum is `llvm20-1`. Version
0.9 is required, not merely preferred.

### macOS with MacPorts

```
sudo port install llvm-22
```

`.cargo/config.toml` in this directory points `LLVM_SYS_221_PREFIX` at
`/opt/local/libexec/llvm-22` and adds `/opt/local/lib` to the link path. The
second is not optional: MacPorts' LLVM links against `zstd`, `xml2` and `ffi`
from there, and without it the build succeeds and the **link** fails with
`library 'zstd' not found`. That failure surfaces at `ld` rather than at the
binding, which is the wrong layer to start debugging at.

### Other platforms

Set `LLVM_SYS_221_PREFIX` in your environment. It takes precedence over the
value in `.cargo/config.toml`, which is declared with `force = false` precisely
so that no edit to a tracked file is needed. The MacPorts link path is harmless
elsewhere, since a search path that does not exist is ignored.

## Running

```
cd native_codegen
cargo test
```

## The two things to know before changing the lowering

**The differential oracle is not a formality.** The first version of this
lowering had a real defect that one of the two test inputs passed straight
through. When you add an opcode, add inputs that distinguish its paths, and
check that each new case can actually fail.

**Bare arithmetic wraps, and that is deliberate.** `a + b` compiles to
`CheckedAdd; PopN(2)`, discarding the outcome flag and the high word so the low
word survives. That is wrapping addition and it is total. `OverflowPolicy::Trap`
exists for Workstream F, and it **diverges from the VM**: with it enabled,
`add(i64::MAX, 1)` aborts where the VM returns a value. It is not the default
for that reason.

## Host contract: a native must not unwind

Every function this backend defines is emitted with LLVM's `nounwind` attribute. **Nothing generated
here can unwind** — Keleusma has no exceptions and a fault traps.

**That assertion covers the natives a chunk calls.** If a host native unwinds through a Keleusma
frame, the behaviour is undefined.

**This is not a new restriction.** Natives are `extern "C"`, and unwinding out of an `extern "C"`
boundary is already undefined in C and aborts in Rust, so a native that unwinds was outside the
contract before this attribute existed. **What changes is the failure mode**: previously such a
native would most likely have crashed, and now it may miscompile instead. A C++ host must not let an
exception escape into a native, and a Rust host must not let a panic escape one.

The attribute is set on defined functions only. Declarations of host-provided natives are left
unmarked, because this backend does not generate that code and does not assert on its behalf.

Rationale and the measurement behind it: `docs/decisions/NOUNWIND.md`.

## Release rule: a skipped native gate step is NO-GO for a release that ships this backend

`scripts/release-gate.sh` builds this package in a step **conditional on an LLVM 22.1 development
install**. Without one the step does not run, and the gate can be green having never built the native
backend at all.

**The skip is loud** — it prints a warning naming what was not verified, and `scripts/gate-summary.sh`
shows it as a row reading `SKIPPED` with `0 binaries 0 tests`.

> ⚠ **A `0 binaries 0 tests` row is not self-explanatory, and this is worth knowing before trusting
> one.** A step that ran and had no tests to report prints the same shape as a step that never ran.
> **The only thing distinguishing them is the word `SKIPPED` in the step's own name.** Verified
> against a synthetic gate log: the markdown-link step and a skipped native step render identically
> apart from that word.

**Therefore**: green-with-a-skip is acceptable for routine development, and **is not acceptable for a
publication that ships the native backend.** A release built on a gate whose native step was skipped
has shipped a backend nothing in the release gate ever compiled.

The condition was set by the `v0.2.3` line, which owns the release process, when agreeing that this
step joins the release gate at the back-merge. **The corresponding rule in
`docs/process/RELEASE_PROCESS.md` is theirs to write**; this note records the requirement on the side
that owns the step.
