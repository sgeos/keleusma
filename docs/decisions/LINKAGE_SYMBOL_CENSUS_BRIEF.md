# Brief — what must a host supply to link a Keleusma native object?

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01.**

---

## The goal set this belongs to

Three goals were available on 2026-09-01. Two of the three are constrained by things outside this
line, and naming that is part of the recommendation rather than an excuse.

| goal | owner | state |
|---|---|---|
| **G1** land the fused-multiply-add guard | this line | built; waits only on a green suite |
| **G2** absorption 43, discharging the standing 69-module prediction | this line | measurable, but a measured absorption needs a solo run and this line has agreed to hold runs while a peer's gate is in flight |
| **G3** the linkage symbol census | this line | **unblocked, and the subject of this brief** |

**Everything else genuinely on the roadmap is blocked on another line or on the operator**: the
runtime arithmetic width, `f16`, `Text<N>`, and `Opaque` sized by `addr_bits_log2`. **Inventing a
fourth goal to look busy would be worse than naming three.**

## Why this one, and why now rather than later

`FLOAT_LADDER.md` precondition 3 already records it, and records it as deferred:

> *"On a target without hardware support LLVM lowers narrow float operations to compiler runtime
> calls. A linked C host will need those symbols, which is a packaging question the JIT path never
> asks. **Worth checking when `f16` lands rather than at the link failure.**"*

**"When `f16` lands" is the wrong trigger and this brief overrules it.** The question is not whether
`f16` will introduce compiler-runtime dependencies. It is what the deployment shape's external
symbol set *is*, which has an answer today and is a fact about every module the backend emits. If
that set is already non-empty beyond the host's own natives, the packaging question is **live now**
and `f16` merely widens it. Discovering that at an `f16` link failure would mean discovering two
things at once and attributing both to the new rung.

The roadmap's own success criterion is that native artefacts **"link as static libraries against a
host"**. A criterion phrased that way is not met by one example linking. It is met by knowing what
linking requires.

## What to build

For corpus modules that lower, emit a real object at the host target and read the object's
**undefined** symbols. Partition them:

- **(a) host-registered natives** — the embedder's contract, expected and fine.
- **(b) compiler runtime and C library** — `memcpy`, `memset`, `__truncdfsf2` and relatives. **This
  is the category the census exists to name.**
- **(c) anything else** — unexpected, and the finding if non-empty.

Pin the partition. State what it implies for `f16` rather than leaving the reader to infer it.

## The prediction, recorded before measuring

**At `f64` on a host with hardware floating point, category (b) is expected to be small and may be
empty.** `memcpy` or `memset` would not surprise me, since composite copies can lower to them.

**Falsifier**: any category (b) symbol that is a floating-point helper. That would mean the runtime
dependency exists already at the shipped rung, and precondition 3's deferral was wrong on its facts
rather than merely on its timing.

**If the prediction fails, say it failed.** This line has a recorded habit of adjusting a figure to
meet a prediction, and the whole value of recording one first is lost the moment that happens.

## Prior failures on this line, and the specific wrong turns to avoid here

**1. An instrument at the wrong level reads green.** Earlier today a guard searched LLVM IR for a
fused multiply-add and passed while measuring nothing, because fusion is a codegen transform and is
first visible in machine code. **The identical trap is live here**: an IR-level census of `declare`
statements without bodies would MISS exactly category (b), because compiler-runtime calls are
synthesised during code generation and do not appear in the IR at all. **The census must read the
object file.** An IR-level answer would be confidently, silently wrong about the one category it was
written for.

**2. A guard that only ever reports absence.** "No unexpected symbols" holds trivially for an empty
object, a refused lowering, or a module that emitted nothing. **A non-vacuity assertion comes first**,
and it must establish that objects were emitted and that the reader found symbols in them at all.

**3. A mutation must perturb the LOWERING, not the subject.** A differential and a census are both
invariant to which program is compiled. If the reader is to be trusted, demonstrate it against an
object known to reference an external symbol, not against a different `.kel` file.

**4. The pipeline-status trap.** It fired three times in one session, every time in a command written
to verify work. A `grep` that finds nothing exits 1. **A status to be read gets its own line, and a
filter never goes last.**

**5. Do not edit test sources while a suite runs.** Both this line and the `v0.2.3` line broke this
rule within one hour today. This suite contains tests that read source text from disk.

**6. `git add -A` once swept an unverified test file into a documentation commit.** Stage explicitly.

## What would make this work worthless

**Reporting a count instead of a list.** "Four undefined symbols" answers nothing. The deliverable is
the names and their partition. If a symbol cannot be classified, it belongs in category (c) and is
reported as unclassified rather than assigned to whichever category makes the total tidy.

**Claiming portability from one target.** The measurement is the host target on one machine. It says
nothing about `thumbv8m` or any target without hardware floating point, which is the case
precondition 3 actually cares about. **State the limit; do not let the reader infer generality.**
