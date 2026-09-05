# Which feature combinations actually build

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: First sweep, measured against the tree. Written 2026-09-05.

## Why this exists

Verifying an unrelated repair required `--no-default-features --features compile,verify`. **That
configuration did not compile**, and two further tests failed once it did. Five defects, all one
class: float-dependent code with no `floats` gate.

They were invisible for a precise reason. The release gate's no-default step is bare
`--no-default-features`, and every continuous-integration job that enables `verify` also enables
`floats`. **Nothing anywhere built that combination.**

This is the second occurrence of the class. V0.2.2 repaired "a verify-without-floats build failure
surfaced by stable 1.97+". A class that recurs after being fixed once is worth measuring rather than
fixing one member at a time.

**And the point generalises past floats.** The configuration in which a defect lives being the
configuration nothing builds is not a coincidence about that defect; it is the mechanism. A build
nobody runs cannot report anything, so whatever is wrong there accumulates silently until someone
wanders in.

## What was swept, and why these eleven

The full product of the feature set is meaningless: most points in it correspond to no deployment,
and a sweep reporting fifty broken configurations nobody wants is noise that gets the exercise
dismissed. These are chosen as shapes a host would plausibly ship.

| configuration | the deployment it stands for | build | what builds it today |
|---|---|---|---|
| bare (no default features) | precompiled bytecode, no verifier, no compiler | ok | CI and the release gate |
| `verify` | precompiled bytecode, verified, integer-only embedded | ok | **nothing** |
| `compile` | build-time compiler, no verifier | **was BROKEN** | **nothing** |
| `compile,verify` | compiler and verifier, integer-only embedded | ok (repaired earlier) | **nothing** |
| `verify,floats` | precompiled bytecode, verified, floats | ok | **nothing** |
| `compile,verify,floats` | the default | ok | CI and the release gate |
| `signatures` | signature checking alone | ok | **nothing** (CI's `--features signatures` is ADDITIVE to default) |
| `compile,verify,signatures` | signed modules, integer-only | ok | **nothing** |
| `encryption` | encryption alone | ok | **nothing** |
| `compile,verify,encryption` | encrypted modules, integer-only | ok | **nothing** |
| `compile,verify,floats,signatures,shell` | the broad docs.rs surface | ok | CI and the release gate |

**Nine of the eleven are built by nothing.** That is the finding, more than any individual failure:
the covered set is three configurations, and every other combination a host might ship is unverified
by construction.

## The one that was broken, and how it under-reported itself

`--features compile` — a compiler without a verifier, which the feature documentation presents as an
independent choice — **did not build.** Three test files import `keleusma::verify` while gated on
`compile` alone: `operand_stack_model.rs`, `host_model_independence.rs`, `opcode_reachability.rs`.

**The first run named only one of the three.** A failing test-crate build stops the others from
being reported, so the initial verdict was a lower bound on the breakage rather than a measure of
it — the same property this project already records for running the suite without `--no-fail-fast`,
appearing here in the build rather than the test phase. Each fix surfaced the next failure.

Repaired by widening those three gates. No source file was involved: `src/lib.rs` appeared in the
diagnostics only as "found an item that was configured out".

## The instrument, and its correction

Each verdict is **cargo's own exit status for that configuration**, captured per run, never a
pipeline's or a summary's.

**Its first revision was wrong in a way worth recording.** It reported implicated files by grepping
every `-->` in the log, which includes WARNING locations, so a PASSING configuration listed files as
though they were implicated. A reader would have chased them. It now reports locations only for a
failing configuration.

**Reach.** The sweep runs `cargo check --tests`, so it establishes that a configuration COMPILES and
nothing more. It does not run the tests, so a configuration marked ok here may still have failing
tests — `compile,verify` did, and they were found separately. **Compiling is the floor, not evidence
that behaviour was ever exercised.** The sweep also covers only the `keleusma` crate, and does not
touch `self-host` or `sdl3-example`. **It did not touch the mutually exclusive `narrow-word-*` and
`narrow-address-*` selectors either, and that exclusion is closed by the final section of this
document** rather than left standing as a permanent boundary — an unexamined exclusion is the shape
of gap this document exists to close.

`--all-features` is deliberately not used: it cascades the narrowest word and address selectors and
builds SDL3 from source, so it produces a confidently wrong answer here. The continuous-integration
workflow says as much in a comment on its broad-features job.

## A recommendation, with its cost, not adopted here

**Adding a configuration to continuous integration is a project-level call**, because every job
costs time on every push, so this recommends rather than adopts.

The highest-value single addition is **`--no-default-features --features compile,verify`**. It is
the only swept configuration that has already been shown to hide a defect of consequence — a module
that verifies, loads, and then traps, recorded in
[`INVALID_BYTECODE_CENSUS.md`](./INVALID_BYTECODE_CENSUS.md) — and it is where the float pin
compiles at all. Cost: one job, roughly the duration of the existing no-default job.

A cheaper alternative that covers far more: a **build-only** matrix step running `cargo check
--tests` over these eleven configurations. It would have caught every defect in this document and
the five before it, at a fraction of a test job's cost, because all of them were compile failures.
It would NOT have caught the two lex-time test failures, which need the tests to run.

## What must not be concluded

**A configuration marked ok is not a supported configuration.** It compiles. Whether its behaviour
is correct is a separate question this sweep does not address.

**The unswept combinations are not known good.** Nine features and several mutually exclusive
selectors make the unswept space far larger than the swept one.

## The narrow word, address and float selectors: all compile, none verified

The sweep above deliberately excluded the `narrow-*` selectors, because they are mutually exclusive
within their groups and would have made that matrix incoherent. **That exclusion was then left
unexamined, which is the shape of gap this whole document exists to close**, so they were swept
separately on 2026-09-05.

| configuration | build | what builds it today |
|---|---|---|
| default (control) | ok | CI and the release gate |
| `narrow-word-8` | ok | **nothing** |
| `narrow-word-16` | ok | **nothing** |
| `narrow-word-32` | ok | **nothing** |
| `narrow-address-8` | ok | **nothing** |
| `narrow-address-16` | ok | **nothing** |
| `narrow-address-32` | ok | **nothing** |
| `narrow-float-32` | ok | **nothing** |
| `narrow-word-16,narrow-address-16` | ok | **nothing** |
| `narrow-word-32,narrow-address-32` | ok | **nothing** |

**Measured, not inferred:** `narrow-word-16`, `narrow-word-32` and `narrow-address-16` appear **zero
times** in `.github/` and in `scripts/`. No continuous-integration job and no release-gate step
selects any narrow width.

### The result is a clean negative, and that is worth stating plainly

Ten of ten compile. **Nothing is broken on this axis today**, which is a different and better outcome
than the feature sweep found, and reporting it as a near-miss would be dishonest.

### What it does NOT establish, which is the part that matters

**The tests were not run under any narrow selector.** `cargo check --tests` establishes that a
configuration compiles. It says nothing about behaviour, and here that gap is known to be real rather
than theoretical: the project instructions record that `--all-features` fails under the cascaded
narrow configuration because **a test that pins 64-bit checked-addition semantics fails**. So
"compiles" and "passes" are known to differ for these selectors, and only the first was measured.

Whether that is one test or many is **unexamined**. It is the obvious next question and it needs a
test run per selector rather than a build.

### A narrow selector is not a cross-compilation target

Continuous integration builds `thumbv7em-none-eabihf` and `wasm32-unknown-unknown`. Those are
TARGETS. Selecting `narrow-word-16` on the host is a third thing: it changes the runtime's word type,
not the machine it runs on. **Neither covers the other**, and treating the embedded target builds as
coverage of the narrow widths would overstate what is verified.

### On the 8-bit selectors specifically

`tests/narrow_vm.rs` excludes `narrow-word-8` and `narrow-address-8` in its own configuration
attribute. That is evidence they are a narrower case than the others rather than a fully exercised
one, and it is recorded here so a future failure there is read against that exclusion rather than as
an unqualified defect.

### Recommendation, again not adopted

If a narrow configuration is ever added to continuous integration, the informative one is a
**coherent width** — a word and address width selected together — rather than a lone selector, since
that is what an embedded target actually looks like. The cost is one job per width. As above, this
is recorded rather than adopted: the standing per-push cost is the operator's call.
