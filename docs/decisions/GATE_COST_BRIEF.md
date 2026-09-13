# BRIEF — the gate's cost, and a sixth stale record

## What I set out to do, and why that was wrong

The gate now takes six commands and roughly twenty-five minutes, and one test —
`how_deep_does_the_undetected_set_go` — accounts for 419s under default features
and 429s under narrow. It was killed once at >360s under contention and passed on
re-run, so **machine load decides whether it completes**, which is not a stable
measurement regime.

The obvious move was to make it cheaper. **Investigating first showed that would
have been a mistake**, and would have undone deliberate work.

## The finding

The test's own doc comment says:

> *"⚠ **OPT-IN: this does NOT run in the everyday gate**. Run it with
> `--ignored`. **Measured at 710s alone** … against a threshold of 600s … **It is
> over, so it stays opt-in** … **Its last green result is a dated measurement, not
> a standing guarantee.**"*

**Every one of those statements is false.** The test carries `#[test]` and no
`#[ignore]`. Commit `e55f307e` (2026-08-29) restored it to the gate on purpose,
and its message is explicit: the deep sweep had been paying for the census's
variant axis twice; removing the duplicated axis took it from 712s to 401s, under
the threshold, and *"both sweeps now run on every gate … so breadth and depth of
mutation sensitivity are protected again."*

The commit also records that the saving *"came from removing a duplicated axis
rather than trading coverage"*, and that site depth was **not** reduced and the
mutation family **not** narrowed.

So this is a **sixth record outliving its subject** — and the most expensive yet,
because it misdescribes the gate's cost structure to anyone deciding what to do
about it. I nearly optimised a cost that had already been optimised by the one
method that does not trade reach.

## What to do

1. Correct the doc comment to what is true: it runs every gate, at a re-derived
   cost, and its green result **is** a standing guarantee.
2. Record today's measurements, taken under contention, with the machine state
   noted — the earlier figures name their load average, and a cost figure without
   one is not comparable.
3. Record the **new** fact the earlier increment could not know: the test is under
   its own 600s threshold but close to the *harness's* 600s ceiling for a single
   command, and that ceiling is what killed a run. These are different limits that
   happen to share a number, and conflating them would be a mistake.
4. **Do not reduce site depth, narrow the mutation family, or re-ignore the test.**
   Those are the trades the previous increment explicitly declined.

## Wrong turns to avoid

- **Optimising before reading why the cost exists.** This is the second
  consecutive iteration where investigating first prevented the wrong action.
- **Treating a stale doc as merely untidy.** This one would have justified
  deleting coverage.
- **Quoting a duration without its load.** 710s at load 5 and 400s at load 8.2
  are both recorded with their conditions; a bare number is not comparable.
- **Claiming the gate is "too slow" as a finding.** It is a deliberate trade,
  recorded, and re-derived. What is new is the harness ceiling, not the cost.
