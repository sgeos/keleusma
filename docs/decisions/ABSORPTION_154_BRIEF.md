# Absorption 154

> **Navigation** [Decisions](./README.md) | [Absorption 153](./ABSORPTION_153_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against upstream `9ef59fc6` on 2026-11-22, with 0 unabsorbed commits
measured after fetching. The merge was made on `docs/v3-resume-20261118`, after
absorption 153, before the branch's first push, which the operator authorized
on 2026-11-22.

- **779** is the proofs line's work.
- **791** fixes the arena's relative bottom mark.
- **792** adds the tightness runner, and **797** is Kaizen's tightness metrics
  in CI.
- **793** is V0.2.X's handoff.
- **794** is ring increment R1. It re-pins
  `native_codegen/tests/corpus_fingerprint.rs` for `parse.kel` and
  `lexer.kel`, which this line reviewed in sequence 580 by recomputing both
  pins.
- **795** is a Dependabot update to the fuzz package.
- **796** is Kaizen's removal of the redundant `package.homepage`, which
  answers the manifest warning this line had noted.

## Merge and effect

No file conflicted, and `CLAUDE.md` keeps this line's routing paragraph. The
only native file that changed is the corpus fingerprint pin of 794. The
checkpoint was scanned with this line's disclosure-term scan, and none of its
565 messages matches a term.

## Verification

| Check | Result |
|---|---|
| Native `corpus_fingerprint` and `handoff_figures` | 9 passed |
| Root `documentation_links` | 2 passed, 0 failed |

The full native suites last ran at `70ec4613`, before this absorption. 794's
own hosted run passed both native gate jobs before it merged, and the hosted run
on this branch's pull request covers the combination.

## Absorption 155 and the checkpoint repair

Pull request 798 failed `Shell scripts and Python tooling`, in two
`test_interline` installation tests, with "duplicate message or unregistered
checkpoint route". Absorption 153's textual merge of
`docs/process/interline/checkpoint.json` had kept two copies of message
`v030-20261101-main-into-v030`, because this line's side and Kaizen's export
held it at different positions. The next merge, absorption 155, brings in
upstream 800, Kaizen's alert record, through `4a222da3`, and takes `v0.2.3`'s
checkpoint verbatim, which holds the message once. The scan finds no term in
its 564 messages, and all 215 script tests pass, the interline tests among
them.

**A checkpoint is not merged as text.** On a conflict or an absorption, take
one exported side whole and confirm with `read_checkpoint` or the script
tests, since a clean textual merge of two exports can still duplicate a
message.
