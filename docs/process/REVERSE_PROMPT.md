# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-15 (session 45, resumed after a system crash)

## Where things stand

| | |
|---|---|
| `v0.2.3` | PR #105 and PR #106 merged green; nothing of this line is open |
| The crashed session's work | recovered intact and landed, not rebuilt |
| The three remaining host models | checked against independent sources, two findings pinned |
| The reported `break` discrepancy | answered and closed; it was never about `break` |
| Construct-support boundary | 79 Ok / 4 Gap / 1 RefRejects, 84 cases, recounted |

## The crash cost the push, not the work

The previous session had committed a complete increment to a local feature branch and never pushed
it. Working tree clean, no stash, all four channels updated in the same commit. **Nothing needed
rebuilding.** What it cost was the push, the pull request, and an accurate handoff.

**`HANDOFF.md` reported itself STALE, correctly, and for the wrong reason.** Its validity check
required `git rev-parse HEAD~1` to equal a recorded parent, so the first unrelated merge invalidated
it while its contents were still largely true. Three merges had landed. The stamp is now an
**ancestor check plus a content check**, which is what the `v0.3.0` line moved to after hitting the
identical defect. A hash match is a claim that nothing else ever lands.

It also carried a stale `selfhost_wire` count, 157 against the tree's 161. The rewritten file
**derives** such numbers with a command rather than restating them.

## The `break` report: the grammar is right, the parser is right

The `v0.3.0` line reported that `GRAMMAR.md` documents a `break;` form the parser rejects, and left
`BreakIf` unisolated in its opcode audit on that basis. **Both halves are wrong.**

The documented form parses verbatim. `TokenKind::Break` is handled at statement position in
`parse_block`, so there is no route from that form to an expression-position diagnostic at all.

**The real cause is a stray `;` after a `for` block** in their probe source. A `for` loop is a
statement and consumes no trailing semicolon, so the parser resumes at statement position and reads
the `;` as the start of an expression. The diagnostic, `unexpected token Semicolon in expression`,
names the semicolon, and their source has two near each other.

**The control settles it rather than my reasoning**: remove `break` entirely, keep the stray
semicolon, and the failure is identical.

**`BreakIf` is reachable.** One semicolon deleted, nothing else changed, and `main` carries
`BreakIf(41)` and `Break(41)`. Their probe source is now a named case pinned by execution.

**Pinned, not repaired.** `if`, `match`, and `loop` accept a trailing semicolon and `for` does not.
Accepting it widens the admitted language, which is a judgment call rather than a correctness fix.
`GRAMMAR.md` gains the rule it was silent on, and all three accepting forms are pinned, not
generalised from `if`.

## A claim of mine that needed checking before it shipped

The grammar sentence names `if`, `match`, and `loop`. I had measured only `if`. I checked the other
two before the merge rather than after, and both hold — but the sentence would have been a
three-part claim resting on one measurement. **The same class as everything else on this list.**

## Open

- **The `analyze_class` catch-all is the highest-value open correctness item.** It ends in
  `_ => (0, 0)`, so a control-flow opcode added later and not classified becomes "plain" silently: a
  graph missing an edge and a bound that is finite and wrong. The boundary is pinned at nine classes
  but the hole is not closed. Closing it needs an exhaustive `match` over `Op` so the compiler
  refuses a new opcode until it is classified. **This is my proposed next increment.**
- **`Op::cost()` disagrees with measurement**, two findings pinned rather than repaired. Only 17
  opcodes of 66 were ever measured; every other emitted value is a bucket assignment checked by
  nothing.
- **The `for` trailing-semicolon asymmetry**, pinned. Widening is the operator's call.
- **`-255` is live and has no negative test**; the corpus tops out at 7,680 distinct name bytes.
- **`bin` was raised, not fixed.** 49,152 covers `parse` at 1.47x.
- **Two pinned coverage gaps**: no stage contributes a constant-interned name, and none nests a
  constant past depth one.
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- Publication remains **HELD**.

## Questions for the operator

1. **The `analyze_class` catch-all.** Closing it is mechanical and changes a `match`, not a bound,
   but it will refuse to compile until every `Op` is classified, which is the point. Proceed?
2. **The `for` trailing-semicolon asymmetry.** Accept a trailing semicolon after `for`, matching the
   other three block forms, or leave the asymmetry pinned as it stands?
3. **`Op::cost()`.** The two findings are pinned, not repaired. Recalibrating is a judgment call I
   have deliberately not taken.
