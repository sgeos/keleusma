# Brief — the linkage census on the target that actually matters

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01**, immediately after the host census.

---

## The goal set

| goal | owner | state |
|---|---|---|
| **G3** the host linkage census | this line | measured; awaits a green suite before landing |
| **G4** the same census on a bare-metal ARM target | this line | **unblocked, and the subject of this brief** |
| **G2** absorption 43 | this line | gated: a measured absorption needs a solo run, and this line is holding runs while a peer's gate is in flight |

The runtime arithmetic width, `f16`, `Text<N>` and `Opaque` remain someone else's. **Three goals, one
of them actionable, is the honest count.**

## Why this follows directly, and why it is not a repeat

[`LINKAGE_SYMBOL_CENSUS.md`](./LINKAGE_SYMBOL_CENSUS.md) closes with its own limit, in its own words:

> *"One target, one machine. `aarch64-apple-darwin`, hardware floating point, hardware 64-bit divide.
> It says nothing about `thumbv8m` or any target without hardware floating point, **which is the case
> precondition 3 actually cares about**."*

**A record that names its own gap and stops has done half a job when the gap is one target string
away.** The host measurement found `Fixed` division reaching for `__divti3` because no target has a
128-bit divide. That reasoning is target-independent. **Everything else about the host result is
target-dependent, and the host is the least representative target this project has.**

`examples/rtos/` targets `thumbv8m.main-none-eabihf`. It is the flagship embedded example, it ships a
`memory.x` and a probe-rs runner, and it is the deployment shape the ecosystem value proposition is
written for. **If linking a Keleusma object there requires symbols nobody has named, that is a real
defect in a real example, not a hypothetical.**

## The prediction, recorded before measuring — and written so it can fail

The host prediction was criticised in its own record for being **equally consistent with the
interesting and the boring outcome**. This one is written to avoid that.

**Predicted: the narrow target requires STRICTLY MORE runtime symbols than the host, and the increase
is dominated by 64-bit integer and double-precision floating-point helpers.**

Reasoning, stated so it can be wrong for a nameable reason: a 32-bit target has no 64-bit divide
instruction, and `Word` is 64-bit by default, so `Word` division should reach for a helper **where the
host did not**. `thumbv8m.main-none-eabihf` has a single-precision floating-point unit; Keleusma's
default `Float` is `f64`, so double arithmetic should lower to runtime calls.

**Three named falsifiers, any one of which refutes it:**

1. The narrow set is a subset of the host set, or equal to it.
2. `Word` division is still clean on the narrow target.
3. Float arithmetic is clean on the narrow target.

**If the prediction holds, it is an argument for the float ladder that has nothing to do with
size**: on the flagship embedded target, every `f64` operation is already a function call, so `f32`
and `f16` buy native instructions rather than merely narrower storage.

## What could go wrong, and what to do about it

**The target may not be registered in the LLVM build.** `Target::initialize_native` only registers the
host. The ARM target needs its own initialisation, and if the linked LLVM was built without it,
`from_triple` fails. **That is a finding, not an error to route around**: report that the census cannot
be taken here and say so, rather than silently measuring the host again under a narrow-sounding name.

**The symbol reader must handle ELF.** The host objects are Mach-O. Verified in advance: `/opt/local/bin/nm`
is `llvm-nm` and reports itself GNU-compatible, so it reads both. **Do not assume it; the census
asserts non-vacuity and a reader that returns nothing on ELF would satisfy every absence claim.**

**Symbol decoration differs by format.** Mach-O prefixes every symbol with an underscore; ELF does
not. The existing reader strips one leading underscore unconditionally. **On ELF that would corrupt a
genuine `__aeabi_` name into `_aeabi_`.** Strip by format, not by habit.

## The specific wrong turns, from failures already recorded this session

**1. A test verified alone is not verified.** The host census passed five tests under
`--test-threads=1` and failed two in the suite, because three tests shared a scratch directory and
deleted each other's objects. **Any new sweep gets a per-call directory from the outset**, and it is
run under the harness's ordinary parallelism before it is believed.

**2. An instrument at the wrong level reads green.** Twice today: an IR search for a fused
multiply-add that could not see codegen, and an IR-level symbol census that would have missed
compiler-runtime calls entirely. **Read the object.**

**3. Do not infer a cause from a neighbouring comment.** The backend's comment beside its 128-bit
widening pointed at checked arithmetic, and two probes built on that inference returned wrong answers
before a sweep found `Fixed` division. **Isolate the construct; report contrasts, not a single
positive.**

**4. Do not edit test sources while a suite runs.** Broken once today, on this line, with a rule
against it in the handoff being validated the same morning.

**5. A count is not a census.** Name the symbols. An unclassified symbol stays unclassified.
