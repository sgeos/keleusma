# Verification Pitfalls

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

These two catalogues lived in `CLAUDE.md` until 2026-10-24, when the kaizen line moved them here
to keep the always-loaded instruction file short. They are moved verbatim. `CLAUDE.md` keeps their
two correctives and a link to this file. Each row was paid for once, and the detail is in
[`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md).

## How a green local run has actually lied

Six observed ways a verification run reported success while covering less than assumed.
**The common property is that the run did less than the person reading it believed**, so
the corrective is knowing what a command covers, not running more commands. Each was paid
for once; the detail is in [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md).

| how it under-reported | what distinguishes it |
|---|---|
| Guards ran **before** the last edit | Check the guard's start time against the file's mtime, or re-run after the final change |
| `-p keleusma --test X` never enables `self-host`; `--workspace` unifies it on | A single-package run and a workspace run are different feature sets; dead-code warnings differ between them for this reason |
| `cargo test` stops at the **first** failing binary | A "1 failed" summary is not "1 failure in the tree"; `nextest` runs them all |
| A run whose subject was edited **while it was in flight** | Its result belongs to no tree. Discard it rather than reading it as a pass |
| A **cached** `clippy` run prints nothing whether or not warnings exist | `touch` a source first; zero warnings from a warm target directory is not evidence. **The tell is in the output**: a run that actually analysed anything prints `Checking <crate>` lines before `Finished`. A bare `Finished` means nothing was re-analysed, so it is not a result. Met again 2026-09-27 -- a `collapsible_if` in a just-appended test passed locally on a cached run and failed the CI Clippy job; local and CI clippy were the SAME build, `0.1.98 (48a229ceae 2026-09-01)`, so a version difference was hypothesised and disproved |
| A log **truncated** by `tail`/`head` looks identical to a clean one — **and so does any truncated LISTING, which is the wider class** | Capture the whole output and the exit status when the result will be quoted. ⚠ **Three instances in one session, 2026-09-27, all by one author who had read this row**: a `head -14` job listing recorded as "fourteen jobs" when the real count was 25 and propagated to four files; a test asserting on a guard's `head -20` path list, which **reddened the gate** when an absorption added a hundred alphabetically earlier paths; and `grep -c` counting matching LINES read as a count of occurrences, which then supported a whole wrong inference. **The shape is a human-facing SUMMARY mistaken for data** — assert on a verdict or a captured status, never on a display |
| A file's **own documented** reproduction command under-ran the file | `tests/float_opcode_without_floats.rs` named `--features verify`, which runs its defect pin and skips its `compile`-gated control. Check the test count against the file's `#[test]` count |
| **Three of the five feature sets, treated as all five** | A test calling `keleusma::selfhost` in a file not gated on `self-host` compiles under `self-host` and fails the other four. Turned three CI jobs red **in the same session that corrected this file's feature-set count**. Compiling under a feature set is also weaker than RUNNING under it |
| A **targeted run selected by binary name**, when the change adds a FILE | `-E 'binary(X)'` cannot see a census in binary Y that pins a population X has just joined. A new backend test file was green under every targeted run and turned the gate red in `host_buffer_census`, which enumerates the harnesses allocating host buffers — because three of them had once sized a region by a literal and **one reached the gate as a SIGSEGV**. Adding a file is not a local act |
| A run whose tree was **perturbed and restored** mid-flight | `frozen-run.sh` compares two SNAPSHOTS, so a file created and deleted inside the window leaves both identical and the verdict reads FROZEN. A probe file in `tests/` turned the narrow gate red against a pinned population count, on a tree that was never committed. **A `target/` write is harmless; a file in `tests/` is an input to the suite** |
| A **push** whose pre-push gate ran while commits landed | The pre-push hook runs the workspace suite against the WORKING TREE, so committing during it changes what its doc-reading guards read. Same class as the row above, and the rule *"do not create files in `tests/`"* was too narrow: **the tree must not move while ANY gate reads it.** Observed 2026-09-24; the run passed, which is not evidence it was safe |
| A gate run **phase by phase**, with the verdict ASSEMBLED by hand | The script accumulates `fail=1` across its phases and prints one PASS or FAIL; running the phases separately — which a ten-minute ceiling forces for a long one — moves that accumulation into the operator's memory, and **a phase can then be dropped with nothing saying so**. A `clippy` failure stood for four commits behind a suite figure recorded as green from "six runs" |
| A **launcher's** exit status, read as the suite's | `&` inside an already-backgrounded call reports the LAUNCH succeeding, not the run. The log carried no summary line and no captured status, and its single FAIL sat at test 306 of 669 — where a `tail -3` cannot reach. **A log missing its final summary is an IN-FLIGHT log, not a clean one**; require the summary line and the exit status together, from the same run |
| The run used a toolchain component the project does not **declare** | `examples/rtos` pins its toolchain, and the pin's component list omitted `clippy`. `cargo clippy` there passed on a machine that happened to have clippy installed for that channel anyway, and failed on a clean runner with `'cargo-clippy' is not installed for the toolchain`. The run was honest; it was not reproducible. Ask what your environment supplies that the repository does not require |
| A **count** whose pattern never matched reported zero | `shellcheck` emits `^--^ SC2004`; a counting grep searched for `^-- SC` and matched nothing, so eight findings were published as "zero findings". A zero from a pattern that cannot match is indistinguishable from a real zero. Print one matching line, or count with the tool's own exit status, before quoting a count of zero |

**A guard for that last row was attempted and ABANDONED, which is worth knowing before
trying again.** A text scan for a gated module path reached by an ungated item produced
false positives in three distinct shapes — a prefix match (`keleusma::selfhost` also matches
the ungated `selfhost_host`), an attribute walk starting at the `fn` line instead of above
it, and gates written indented inside a function — and each fix traded one class for
another. The weaker form, "the file mentions the feature somewhere", is useless here:
`tests/selfhost_parse.rs` already carried 41 such attributes, so a new ungated test hides
among them. A sound version needs brace-aware scoping. **The compiler already does that
correctly, in five configurations, which is what the gate runs** — so the instrument for
this class is the gate, not a scan.

## And a DIFFERENT class: the run was complete and the subject was never reached

Every row above shares the property that section states — the run did **less** than the
reader believed. These do not. In each, the command ran fully and honestly, reported
truthfully, and **the thing under test was never exercised**. A green result here is not
an under-report; it is an empty one.

| how it was empty | the instance that paid for it |
|---|---|
| A whole family produced **zero** cases, behind a healthy aggregate | A mutation corpus read 5184 mutants while the native-return-shape table had never been touched, because no corpus program called a native. An aggregate floor is satisfied by whichever population is largest |
| Cases attacked a check that is **not on the path under test** | Nineteen mutants of a field compared only at hot swap ran as passes against a fresh load, where nothing compares it. A mutation aimed at a check that is not on the path is indistinguishable from a passing one |
| A phase was **entered but never advanced** | Every accepted mutant got one argument-free call, so the coroutine reentry path — the one the typed pass explicitly DEFERS on — was never entered. The stage census cannot see this: it records where a mutant STOPPED, not what it DID |
| The **instrument shared a failure mode with its subject** | A recursive test builder overflowed the stack alongside the recursive walk it was measuring, so the reported boundary described neither. It also produced a second wrong reading before it was caught |
| The **harness** broke the subject and the result looked like a defect | A language server driven down a closed pipe cancels its work and looks exactly like a dead server — and two increments earlier, a probe reading exactly like that WAS one |
| A check passed **whenever its anchor was any ancestor** | `HANDOFF.md`'s ancestry validation passes forever on a forward-only branch, so it detects a reset and never staleness. It was 32 commits behind with four of five content checks still green |
| The job was attached to a branch the work never **reaches** | `book/ci/verify_examples.py`, described in its own workflow as running "on each CI run", triggered only on the default branch. Measured 2026-09-26 it had last run 2026-07-24 and had **never** run on the version branch, while `book/src` was edited there repeatedly. Nothing was red, because nothing ran |
| The harness used a **weaker command** than the claim required | `keleusma compile` writes bytecode without the structural pass and accepts a program `keleusma run` refuses. A rejection expectation checked with `compile` passes on a program the runtime would reject, so the documented refusal reads as confirmed while never occurring |
| The extraction matched **prose**, so a rewording silently emptied it | The same verifier asserted only the 51 examples whose surrounding sentence happened to say "output is" or "prints". 34 complete programs, including the principal examples of four chapters, passed through a `continue` that no count recorded |

**The corrective is different from the one above.** For the first table it is knowing what
a command covers. For this one it is **counting what was REACHED** — how many cases
arrived at each stage, how many actually ran, how deep the walk actually got — and
failing when a count collapses. Every instance here was caught by such a count, never by
the assertion it supported. The same applies to a control: a case that must SUCCEED is
what stops "everything was refused" from reading as "the refusals were meaningful".

**Also not exhaustive**, and it is the younger list. The three added on 2026-09-26 sharpen
the corrective rather than restate it: **ask what the check is attached to, and what
command it actually runs.** A count of cases reached cannot see a job that never fired,
nor a command too weak to reach the property under test.

**Not exhaustive.** Eight found across two sessions is evidence that more exist, not
that these are all of them. The seventh came from re-running a measurement whose feature
set continuous integration does not cover. **The eighth was committed by the author of the
seventh, two increments later**, which is the more useful fact: knowing a failure class
does not prevent producing it, and only running the configurations does. A run that is about to justify a claim deserves the question
"what did this command actually cover?" before the claim is made.

Five rows of the first table, from the targeted run selected by binary name to the launcher's exit
status, and the longer form of the truncated-log row, were written on the V0.3.X line's copy of
`CLAUDE.md`. They were carried here when that line absorbed this file's move on 2026-10-24.

