# BRIEF — test the clustering claim instead of trading on it

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Measure whether the corpus's structurally unusual cases concentrate in a few modules | yes |
| 2 | Keep or retract the clustering claim according to the measurement | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

Last increment ended with an observation, published in the handoff and the journal: three independent
investigations converged on `14_frame_log.kel::main` op 24, **"which suggests the corpus's awkward
cases cluster."**

**That is a hypothesis stated as a finding, and it has not been measured.** It is also the kind of
claim that changes behaviour — if true it justifies searching a handful of modules first; if false it
would send future work to the wrong place while feeling efficient.

Three convergences on one instruction is **weak evidence either way**. It is equally consistent with a
corpus where every module has one unusual property and this line simply asked three questions that all
happen to touch composites. **Distinguishing those requires counting, not more anecdotes.**

## Prior failures to avoid repeating

1. **Twelve recorded premises have been found false in consecutive increments**, several being this
   line's own written-down predictions. Write this one down too.
2. **A figure without its population is how two numbers get compared that measure different things.**
   Any distribution here must name the corpus it is over.
3. **`stack_growth`/`stack_shrink` are the peak model.** Any producer walk uses `op_depth_effect`.
4. **A guard whose population is narrower than its subject** has appeared three times. If properties
   are counted per module, count over the same corpus the other censuses use.
5. **An observation repeated becomes a premise.** This one is two documents old and already reads as
   established; that is precisely how the earlier stale figures survived.

## Specific wrong turns to avoid

- **Do not define "unusual" to fit the two modules already in mind.** Choose properties that matter to
  the backend for independent reasons — construction in a loop, storage into a slot, escape by yield,
  a multiply-written local — and let the distribution fall where it does.
- **Do not treat "these two modules appear in my notes often" as evidence.** They appear often partly
  because they were investigated often, which is selection by attention rather than by structure.
- **Do not report concentration without a baseline.** A property held by two of sixty-nine modules is
  concentrated; one held by forty is not, and the same words would describe both without the counts.
- **Do not extend the claim to "so search these first" unless the numbers support it.** The
  actionable half is the part most worth being wrong about.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
