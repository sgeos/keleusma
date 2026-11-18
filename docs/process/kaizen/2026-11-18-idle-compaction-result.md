# The Idle Compaction Trial, Result, 2026-11-18

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

[The trial report of 2026-10-31](./2026-10-31-idle-compaction-trial.md) found the
`idleCompaction` setting absent and proposed applying `idleCompaction: false`, then measuring
again after about two weeks. The operator applied it in the global settings at 05:42 UTC on
2026-11-01, D2 of [the open decisions of 2026-11-01](./2026-11-01-open-decisions.md). This
record gives the second reading, taken on 2026-11-18.

## Method

`context_quality.py` gained `--since` and `--until`, which limit the idle-gap resumptions to a
range of days, so that the weeks before and after the change can be compared. Every other
figure it reports still covers all transcripts. It was run over every `keleusma`
main-conversation transcript on this machine, once for resumptions before 2026-11-01 and once
for resumptions from that day on. The outputs are
[`context-quality-2026-11-18-until-1101.json`](./evidence/token-economy/context-quality-2026-11-18-until-1101.json)
and
[`context-quality-2026-11-18-since-1101.json`](./evidence/token-economy/context-quality-2026-11-18-since-1101.json).

## Result

**Fact.** The setting took effect. From 2026-11-01 there were 2 compacted resumptions, both on
2026-11-01 itself, the day the setting was applied, and none after it, against 38 in the 39
days before.

| Resumption after an idle gap of 30 minutes or more | Period | Count | Error rate, next 50 calls | First-call cache write, median | First-call cache read, median |
|---|---|---|---|---|---|
| Compacted | before 2026-11-01 | 38 | 1.95% | 51,211 | 28,910 |
| Uncompacted, past the 60-minute cache lifetime | before 2026-11-01 | 118 | 1.22% | 339,218 | 0 |
| Uncompacted, past the cache lifetime | from 2026-11-01 | 113 | 0.90% | 654,349 | 26,956 |
| Uncompacted, within the cache lifetime | from 2026-11-01 | 42 | 0.93% | 720 | 457,275 |

**Inference.** Resuming without a compaction carried a lower tool-error rate than resuming
after one, 0.90 percent against 1.95 percent, and the earlier reading's 0.8-point difference
held in the same direction. The price is the cache write. A resumption past the cache lifetime
now rewrites a median of about 654,000 tokens, against about 51,000 after a compaction. If each
of the 113 had instead been compacted, about 68 million cache-write tokens would have been
saved over 17 days, an upper bound, since idle compaction triggers only above a context size
and not every one of the 113 would have qualified.

## Limits

- A tool error is a weak proxy for quality, as the script's header says, and the error counts
  are small: 36 errors after compacted resumptions before, 46 after uncompacted ones since.
- The two periods differ in work, sessions, models and context sizes. The larger median cache
  write since 2026-11-01 reflects larger contexts, which the change itself allows to persist,
  and also different tasks. The comparison is not controlled.
- The period from 2026-11-01 includes a machine restart, a macOS update and a pause of several
  days, which add long gaps unlike the working gaps of October.
- Days are as the script assigns them from transcript timestamps, so a resumption on
  2026-11-01 may fall on either side of the change at 05:42 UTC.

## Recommendation

Keep `idleCompaction: false`. On this evidence it trades cache cost for fewer tool errors after
a long gap, and the operator stated on 2026-10-24 that an improvement above the polishing level
is nominally worth its token cost. Revisit it if the cache cost begins to matter more, by the
same two-period run.
