# BRIEF — what to measure first when the gate returns

**Filed 2026-09-18, against `c090dae6`, while the toolchain cannot link.**

> ⚠ **STATUS.** Filed before its own work lands, like every brief. This one
> describes a plan for a future run rather than a gap in the tree, so its claims
> are about what is UNVERIFIED, which the passage of time can only widen.

## Why a plan is worth writing while blocked

**Gate time is this project's stated bottleneck** — the reason the merge rule was
changed in August was that two sessions were serialising on one machine. A full
backend gate is about twenty-five minutes per float configuration, so an ordering
decided in advance is worth more than the same decision made hurriedly later.

And there is a specific trap to defuse now rather than then.

## ⚠ FOUR COMMITS ARE "DOCS-ONLY" AND THAT IS NOT A REASON TO SKIP THE GATE

`51542c99` was gate-verified in both configurations. The four after it —
`af51d7d7`, `7fbdad36`, `c081515d`, `c090dae6` — touch only documentation and
`docs/decisions/`.

**The tempting conclusion is that they are safe. This project has recorded that
exact conclusion being wrong.** `CLAUDE.md` carries it as a catalogue row: a
docs-only change failed under precisely two feature sets, which is why the recorded
feature-set count was corrected from three to five. Several guards in this package
**read documentation from disk**:

| guard | what it reads |
|---|---|
| `handoff_figures.rs` | the handoff's state table, which I edited |
| `comment_citations.rs` | comments in the test sources, which I edited |
| `outstanding_reports.rs` | `REVERSE_PROMPT.md` and two test files it cites |
| `prediction_stamp.rs` | the newest `ABSORPTION_<N>_BRIEF.md` |
| `test_population_guard.rs` | the test file and function counts |

So a docs-only commit can turn this suite red, and four of them are stacked
unverified. **That is the first thing to measure, and it is cheap.**

## The order, cheapest and most informative first

1. **The record guards alone**, default features. Seconds. They are the ones most
   likely to have been broken by four documentation commits, and they are the
   cheapest signal available.
2. **`cargo fmt --check`, `clippy --all-targets -D warnings`, both configurations.**
   Already green without linking, so this only confirms nothing regressed.
3. **The full gate, default features, script end to end.** Twenty-five minutes.
4. **The full gate, `--narrow`.** One invocation covers ONE configuration.
5. **Only then push**, and verify by `ls-remote` rather than by the hook's own
   "all checks passed" line, which has accompanied a silently unlanded push five
   times on this project.

## Wrong turns, named

1. **Do not skip step 1 because steps 3 and 4 subsume it.** They do subsume it, but
   twenty-five minutes later. A red record guard found in seconds is the difference
   between one gate cycle and two.
2. **Do not assume the previously green gate still applies.** `51542c99` was gated
   before four commits landed on top of it. A gate result belongs to the tree it
   ran against.
3. **Do not treat `cargo check` and `clippy` as a gate.** They ran no test. Any
   claim resting on them must say so.
4. **Do not batch further work into the same push** until the four stacked commits
   are verified. If something is red, a fifth change makes attribution an argument
   rather than an observation — which is the whole reason this line measures
   absorptions alone.

## Pre-commitment

- **If step 1 is red, fix it before running the full gate.** Spending fifty minutes
  to rediscover a failure the record guards report in seconds is the waste this
  plan exists to prevent.
- **If everything is green, say plainly that the docs-only worry did not
  materialise** rather than quietly moving on. A precaution that was unnecessary is
  still worth recording, because the catalogue row that motivated it came from a
  case where it was necessary.
