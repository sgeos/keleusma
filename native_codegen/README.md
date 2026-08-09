# keleusma-native

> **Navigation**: [Documentation Root](../docs/README.md) | [V0.3.x Roadmap](../docs/roadmap/V0_3_X_ROADMAP.md)

LLVM native code generation for verified Keleusma bytecode. V0.3.x Workstream A.

**Status: early subset.** 22 of the instruction set's 66 opcodes lower. This is
not yet a code generator for the language; it is the beginning of one, with the
differential oracle in place first so that widening the subset is checked from
the start.

Scoping for the remaining 44, and the two open analyses, is in
[`docs/decisions/NATIVE_LOWERING_INVENTORY.md`](../docs/decisions/NATIVE_LOWERING_INVENTORY.md).
**The next increment is structural rather than another opcode**: `Loop` and
`Break` introduce backward jumps, which the current merge-depth walk cannot
express, and almost everything real needs iteration.

## What it does today

Takes a `Module` from the Rust reference compiler, lowers a chunk to LLVM IR,
and produces either a JIT-executed function or a native object file. Correctness
is established by executing the same bytecode on the VM and requiring identical
results.

Supported opcodes: `GetLocal`, `SetLocal`, `PopN`, `Dup`, `CheckedAdd`,
`CmpEq`, `CmpNe`, `CmpLt`, `CmpGt`, `CmpLe`, `CmpGe`, `Not`, `BitAnd`, `BitOr`,
`BitXor`, `Shl`, `Shr`, `If`, `Else`, `EndIf`, `Return`, `Trap`. Anything else
is **refused** with `LowerError::UnsupportedOp` rather than lowered to something
plausible.

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
