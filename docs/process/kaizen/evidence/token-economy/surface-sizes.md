# Instruction-surface sizes on origin/v0.2.3

Bytes of each file at the last commit on or before each date. Regenerate with the loop at the end.

| date | commit | CLAUDE.md | HANDOFF.md | TASKLOG.md | REVERSE_PROMPT.md | DESIGN_JOURNAL.md |
|---|---|---|---|---|---|---|
| 2026-06-01 | `b652249a` | 12025 | 0 | 118981 | 5357 | 0 |
| 2026-07-01 | `de7b1d55` | 13161 | 0 | 204946 | 126853 | 0 |
| 2026-08-01 | `fc0c1b97` | 23633 | 9320 | 206245 | 5109 | 404594 |
| 2026-09-01 | `27fcbd11` | 26773 | 88036 | 425196 | 8247 | 1023872 |
| 2026-10-01 | `563cc273` | 40702 | 140195 | 537883 | 3248 | 1371861 |
| 2026-10-24 | `be81c214` | 41275 | 153546 | 692257 | 15912 | 1490218 |

On origin/v0.3.0 at `9dba0d2f`, the same files measure:

| CLAUDE.md | HANDOFF.md | TASKLOG.md | REVERSE_PROMPT.md | DESIGN_JOURNAL.md | handoffs/v0.3.0.md |
|---|---|---|---|---|---|
| 45510 | 149247 | 703587 | 60534 | 1919351 | 768330 |

The status paragraph of CLAUDE.md on v0.2.3 is 10021 characters long.

Per-prompt hook banners measured in this session: two `UserPromptSubmit` hooks fire, one from the worktree local settings naming `kaizen` and one from the primary checkout local settings naming `v0.2.3`.

```bash
for d in 2026-06-01 2026-07-01 2026-08-01 2026-09-01 2026-10-01 2026-10-24; do
  c=$(git rev-list -1 --before="$d 23:59" be81c214)
  for f in CLAUDE.md docs/process/HANDOFF.md docs/process/TASKLOG.md docs/process/REVERSE_PROMPT.md docs/process/DESIGN_JOURNAL.md; do
    printf "%s %s %s\n" "$d" "$f" "$(git show $c:$f 2>/dev/null | wc -c)"
  done
done
```
