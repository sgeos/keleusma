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

> **CORRECTED 2026-09-08: "compile, never run" is TRUE OF THE FEATURE CONFIGURATIONS AND FALSE OF
> NARROW RUNTIMES.** This section says no continuous-integration job selects a narrow width, which
> is true, and then lets that stand as "narrow widths are not exercised", which is not. **They are
> exercised on every run**, through host-defined aliases in the DEFAULT build.
>
> `tests/narrow_vm.rs` defines a sixteen-bit-word runtime, a sixteen-bit word with a wide float, and
> an eight-bit word with a sixteen-bit address -- the 6502 shape. **32 of its 37 tests drive one of
> them**: 14 name an alias directly and 18 reach one through its helpers, leaving 5 that do not
> (they exercise the FLOAT width on a wide-word runtime). 14 + 18 + 5 = 37.
> `tests/composite_width_skew.rs` adds 9 more. **41 tests in total, in every continuous-integration
> run.**
>
> **The two claims differ in what they cover.** A host alias narrows ONE runtime inside ONE test, so
> coverage is whatever those tests do. A `narrow-*` feature narrows the bundled `Vm` alias, so the
> WHOLE suite runs at that width -- and that is what nothing builds, and what the failure counts in
> this document describe. Both statements have content; stating the first as the second overstates
> the gap and sends a reader looking for coverage that already exists.
>
> **How the 41 were identified, and what that would miss**: by parsing each test for the alias its
> body names or the helper it calls. A test reaching a narrow runtime through a deeper indirection
> would not be counted, so 41 is a lower bound.
>
> **This is the third claim in this document family stated more broadly than its evidence**, and
> naming the pattern is more useful than fixing the third quietly. The first was "the reason is not
> a runtime defect", refuted by five failures. The second was "nothing builds this configuration",
> true of features and false of widths, since a host alias reaches any pair. This is the third. All
> three are mine, and all three read as flat statements where the evidence supported a narrower one.

### What it does NOT establish, which is the part that matters

**The tests were not run under any narrow selector.** `cargo check --tests` establishes that a
configuration compiles. It says nothing about behaviour, and here that gap is known to be real rather
than theoretical: the project instructions record that `--all-features` fails under the cascaded
narrow configuration because **a test that pins 64-bit checked-addition semantics fails**. So
"compiles" and "passes" are known to differ for these selectors, and only the first was measured.

Whether that is one test or many is **unexamined**. It is the obvious next question and it needs a
test run per selector rather than a build.

### A narrow selector is not the only way to reach a narrow width

**Added 2026-09-08.** The table above reports that nothing builds any narrow selector, which is true
and is a statement about the FEATURE matrix. It should not be read as saying narrow or skewed widths
are unreachable. `GenericVm<W, A, F>` is generic over the word and address types independently and
every `Word` and `Address` implementation is unconditional, so an ordinary test in the DEFAULT build
can drive any pair of widths through a host-defined alias. `tests/composite_width_skew.rs` does
exactly that.

**Where a defect is width-dependent, it can therefore be guarded at no standing per-push cost**, which
is a cheaper answer than the job this document recommends and does not adopt. That does not extend to
the feature axis: a build omitting `floats` cannot be reached by an alias.

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

## Running the tests under `narrow-word-16`: the open question, answered

> **SUPERSEDED 2026-09-08 in one conclusion, and it is the section's headline.** Every failure has
> since been classified individually in
> [`NARROW_WIDTH_FAILURE_CLASSIFICATION.md`](./NARROW_WIDTH_FAILURE_CLASSIFICATION.md). **Five of
> them were a runtime defect, not a test premise**, so the claim below that "the reason is not a
> runtime defect" is REFUTED and is marked as such where it appears. The counts here are also a
> lower bound: the finished run has **41** distinct failures across **16** binaries, not 37 across
> 15. The rest of this section stands, and is kept because how the wrong conclusion was reached is
> the useful part.

The section above said whether the narrow-selector test failures were "one test or many" was
unexamined, and that answering it needed a test run rather than a build. **A question raised in a
durable document and then abandoned reads as a lead for someone else**, so it was run.

