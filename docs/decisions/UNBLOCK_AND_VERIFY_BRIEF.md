# BRIEF — absorb the fix, verify green independently, publish the backlog

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Absorption 25, which carries the upstream fix | yes |
| 2 | **Verify the workspace suite is green by running it**, not by inference | yes |
| 3 | Publish the nine held commits once the gate genuinely passes | yes |
| 4 | Retire the known-red record only after it is observed clear | yes |
| 5 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

PR #314 has merged. It fixes the branch-dependent pin that made this line's workspace suite red and
its pre-push gate refuse nine commits.

**The temptation this increment must resist is the whole point of it.** The peer stated plainly that
they make no claim about whether their fix cleared my only red — *"I have not seen your suite and
will not infer a green from a single fix"*. That is the correct epistemics and it applies to me
harder: I measured exactly one failure, but a suite I have not re-run after absorbing seven commits
is a suite whose state I do not know.

**A green I inferred is worth nothing here**, particularly after an increment in which a test named
for a property it did not establish passed for weeks.

## Prior failures to avoid repeating

1. **A test named for a canary firing did not make it fire.** Do not let a record say "green" on the
   strength of an upstream claim.
2. **A gate run killed by a signal reported "51 passed, 9 binaries"** — a plausible small number that
   was not a result. **Read the exit status, not just the counts.**
3. **A workspace run hit the 10-minute tool cap and returned 143.** Background the long suite.
4. **A census surveys a population**; say which one.
5. **Eight recorded premises have been found false in consecutive increments**, several of them this
   line's own guesses. Predictions are cheap only because they are written down first.

## Specific wrong turns to avoid

- **Do not push before the suite is observed green.** The gate will refuse anyway, and attempting it
  to "see" is a four-minute way of asking a question the suite answers directly.
- **Do not use `--no-verify`, even now.** If something else is red, that is a finding.
- **Do not retire the known-red section on the merge alone.** Retire it when a run shows zero
  failures, and say which run.
- **Do not report the absorption's prediction as hit unless both figures match.** The workspace count
  moves for two reasons here — new tests and a previously failing test now passing — so state the
  arithmetic rather than a single number.
- **Do not skip the ownership check** because the incoming change is a fix this line asked for. It is
  still someone else's commit landing in shared trees.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
