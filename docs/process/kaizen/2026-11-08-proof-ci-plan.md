# A Long-Term Plan for Proofs in Continuous Integration, 2026-11-08

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The proofs line asked on 2026-11-08, in interline sequence 408, for a long-term plan for
running its proofs in continuous integration, at its operator's direction to confer with
this line, which holds the project's runner strategy. This record answers its four
questions. It is a proposal. **No job is added by it**, because the operator ruled, as the
proofs line's handoff records, that a new job in `.github/workflows/ci.yml` or running any
proof `check.sh` in CI waits for the release or for agreement with the V0.2.X line.

## The constraints

- **Fact.** GitHub's standard Linux runner for a public repository has 4 processors, 16 GB of
  memory and 14 GB of disk. Larger runners are available only to organizations on the Team
  and Enterprise Cloud plans, and this repository belongs to a personal account. Whether to
  change that is the operator's check K9.
- **Fact**, from the proofs line's report. The Kani light tier, five harnesses expected to
  verify and three expected to fail, ran green locally in 238 seconds one at a time and 178
  seconds two at a time. Its largest measured peak is 336 MB. One heavy-tier harness held
  about 30 GB for more than 30 minutes before it was stopped, and ten heavy harnesses have
  never run.
- **Fact.** Each of the five Rocq `check.sh` scripts and the difftest runs in under 30
  seconds locally once built. Rocq 9.3.0 comes from opam, and the extraction needs OCaml.
  Kani needs its own nightly toolchain and its CBMC bundle.
- **Fact.** A scheduled workflow runs only from `main`, which `fuzz.yml` handles by checking
  out the version branch named in `FUZZ_REF`.

## The plan

**Phase 0, now.** No CI job. The proofs line lands its work on its branches, runs the light
tier and the Rocq checks locally under the shared machine's playbook, and records each
harness's peak where `/usr/bin/time` allows. Kaizen raises no objection to landing the Kani
work as it stands, since nothing runs it in CI yet.

**Phase 1, once the V0.2.X line agrees or the release is cut.** One job in `ci.yml`,
`proofs`, which runs the five Rocq scripts, the difftest and the Kani light tier.

- **(a) The light tier runs in CI, and so do (c) the Rocq scripts and the difftest.**
  Together they are minutes of work and well inside a standard runner, and a proof that CI
  does not check can rot unnoticed.
- **Triggers.** A pull request or branch run whose changes reach `docs/proofs/rocq/`,
  `tools/proof-difftest/`, `tools/proof-kani/`, `src/verify.rs`, `src/bytecode.rs` or
  `keleusma-wire/`, through a new output of `scripts/ci_classify.py` in the way the VS Code
  extension's job is triggered, and every full run once a week, so that drift in an unlisted
  dependency is still caught. The job joins `certify` and `CI result`, so it gates merges as
  the other jobs do.
- **Installation.** Kani by `cargo install --locked kani-verifier` at the version the script
  pins, then `cargo kani setup`, and Rocq through the `ocaml/setup-ocaml` action with opam
  pinned to the versions the scripts pin, both cached by version so that a warm run spends
  its time on the proofs. Every action pinned by commit, as the workflow's others are.
- **Evidence.** The job records each harness's peak with GNU `time -v`, which works on the
  runner, and keeps the peaks and every log as an artifact. `scripts/metrics.py` already
  counts the Rocq results and admitted steps from the tree.

**Phase 2, the heavy tier.** **(b)** Not on standard runners as it stands, since one harness
needed about 30 GB.

- The proofs line first splits each walk harness by instruction selector, as it intends,
  and measures each piece alone, locally, under the playbook.
- A piece whose measured peak is under 12 GB joins a separate scheduled workflow,
  `proofs-heavy.yml`, weekly, from `main` with the version branch checked out as `fuzz.yml`
  does, running one harness at a time with a per-harness time limit and a `ulimit -v` cap,
  which Linux enforces. It is advisory, outside `CI result`, so that a heavy run never blocks
  a merge, and a failure opens visibly in the run list as the fuzz job's does.
- A piece above 12 GB stays a local run under the playbook, unless the operator moves the
  repository to a plan with larger runners, which K9 would decide.

## Parallelism, question (d)

Choose it from data, not from the machine it was tuned on.

- **A tracked baseline of peaks.** The proofs line commits the peaks it measures, one line
  per harness, beside the script. A job reads them.
- **Each job derives its parallelism** as the smaller of the runner's processor count and
  the runner's available memory, less a 2 GB reserve, divided by the largest recorded peak
  among the harnesses it will run, and never less than 1. On a 16 GB runner and the light
  tier's 336 MB, that gives 4, which the processor count caps.
- **A harness without a recorded peak runs alone**, so that the first run of something new
  measures before it widens, as the playbook says.
- **CI keeps the measured peaks as an artifact.** When they differ from the baseline, the
  proofs line updates the baseline in its next pull request, so that the figures follow the
  harnesses rather than a guess.
- **On this machine the script keeps its machine-status gate**, which already holds a job
  back at elevated, and runs where no monitor exists, as on a runner.

## What is asked, and of whom

- **V0.2.X.** Whether it agrees to Phase 1 before the release, since the job adds CI time
  to its pull requests that touch the verifier. Kaizen asks in the interline mailbox.
- **The operator.** K9, which decides whether larger runners are ever available for the
  heavy tier.
- **The proofs line.** To split and measure the heavy harnesses, and to commit the peak
  baseline. Kaizen writes the Phase 1 job once V0.2.X agrees, since the workflow is shared
  tooling this line maintains, and the proofs line reviews it.
