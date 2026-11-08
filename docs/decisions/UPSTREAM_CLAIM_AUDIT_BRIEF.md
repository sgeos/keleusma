# BRIEF — the claims this line makes about the other line's code

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## Why this subset, and why it is the dangerous one

Two increments ago, `NATIVE_BOUNDS_TRANSFER.md` was found asserting that the reference's worst-case
memory bound is **unsound**, four weeks after the `v0.2.3` line repaired it. Then `REVERSE_PROMPT.md`
was found claiming "none acted on" from recollection.

**A claim about your own code that goes stale is an embarrassment. A claim about someone else's code
that goes stale is an accusation.** This audit takes only that subset.

## The scope, measured rather than guessed

252 documents under `docs/decisions/`. 103 mention the other line's code — far too broad to mean
anything. **19 contain an unrepaired-defect phrase**, and reading the matching lines narrows it to
**three live accusations**, the rest being this line's own reasoning about soundness in general:

| document | claim |
|---|---|
| `NATIVE_COMPOSITE_RETURN_ABI.md` | "Reported and pinned, not repaired", with `composite_return_aliasing.rs` carrying the failing case |
| `ORDER_1_VERIFY_TYPES_BRIEF.md` | a defect in `src/selfhost/kel/`, "REPORTED, not repaired" |
| `INVALID_BYTECODE_CENSUS.md` | a table column headed "unrepaired" |

**The hand narrowing is itself suspect** — the host-buffer audit's hand-written population was wrong
twice in one increment — so the extraction above is from a matcher and the triage is recorded with it,
not instead of it.

## The wrong turns

1. **Do not treat a self-verifying claim as unchecked.** If a document's claim is carried by a test
   that fails when the defect is repaired, it is already watched, and the audit's job is to confirm
   that rather than to re-measure by hand.
2. **Do not retract on a passing suite.** A test that does not exist proves nothing; a guard that
   passes while asserting the defect still fires proves the defect is open. Only the second is
   evidence.
3. **Do not widen to every soundness word in the corpus.** Most matches are this line reasoning about
   soundness as a property, not accusing anyone. A sweep that cannot tell those apart produces a list
   nobody will act on.
4. **Do not edit `src/` or `tests/` at the repository root.** If a claim turns out still true, it
   stays reported. This line audits its own record, not their code.
5. **Do not leave a confirmed-open claim unwatched.** The point of the previous increment was that
   being right by recollection expires. A claim confirmed by hand today and guarded by nothing is the
   same exposure a week from now.

## What done looks like

Each of the three live accusations is either confirmed open with evidence and watched by something
that fails when it is repaired, or retracted with its date and its reason; the triage from 19 to 3 is
recorded so a later reader can challenge it; and any claim left unwatched is named as such rather than
quietly trusted.

---

## OUTCOME — 2026-09-12

**Three candidates, three different answers, and not one of them was "still open as written".**

| claim | outcome |
|---|---|
| `NATIVE_COMPOSITE_RETURN_ABI.md` — "reported and pinned, not repaired" | **STALE.** Repaired 2026-08-14 by THIS line. The test says so in its own text and carries no ignored cases; each call site receives a disjoint block of the caller's region. |
| `ORDER_1_VERIFY_TYPES_BRIEF.md` finding 1 — `cmd` declared, documented, never read | **ACTED ON by the `v0.2.3` line.** Their header now states it plainly and explains why the slot is kept: it sits at slot 0 and removing it would shift the seeding. |
| `ORDER_1_VERIFY_TYPES_BRIEF.md` finding 2 — `ty_max_steps()` is 1801 against a 60-tick drive | **STILL TRUE**, measured: the function still sums to 1801. |
| `INVALID_BYTECODE_CENSUS.md` — a column headed "unrepaired" | **FALSE POSITIVE.** It compares a suite before and after a repair. The matcher cannot tell a status claim from a table heading. |

### The finding is that the register was wrong in BOTH directions

The previous increment found a report outliving its defect. This one found that **and its mirror**: the
other line fixed something this line reported, and nothing here recorded it. A register that is stale
only in the accusing direction is a bias; stale in both is simply unmaintained — which is the more
accurate and less flattering diagnosis.

> **Confirming a repair deserves the same diligence as reporting a defect.** One increment was spent
> learning that an un-retracted report becomes an accusation. The unacknowledged fix is the same
> failure with the sign flipped.

### One repair was this line's own

`NATIVE_COMPOSITE_RETURN_ABI.md` is not an accusation against anyone — it describes a defect in this
backend, repaired here, still recorded as open. **The class is not "claims about others"; it is
"status recorded once and never revisited".** The brief's own framing was narrower than the problem.

### What is NOT closed

Finding 2 stands and is now the only live item among the three. It is **unwatched**: nothing fails if
`ty_max_steps()` changes or if the drive length is raised. Named here rather than left implicit, and
left unwatched deliberately — it is a design constraint on subject sizing rather than a defect, and a
guard would assert a number this line does not own.
