# ABSORPTION 72 — the fourth vacuous ownership check

**Computed against** `1c5bcd3f9f82603dce94b82c253a3d7a9451d013`, with **2 unabsorbed** commits on `origin/v0.2.3`
(`71c9afb9`, `fc4081fb`). Figures derived at that tree.

## What is arriving

WASM coverage for the last exported item, with the failure class named beside its table:
`keleusma-wasm/{src/lib.rs,Cargo.lock}`, `CLAUDE.md`, and their journal and task log.

## Predictions

1. **The merge is clean**, including `CLAUDE.md`, which both lines edited this session —
   theirs in the status prose, this line's in the green-run catalogue table.
2. **Zero reach paths touched**, so `gate-status.sh` will name nothing from the absorption
   and the backend verdict is unaffected. **This is consistent with the CONDITIONAL
   prediction** now recorded — "*if* reach flips, the likeliest members are
   `REVERSE_PROMPT.md` or `docs/decisions`" — which permits no flip. The earlier
   unconditional phrasing would have been wrong a second time.
3. ⚠ **The ownership check is vacuous for the FOURTH absorption running** (70, 71, 72, and
   66-68 before 69). Absorption 69 remains the only firing in seven.
4. **No gate run is warranted**: no `native_codegen/` source arrives and no reach path
   moves, so the verdict at `a519d64d` still speaks to the package.

## Outcome

**Prediction 1 was WRONG. The merge conflicted, in `CLAUDE.md`.**

⚠ **And the refutation was printed by the same command that filed this brief.** That command
began with `git merge-tree --write-tree`, which reported `conflicts: YES`, and the brief was
composed in the same invocation claiming "The merge is clean". **This is absorption 59's
failure, reproduced exactly** — its brief predicted zero conflicting files while `merge-tree`
computed one, in output printed by the same command, and `prediction_stamp.rs` exists because
of it. That guard requires a prediction to record the TREE it was computed against; it cannot
require the author to read the output already on the screen.

The stamp discipline worked and was not enough. **Knowing a failure class does not prevent
producing it** — a sentence this project's own catalogue already carries, now with one more
instance behind it.

Predictions 2, 3 and 4 held: zero reach paths touched, the ownership check vacuous a fourth
consecutive time, and no gate run warranted.

**What would have caught it:** composing the brief AFTER reading the conflict check, not
alongside it. The cheap form is to run the check in its own command.
