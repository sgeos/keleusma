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
| `v0.2.3` | PRs #105, #106, #107 merged green; nothing of this line is open |
| The crashed session's work | recovered intact and landed, not rebuilt |
| The three remaining host models | checked against independent sources, two findings pinned |
| The reported `break` discrepancy | answered and closed; it was never about `break` |
| Construct-support boundary | 79 Ok / 4 Gap / 1 RefRejects, 84 cases, recounted |
| Housekeeping | one stale branch pruned after verification; worktrees settled |

**This is a stopping point.** The tree is clean, no pull request of this line is open, the handoff
validates by ancestry and content against the current tip, and the three open questions below are
the operator's rather than blockers on my side.

## B1 is done, and it was a third the size the plan stated

**`wire_names_via_kel` takes a `Module` and builds its own input.** It previously accepted a
pre-built blob and opened with `let _ = module;`, while the only producer of that blob was a Rust
function in the test harness. Byte identity against the reference is unchanged; 163 wire tests,
1242 library tests, 133 codegen tests green.

**Three of the plan's four remaining items were already done**, established by reading the code
rather than the plan: `wire.kel` was already in `read_stage`, the interning-sequence producer was
already self-hosted in `wire.kel`, and the residency staging was never needed. What was missing was
the ENCODER.

**The staging coupling came from the 395,804 figure.** The plan says the producer and the staging
"are the same increment, and doing either alone is wasted". Measured: the worst stage, `parse`,
interns 627 names from a 33,395-byte blob against caps of 1024 and 49,152, so **nothing in the
corpus needs staging**. That figure is a `CONSTS` region record count and still sits at five sites
in the plan. A wrong figure did not merely misstate a size, it invented a dependency between two
pieces of work.

**The name count was wrong in the unsafe direction.** The caller passed
`interner_input(&module).len()`, which omits the data-slot contributor: 252 for `parse` where the
module interns 627. Its only consumer is the cap check that exists to refuse a module which would
overrun the interner, so an under-count defeats the guard. The blob and the count now come from one
walk.

**Adding coverage found the real semantics.** Asserting the count EQUALS the reference's `NAMES`
record count passed on all ten stages; a named-constant case then failed, 9 against 4. The reference
dedups and `intern_fresh` records its entry so a later `intern` can share it, making the exact count
order-dependent. Reproducing it host-side would be a second model of the thing under test, so the
value is an explicit upper bound with soundness asserted and the looseness pinned. **Equality on ten
stages was a corpus property I was one test away from recording as a guarantee.**

**What these green suites do NOT establish**: that the bound is tight for arbitrary modules (it is
not, and a case proves it), and that the constant-name branch matters to any stage (it does not —
dropping it leaves all ten green, which is why the named-constant cases exist).

## RETRACTED: the doc-coverage gap I reported does not exist

**I reported that CI never doc-builds the `self-host` feature surface. That is false, and the
error was mine.** The Doc job already carries a step named "keleusma (self-host feature surface)"
running `cargo doc -p keleusma --no-deps --features signatures,encryption,shell,self-host`, which is
the exact command I independently derived as the fix. It ran on PR #114 and passed, so the ~200
lines of doc comments B1 added to `src/selfhost/` were checked all along.

**How I got it wrong**: I read the FIRST step of the Doc job, saw the docs.rs feature set, and
reported the job's coverage from it without reading the remaining steps. The comment immediately
above the step I missed says this job "lists crates BY NAME, so a new crate is invisible to it until
someone remembers", and records that broken intra-doc links in `src/selfhost/` once survived four
releases — so the gap was found and closed before I claimed it was open.

**Two figures in that report were also wrong.** It is three unresolved links, not four; the fourth
line I counted was rustdoc's aggregate `could not document`. And they are not a defect: they resolve
under every feature set the project actually documents, and fail only under `--features self-host`
alone, which neither docs.rs nor CI builds. Fixing them would mean de-linking or duplicating prose
under `cfg_attr` to serve a configuration nobody ships.

**The lesson is the one already written down**: check an item against the code before repeating it.
I made that error while writing a finding that a goal statement then carried forward.

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