### The measurement, and its status as a LOWER BOUND

`cargo test --features narrow-word-16 --no-fail-fast`, on 2026-09-05.

| | |
|---|---|
| test binaries passing | 89 |
| test binaries failing | **15** |
| distinct failing tests | **37** |
| completeness | complete except for one binary, killed — see below |

**One qualification travels with these figures.** The `perf_canary` binary did not terminate and was
killed; the run then completed normally. So the only tests unaccounted for are that binary's TWO, and
the counts are otherwise a total rather than a lower bound. **A first draft of this table said 84
passing**, a figure read while the run was still going and corrected here, which is the reason to
take a count from a finished run rather than from a progress line.

### The answer is "many", and the reason is ~~not~~ MOSTLY not a runtime defect

**This heading was wrong as first written, and the row that carried the error is in the table
below.** Five of these failures are a defect in the virtual machine. The rest of the paragraph is
accurate for the other thirty-six.

The failures cluster, and the clusters share a premise rather than a cause in the virtual machine:

| group | size | what they presuppose |
|---|---|---|
| `*_narrows_to_the_declared_width` (add, sub, mul, div, the checked forms, int-to-float) | 9 | a module declaring a NARROW width running on a WIDER runtime. At a 16-bit runtime that premise is void |
| opaque and flat-composite layout | 5 | **THIS ROW WAS THE DEFECT.** Its guess -- "handle widths taken at the host's natural width" -- was close to the mechanism, and it was still filed as a test premise |
| float-width classification | 5 | the encodable float widths available at the default configuration |
| the remainder | 18 | not individually examined |

**Per-test verdicts were NOT made.** The grouping is by name and by the premise the name implies,
which is weaker evidence than reading each test, and it is recorded at that strength deliberately.

**Recording the weakness was not enough to stop it misleading.** The opaque row above names a
plausible mechanism and files it under "what they presuppose", and a reader with the surrounding
prose would take it as another test assumption. Reading the five assertions found a wrong ANSWER
sitting among them. **A caveat about an instrument's strength does not make its output safe to
build on.**

### The worked example, which settles the character of the whole set

`perf_canary::constant_loads_in_a_loop_stay_fast` ran for **57 minutes at 99% of a core** and never
finished. It is a performance canary, so a reader's first instinct is a performance regression at a
narrow word.

It is not. The program the test compiles is
`for i in 0..hi limit 200000 { d.s = d.s + 1234567 + i; }`, and **both `1234567` and `200000` are far
outside a 16-bit word's range**, whose maximum is 32767. The test's own parameters cannot be
represented at the width it was asked to run at. It is written for a wide word.

That is the character of MOST of this set: **the suite assumes a 64-bit host runtime**, and running
it at 16 bits mostly measures that assumption. For thirty-six of the forty-one that is a statement
about the tests. For the other five it was a statement about the runtime, and the word "mostly" was
carrying more weight here than it could bear.

### What this does and does not license

**~~It does not say the narrow widths are broken.~~ RETRACTED 2026-09-08.** This said nothing here
demonstrated a defect in the virtual machine at a narrow word. Five of the failures did, and reading
them is all it took. Every configuration still compiles, which the section above establishes, but
compiling was never the question.

**It does not say they work, either.** A suite that cannot run at a width cannot vouch for it.

**But "the narrow widths are unverified" -- as this sentence originally read -- is too broad.** What
is unverified is the whole suite AT a narrow width. Narrow runtimes themselves are driven by 41
tests in the default build; see the correction above.

**Making the suite run at 16 bits is a real project**, not an increment: it means auditing every test
that bakes a wide literal or a wide expectation, and deciding per test whether to widen a
configuration attribute, parameterise the constant, or leave it excluded. Nothing here should be read
as a recommendation to start that without deciding it is worth the cost.

**One thing is worth fixing on its own merits regardless**: a performance canary that spins for an
hour instead of failing is a poor citizen in any configuration, and a bound on its own runtime would
cost little.
