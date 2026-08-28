# BRIEF — the interprocedural residual, and a gate step never run

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals, as they actually stand

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Correct a false claim this line published about gate coverage | yes |
| 2 | Run the gate step that was never run, including `cargo doc -D warnings` | yes |
| 3 | Absorption 19 (`e88ce2dc`) | yes |
| 4 | Bound the interprocedural residual of the yield-escape obligation | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |
| 6 | Lower `Stream` (the largest coverage gap) | not in one increment |

## Rationale

**Goal 1 is first because it is a correction, and corrections decay.** The previous increment
published, in a commit message and in four documents, that *"neither `scripts/release-gate.sh` nor
CI covers this subproject"*. **That is false.** `scripts/release-gate.sh` runs `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test`, AND `RUSTDOCFLAGS="-D warnings" cargo doc`
over `native_codegen/`. The step is conditional on an LLVM 22.1 install and prints a loud SKIPPED
banner otherwise. **LLVM 22.1 is installed on this machine**, so the step would have run. The four
warnings accumulated because **the gate was never run on this line**, not because coverage was
missing. The true cause is worse and more useful: the everyday loop substituted `cargo test` for the
gate and never noticed.

**Goal 2 follows directly.** If the gate was never run, then `cargo doc -D warnings` has never seen
`native_codegen/`, and the previous increment added a large volume of doc comments with intra-doc
references. That is an unverified surface on new code, and the project has shipped a red Doc job
before for exactly this reason.

**Goal 4 is the honest completion of what the last increment left open.** The yield-escape refusal
covers one chunk at a time. A composite built in a loop body, returned to a caller, and yielded
there is the same defect and is invisible to it. That residual was *named* but never *measured*, and
an unmeasured residual is indistinguishable from an unbounded one.

## Prior failures to avoid repeating

1. **A claim about infrastructure was published without reading the infrastructure.** The gate script
   was four lines of `grep` away. **Read the file before characterising it.**
2. **"No corpus module has the shape" was carried across two documents and was false.** The corpus
   module was named in its own header. **Measure where a zero is expected; a zero that is never
   computed is not a zero.**
3. **A guard that could not fire passed for weeks.** Every predicate added here must be shown to
   fire on a positive instance AND to stay silent on the nearest benign neighbour.
4. **A cost census that double-counted.** Nested loop scopes report one site several times unless
   deduplicated. Any new census must state its unit and be deduplicated at that unit.
5. **Isolation slipped when files were edited mid-run.** Establish that the build phase completed
   before any edit, or re-run.

## Specific wrong turns to avoid

- **Do not treat the interprocedural gate as free before measuring it.** Refusing every chunk that
  constructs in a loop and contains a `Call` could refuse a large fraction of the corpus. **Measure
  the newly-refused count first; wire only if it is acceptable, and say what it costs if it is not.**
- **Do not conflate "callee yields" with "the composite reaches the callee".** The former is an upper
  bound. Name which one the census reports, and in which direction it errs.
- **Do not follow calls without a visited set.** The call graph may contain cycles; a naive walk
  will not terminate. Termination must be structural, not hoped for.
- **Do not claim the obligation is discharged.** Even with the interprocedural case covered, slot
  reuse itself is unchanged. This line has already made that mistake once and had to retract it to
  the proofs line.
- **Do not edit `docs/proofs/` argument text.** Annotate only; that document belongs to another line.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
