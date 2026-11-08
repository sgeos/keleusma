# BRIEF — absorption 4, and it lands in the middle of the work it can move

## The goal

`origin/v0.2.3` is at **`2fff9d0b`**, eight commits ahead of the absorption point `abc4bac2`. Four are
merges; the substance is three commits touching the **CONSTS region and the assembled artifact**:

- `a1a0815d` gives the emitted constant set one definition and corrects the figures it made stale
- `7a788f65` emits CONSTS from Keleusma, byte-identically for every stage
- `c47ccb98` derives a region-coverage figure and routes CONSTS into the assembled artifact

## Why this one is not routine, and the timing matters

The last three increments of this line have been **seeding self-hosted stages and measuring the
Order-1 gate against them**. Those stages are `src/selfhost/kel/*.kel` — the very files whose emitted
artifact these commits change.

**So absorption 4 can move figures this line just published**, specifically:

- the seven seeded stages' shared-segment sizes and offsets, since slots are resolved BY NAME from
  the layout and a changed artifact changes the layout
- the Order-1 comparison count, currently **1680**
- the derived subjects for `parse.kel`, which come from running `lexer.kel`
- the `verify_types.kel` step budget, if the CONSTS change alters how many resumes a fold needs

**That is a reason to absorb NOW rather than later.** The longer this line measures against a stale
tree, the more figures have to be re-derived at once, and a large re-derivation is where a stale
number survives.

## Prior failures and the specific wrong turns to avoid

- **CHECK OWNERSHIP AGAINST THE ABSORPTION COMMIT, NEVER AGAINST `origin/v0.2.3`.** That ref MOVES.
  Diffing `src/` against the tip shows their unabsorbed work and reads as if this line had edited
  their files — which happened once already this session and cost a real moment of alarm. After this
  absorption the anchor becomes the NEW merge commit; update it wherever it is recorded.
- **GUARDS THAT FIRE ARE THE INSTRUMENT WORKING.** Absorption 3 fired three and all three were
  correct. **Commit the merge with them still failing**, then rewrite verdicts against the observed
  result. Pre-amending is writing the answer before measuring it.
- **NEVER DELETE AN ASSERTION TO MAKE A MERGE GREEN.** Invert it, and record what it used to say.
- **RE-DERIVE EVERY FIGURE, DO NOT SPOT-CHECK.** If the seeded stages' layouts moved, several
  numbers move together and a partially updated report is worse than a stale one, because a reader
  cannot tell which half to trust.
- **A DECLINED SUBJECT AFTER ABSORPTION IS A FINDING, NOT A THING TO DROP.** If a seed stops
  building or a subject stops reaching its verdict, that is news about the artifact change and must
  be reported, not silently reflected in a smaller count.
- **DO NOT EDIT THEIR FILES TO FIX A BREAK.** If absorption breaks a seed because a slot was renamed
  or removed, that is a report to them and a possible request, not an edit.
- **Watch the runtime.** The differential is at roughly 72 s and the full suite well past ten
  minutes. If the CONSTS change enlarges the stage artifacts, both grow.

## What a good outcome looks like

`origin/v0.2.3` at `2fff9d0b` is absorbed; every guard that fired is rewritten rather than removed,
against a measured result; every Order-1 figure in the resume document is re-derived by its own
command on the absorbed tree; and any seed or subject that stopped working is reported with its
cause named. **If nothing moved, that is stated as a measurement rather than assumed from a green
suite** — a suite passing does not by itself show the figures are unchanged.
