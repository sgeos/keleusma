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
| `IsStruct` | **refused, loudly and by name** — `UnsupportedOp { op: "IsStruct" }`, driven by `tests/is_struct_verdict.rs` against a hand-mutated module, with a control that lowers cleanly. No source can supply the witness: the reference emits it only where a struct pattern's annotated type differs from its own struct name, and the type checker refuses every such program. Refusing is consistent with the reference, whose machine returns an error for `IsStruct` against a FLAT struct because the test is then a compile-time constant |
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


## Bytecode coroutines

`coroutine::lower` provides the Workstream B retcon path for verified streams
and reentrant entries with scalar or flat arguments. Its
[module documentation](src/coroutine.rs) specifies the provisional host contract,
completion status and checked frame reservation. [Execution tests](tests/retcon_bytecode.rs)
compare suspension and completion against the virtual machine. Reentrant and
stream callees share the LLVM continuation. Reentrant entry arguments survive
resume unchanged. Only a stream entry's first parameter receives each reply.
The convenience entry selects a uniform dialogue. `lower_with_dialogues` accepts
independent output and reply shapes at every yield, with a host site query.
Parameterless reentrants can receive any supported declared reply. Host buffers may be reused
once start or resume returns. Owned flat completion results survive destruction
of the continuation frame in a bounded caller-owned tail reservation.

The stable arena handle supports start, resume, completion, release and reuse.
`lower_with_contracts` also accepts native flat result lifetimes. A `Snapshot`
is copied into bounded instance storage before the host reuses its buffer.
An `InstanceBorrow` retains aliases into the instance's declared regions.
The host must uphold the chosen lifetime and packed representation. Calls with
uncontracted flat results remain refused. These contracts do not attest native
execution cost. Unused chunks impose no dialogue contracts or native body emission,
while whole-module verification and resource admission remain mandatory.

Composite reads and transfers require extents proven across control-flow joins.
Private-slot kinds are inferred from writes and preserved in runtime metadata.
Mixed enum and scalar inspection checks the kind before reading a body. Owned
mixed values retain their selected size in bounded storage. Array bounds use
that actual size, while field reads require a proven minimum. Reset clears
runtime metadata with the corresponding locals. Possibly cleared operands have
kind checks at typed consumers. Equality and enum inspection preserve valid Unit
behavior. Composite construction packs actual operand sizes within its proven
reservation. Tuple and array lengths vary with those values. Struct and enum
padding is zeroed. Variable field reads and host transfers check the selected
length before access. Internal calls carry actual kinds and extents through
parameters and returns. Private composite slots preserve actual view lengths,
including older aliases after shorter writes. Reads and writes preserve Unit
until a consumer requires a body. Private copies permit overlapping aliases.
Private scalar values retain their actual kinds across suspension and release.
Zero a fresh private region before installing its initialization image. Preserve
that region when reusing a frame with existing private values. Scalar kind
metadata fits within the checked existing reservation. Ordering selects the
runtime numeric kind and rejects unequal kinds. Checked and bare arithmetic
also select actual numeric kinds. Checked outputs preserve the virtual machine's
low value, high value and status classification. Bare integer zero division traps.
Stream replies obey the virtual machine's scalar type checks. Composite stream
parameters may receive other bounded flat shapes or scalar replies through
explicit dialogues. Actual reply kind, length and ownership survive delegated
yields and Reset. Typed consumers check the selected value before reading it.

| Host control model | Lowering entry |
|---|---|
| Atomic function or existing synchronous callback integration | `lower_module` with its documented admission preconditions |
| Host-driven start, suspension, resume and completion | `coroutine::lower` or an explicit-contract variant |
| Proven degenerate stream step using the existing step contract | `lower_module`, with the actual degenerate shape checked by that route |

The routes do not select or fall back to one another. The coroutine route accepts
Stream and Reentrant entries and emits no `kel_yield` callback. The execution suite
checks these distinctions, links snapshot completion from a C host, and checks
object emission for the roadmap's four tier-one platform and architecture pairs.
Cross-target emission does not establish execution or timing on those targets.
The [completion requirements](../docs/decisions/WORKSTREAM_B_COMPLETION.md) track
full Workstream B. The bounded baseline merged through
[pull request 478](https://github.com/sgeos/keleusma/pull/478) as `a1aa53b8`.
Historical complete native gates cover `5a0fec1d`, not subsequent trees.
