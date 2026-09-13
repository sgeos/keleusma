# BRIEF — the shared channel files, and a fifth stale record

## The finding

Absorption 60 conflicted on `docs/process/REVERSE_PROMPT.md`. Investigating it
turned up a stale commitment in this line's own handoff:

> *"I touch **none** of `REVERSE_PROMPT.md`, `DESIGN_JOURNAL.md`, or
> `TASKLOG.md`."*

**This line has written to two of the three continuously.** Of the last twelve
commits touching `DESIGN_JOURNAL.md`, eight are this line's, interleaved with the
other line's. `REVERSE_PROMPT.md` carries this line's reports 4 and 5, written
this session.

That handoff catalogues four records that outlived their subjects and says an
un-retracted report becomes an accusation while an unacknowledged repair is the
same failure with the sign flipped. **This is a fifth, pointed inward: a
commitment recorded as kept while being broken every increment.**

## The two files behave differently, and only one has bitten

- **`REVERSE_PROMPT.md` — wholesale overwrite by both lines.** 106 lines against
  1988. This is what conflicted, and it conflicts every time both lines write
  between absorptions.
- **`DESIGN_JOURNAL.md` — prepend by both lines, at the same anchor.** It
  auto-merged at absorption 60 **by luck of position**, not by design. Two
  prepends at one anchor is the classic conflict shape; it has not bitten yet.

## What to build

1. **Correct the stale commitment.** State what this line actually does, so the
   next reader is not told a rule that has been broken for weeks.
2. **A guard on the resolution shape.** Absorption 60 was resolved by keeping the
   other line's document entire and appending this line's section. That shape is
   checkable: the file must contain this line's addendum marker **and** substantial
   content before it. If a future increment overwrites the file wholesale again,
   the guard fires rather than the next absorption discovering it.
3. **Record the journal hazard as predicted, not observed.** It has not conflicted.
   Saying it has would be the same error this session has now made twice with
   figures.

## Wrong turns to avoid

- **Keying the guard on the other line's prose.** Their section headings are
  theirs to change. Key on this line's own marker plus a structural property —
  content exists before the addendum — so their edits never fail this line's test.
- **Deciding where this line's reports belong.** That is the operator's call. The
  question is already asked in the file; asking it twice is not progress, and
  answering it unilaterally is worse.
- **Claiming the journal conflicts.** It has not. Predicted is not observed.
- **Deleting the reports to avoid the collision.** They are open questions the
  other line has not answered; removing them to make a merge easier loses the
  communication the file exists for.
