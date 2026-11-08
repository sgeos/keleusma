# Brief — two opcodes have no verdict, and the reason is not missing support

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02, night.**

---

## The present goals

| goal | state |
|---|---|
| **the verdictless opcodes** | **the subject of this brief** |
| `f16` | blocked, no oracle. The reference refuses `float_bits_log2` 3 and 4 at load, so a binary16 module never runs on the reference side |
| publication | held |
| absorption 48 | nothing unabsorbed, verified by fetch |
| the workspace staleness clause | **done**, and executable |

## What the two censuses actually say

Measured this session, both under default features on the `native_codegen` package.

| census | figure |
|---|---|
| ISA lowering census, over 74 corpus modules | **63 of 66 lowered** |
| backend support census, over 16 hand-built probes | **15 lower, 0 refused, 1 never visited** |

Three opcodes sit outside the lowered set, and **they are outside it for three different reasons**,
which is the part a single number destroys.

- **`Len`** is named in a refusal. That is a verdict. It is not this brief's subject.
- **`Reset`** is emitted and **never visited**. Neither census can say lowered or refused.
- **`IsStruct`** has **no corpus witness and no probe**. Nothing has ever put it to the backend.

**Only the last two are verdictless**, and a reader who sees `63 of 66` will infer that three opcodes
are unsupported. **That inference is wrong for at least one of them.**

## The hypothesis, which is to be tested rather than asserted

Reading the backend shows `Reset` handled by **shape recognition** rather than by opcode dispatch. A
degenerate stream has the form `Stream ; <body> ; Yield ; PopN(1) ; Reset`, and in that shape `Stream`
and `Reset` lower to nothing. On the general path both are refused.

So the likely truth is that **`Reset` behaves exactly as `Stream` does** — accepted in one shape,
refused in another — and the ISA census already reports `Stream` in precisely that dual position. If
so, **the `UNPROVEN` row is an artifact of where the census instruments**, not evidence of a gap.

**This is a hypothesis obtained by reading source, which is the weakest kind of evidence this line
accepts.** It must be established by driving the backend and recording what it did.

## Prior failures this brief exists to avoid repeating

**A census that measured the wrong site is this line's recurring defect.** The fused-multiply-add
search passed at the intermediate-representation level while fusion is a later transform, so it
measured nothing. Earlier this same session, a reach test asserted a classifier's verdict and not its
contents, and stayed green under a mutation that mis-classified the source tree. **An absent verdict
is not a negative verdict**, and the instrument's blind spot is the thing to check first.

**A count read past its population.** `63 of 66` is true about the corpus census. It does not mean
three opcodes are unsupported, and the two censuses are explicitly complementary, neither subsuming
the other.

## The wrong turns

**1. Do not conclude that `Reset` is supported because the source contains a pattern match for it.**
That is the inference this line has been burned by. Compile something and observe.

**2. Do not report a single verdict for an opcode whose treatment depends on shape.** `Stream` is
already recorded as lowering and refusing. Collapsing that to one word loses the only interesting
fact.

**3. Do not treat an acceptance as a correctness claim.** `lower_module` returning success is a fact
about the compiler, not about the emitted code. This line has already shipped an opcode whose
saturating clamp no program reached.

**4. Do not add a probe that cannot fail.** A probe asserting only that compilation succeeded proves
nothing about the opcode if the opcode never reached the backend. That is exactly how `Reset` came to
sit in a column marked never-visited: the existing probe emits it and the lowering never sees it.

**5. `IsStruct` may legitimately have no witness.** If no corpus program can emit it and no probe can
construct it, that is a finding about reachability and should be recorded as one, not forced.
