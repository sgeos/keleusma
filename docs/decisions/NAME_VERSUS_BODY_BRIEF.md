# BRIEF — audit the names that make the strongest claims

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Check every strongly-quantified test name on this line against what its body proves | yes |
| 2 | Correct the ones that overclaim, and report the RATE rather than anecdotes | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

**Three increments running, the finding has been a name asserting more than its body checks**: a test
named for a canary firing whose body never made it fire; a guard whose watched population was
narrower than its subject, three times over; and a measured property called *"yields a composite"*
that tested co-occurrence.

That is enough recurrence to stop meeting it by accident. **The highest-risk names are the ones making
universal or negative claims** — "every", "no", "all", "never", "only", "each" — because a body
weaker than the name is most misleading exactly there. A reader consults `every_X_is_Y` precisely to
learn whether every X is Y.

**Measured: 11 such names among 325 tests.** Small enough to read all of them, which turns "I keep
finding these" into a rate.

**One is already suspected.** `each_float_conversion_is_refused_by_name` was written this session, and
the journal entry beside it records that only `IntToFloat` is named in the refusal while `FloatToInt`
is *unreached behind it*. The name says both are refused by name; the body cannot show that.

## Prior failures to avoid repeating

1. **A test named for a canary firing did not make it fire.**
2. **A guard's population narrower than its subject**, three times at three granularities.
3. **A property name asserting more than its body checked**, in the instrument built to fix the
   previous defect.
4. **An edit that silently did nothing** because its assertion was omitted.
5. **Fourteen recorded premises found false or overstated in consecutive increments.** Write the
   prediction: at least two of the eleven overclaim.

## Specific wrong turns to avoid

- **Do not fix an overclaiming name by weakening it when the strong claim is the one worth having.**
  Sometimes the body should be strengthened instead, and choosing the cheaper repair silently reduces
  what the suite proves.
- **Do not count a name as sound because the test passes.** Passing says the body holds, not that the
  body matches the name — that is the entire defect.
- **Do not audit only the tests written this session.** Selection by attention has already been caught
  once; the set is defined by the name pattern, over all files.
- **Do not report "several" or "a few".** The deliverable is *n of 11*, because a rate is what
  distinguishes a systemic habit from a run of bad luck.
- **Do not extend the pattern list to catch more names mid-audit.** Fix the set first, report against
  it, and note what a wider pattern would have added.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
