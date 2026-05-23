# Review of `docs/tmp/guide/`

Staged from `tmp/guide/` on 2026-05-22. The material was drafted by another session and is offered as a candidate replacement for `docs/guide/`. This note records the comparison against the current tracked content so the operator can decide whether to promote.

## Headline

The staged material adds a forty-chapter linear learning course that does not exist in the current `docs/guide/`. It also brings the existing reference pages along, but the reference pages are mostly bit-identical to the tracked versions. Two reference pages and one omission category require attention before promotion.

## Inventory

The staged tree carries fifty-two files.

- `OUTLINE.md` and `README.md`. Planning document plus the new combined index.
- `01_*.md` through `40_*.md`. Forty new chapter files arranged as ten parts. Net-new content; nothing in `docs/guide/` corresponds.
- Reference pages: `BIG_NUMBERS.md`, `COOKBOOK.md`, `EMBEDDING.md`, `FAQ.md`, `GETTING_STARTED.md`, `LLM_USAGE.md`, `PIANO_ROLL.md`, `ROGUE.md`, `SECURITY_POLICY.md`, `WHY_REJECTED.md`. Same filenames as the tracked versions.

## Comparison against the tracked `docs/guide/`

| File | Status |
|------|--------|
| `BIG_NUMBERS.md` | Bit-identical |
| `COOKBOOK.md` | Bit-identical |
| `EMBEDDING.md` | Bit-identical |
| `FAQ.md` | Bit-identical |
| `GETTING_STARTED.md` | Bit-identical |
| `LLM_USAGE.md` | Bit-identical |
| `PIANO_ROLL.md` | Bit-identical |
| `ROGUE.md` | Bit-identical |
| `WHY_REJECTED.md` | Bit-identical |
| `README.md` | Differs. Staged version is the combined course-plus-reference index. Tracked version is the existing reference-only sequence table |
| `SECURITY_POLICY.md` | Differs. Staged version is missing the "Daemon deployments and tick-interval cadences" section that was added in commit `93a2dbf` (this session) covering fail-fast configuration, memory residency as a feature, and the cron-or-noop-cycles pattern for cadences longer than four weeks |
| `METRICS.md` | Not present in staged. Tracked version was added in commit `aa5381b` and updated in this session's commit `448146c` with the "Steady-state at sleep cadence" subsection |
| `SHELL_AUDIT.md` | Not present in staged. Tracked version was created in commit `aa5381b` and substantially revised in commit `93a2dbf` to reflect the closed gaps and the V0.2.1 shell-audit critical natives |

## Concerns to address before promotion

1. **`SECURITY_POLICY.md` regression**. The staged version pre-dates the tick-interval daemon documentation. Promoting as-is would drop that material. Either merge the staged changes into the tracked version, or graft the "Daemon deployments and tick-interval cadences" section onto the staged version.

2. **`README.md` divergence**. The staged version reframes the guide as a forty-chapter course plus reference pages. The tracked version is the reference-only sequence table. Promoting the staged version is a deliberate restructure that ought to be confirmed.

3. **Missing `METRICS.md` and `SHELL_AUDIT.md`**. Both were added in V0.2.1 work and are not mentioned in the staged `README.md` index. Either reincorporate them and update the index, or document a deliberate decision to retire them.

4. **Course-versus-reference duplication**. The course chapters cover the same ground as the reference pages at a learner-level depth. The staged `OUTLINE.md` section 5 calls this out explicitly as a strict-superset arrangement. The implication for maintenance is that an update to a reference page may need a parallel update to one or more course chapters. The staged `README.md` does not state a single-source-of-truth policy.

5. **Code examples in the new chapters need verification**. The staged chapters were drafted against the V0.2.0 baseline. Anything that uses V0.2.1 surface (signed bytecode flow, encryption, tick-interval rate limiting, new shell natives) is either missing or based on superseded behaviour. Sampling one or two later chapters before promotion is prudent.

## Recommended next steps

1. Operator reviews this note and decides whether the forty-chapter course is wanted as a permanent fixture or whether the reference-only structure should remain.
2. If the course is wanted, resolve the three discrete items above (graft the SECURITY_POLICY section, decide on METRICS and SHELL_AUDIT, declare a maintenance policy in the README) before moving `docs/tmp/guide/` to `docs/guide/`.
3. If the course is not wanted, the staged material can be deleted with no impact on the tracked documentation.
