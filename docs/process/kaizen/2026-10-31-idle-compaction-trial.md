# The Idle Compaction Trial, 2026-10-31

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

Next increment 2 of the kaizen state file asked that `context_quality.py` be extended
for a trial of `idleCompaction: false`, comparing error rates and cache writes after
long gaps with the idle compactions already measured. The operator adopted the line's
next increments as obvious choices on 2026-10-31. This report gives the extension and
its first reading.

## The trial setting is absent

**Fact.** On 2026-10-31 no `idleCompaction` key is present in the operator's global
`settings.json`, in its environment block, in the global configuration file or in the
project's settings files. **Fact.** By the output below, an idle compaction occurred
on every day from 2026-10-20 to 2026-10-31 that had a resumption, except 2026-10-27,
and two occurred on 2026-10-31. **Inference.** The trial has not run. Either the
setting was never applied or it was removed. The transcripts alone cannot tell which. This line cannot
apply it, because the sandbox denies writes to the settings files, and whether to apply
it is the operator's decision.

The existing transcripts still allow the comparison the trial was meant to make,
because some resumptions after a long gap were not compacted.

## The extension

`context_quality.py` now records each resumption after a gap of at least 30 minutes
between activity records. A resumption opens at the first activity or compaction after
the gap and closes at the first assistant call with usage. It is marked compacted when
a compaction summary falls inside it. For each it records the error rate of the next 50
tool calls, without the read-before-edit guard that compaction trips, and the cache
tokens the first assistant call wrote and read. It reports three groups and a daily
count. The output is
[`context-quality-2026-10-31.json`](./evidence/token-economy/context-quality-2026-10-31.json).
The older output `context-quality.json` is kept unchanged, because the report of
2026-10-24 cites its figures.

## The first reading

All `keleusma` main-conversation transcripts on this machine, read on 2026-10-31, 373
resumptions over 39 days.

| Resumption | Count | Calls after | Error rate | First call cache write, median | First call cache read, median |
|---|---|---|---|---|---|
| Compacted | 37 | 1,801 | 2.00% | 51,357 | 28,910 |
| Uncompacted, gap under 60 minutes | 211 | 10,311 | 1.18% | 576 | 361,651 |
| Uncompacted, gap of 60 minutes or more | 125 | 6,009 | 1.21% | 345,545 | 26,351 |

**Inference.** An uncompacted resumption past the one-hour cache lifetime is what
`idleCompaction: false` would turn an idle compaction into. Against it, a compacted
resumption shows about 0.8 points more tool errors in its next 50 calls and writes
about 294,000 fewer cache tokens on its first call. Over the 37 compacted resumptions
that is roughly 11 million cache-write tokens in 39 days, a bound on what the setting
would have cost.

**Limits.** A tool error is a weak proxy for quality, as the tool's own header states.
The calls cluster in 37 compacted resumptions, so the difference is suggestive and not
established. A naive test that treats calls as independent gives about p = 0.01, which
overstates the evidence. The groups differ in session, task and model. The compaction
request itself does not appear in the transcript, so the compacted group's cost is
understated by that call. The gap threshold of 30 minutes and the cache lifetime of 60
minutes are assumptions in the script.

## What would settle it

Apply `idleCompaction: false`, globally or for one line, then run the script again
after about two weeks. The compacted group should stop growing on the days after the
change, which confirms the setting took effect. The uncompacted group past the cache
lifetime then shows the cost and error rate directly.
