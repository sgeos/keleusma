# BRIEF — consume the confinement verdict to REFUSE, and to refuse only

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The ruling

The operator ruled on 2026-08-31 that the region planner's soundness obligation is discharged **by
analysis**; that an inconclusive verdict is *"a de facto decline"*; that the pre-drafted solution is
to be adopted rather than a new procedure written; and that the `V0.2.X` line owns the analysis while
this line consumes it. The corpus *"still needs to be useful and perform its intended purpose."*

## THE PLAN THIS BRIEF REPLACES WAS BUILT ON A MISREADING

The first plan was to consume the verdict to *decline in-place reuse*. **There is no site-to-site
reuse to decline.** Every static construction site already gets its own offset, `region_nonreuse.rs`
enforces it on ranges rather than offsets, and `region.rs` states plainly that nothing in it consults
liveness, escape, aliasing or confinement.

**The cross-iteration hazard is not site-to-site reuse.** It is that one site has ONE offset, so a
loop body writes the same bytes on every iteration. That is inherent in the placement.

**This is the conflation the handoff records this line making once already** — static-site
disjointness, which is true and enforced, against cross-iteration reuse, which is unconditional. It
was nearly repeated. Reading the consumer is what caught it, for the second time in this session.

## The two uses of the verdict, which must not be bundled

| use | buys | costs |
|---|---|---|
| **REFUSE a site the analysis says escapes its iteration** | closes the interprocedural gap the obligation names as still open, since `module_confinement` carries callee summaries and the existing check does not | nothing |
| Overlap confined sites so they share bytes | closes the measured arena-bound gap, 11 of 71 modules, 24 to 96 bytes each | **takes on an exposure that does not exist today**: a verdict wrong in the unsafe direction becomes a miscompile rather than a wasted byte |

**This increment takes the first row and only the first row.**

## Why that gets the soundness without the exposure

`region.rs` warns that whoever closes the gap is buying both halves. **That is true of the second row
and not of the first.** If the verdict is used only to ADD refusals on top of the existing
`yield_escape_hazards` check, and never to remove one, then a verdict wrong in the unsafe direction
merely fails to add a refusal and the lowering falls back to exactly today's behaviour. Nothing that
lowers today stops lowering because a verdict was wrong; nothing that is refused today is admitted
because a verdict was wrong.

**The monotonicity is the safety argument and it must be preserved literally in the code**, not
merely intended. The verdict may only widen the refusal set.

## What the analysis already gives, measured before relying on it

`probe_confinement_join.rs`, over 69 modules: **256 verdict sites against 256 planner placements, and
all 256 join at the same key**, which is the address of the `NewComposite`. `Scope::Iteration` is
documented as one iteration of the iterating loop, which is the question verbatim.
`Confinement::CannotEstablish` already carries the instruction to treat it exactly as `Escapes`, so
the operator's decline ruling is the analysis's own contract. And the known site reports
`Escapes because Yielded { ip: 25 } scope Iteration { loop_ip: 12 }`, matching the handoff's
independent record of built at op 24 and yielded at op 25.

## Prior failures to avoid repeating

- **Do not replace `yield_escape_hazards`.** Replacing it makes a wrong `Confined` verdict able to
  remove a refusal that exists today. Adding to it cannot.
- **A `Confined` verdict is sound to trust; an `Escapes` verdict is an upper bound.** The `v0.2.3`
  line's escape count fell from 12 to 10 when callee summaries landed and **those two were wrong
  rather than merely unestablished.** So an added refusal may be pessimistic, which costs coverage
  and never correctness — and coverage is what the acceptance criterion below measures.
- **Do not verify by acceptance.** The oracle is the corpus lowering set, not the fact that it built.
- **Check the binary count, not just the pass count.** A SIGTERM produced a plausible
  "398 passed, 0 failed" this session and only the short binary count betrayed it.
- **Stage explicitly.** `git add -A` swept a test file into a documentation commit this session and
  pushed it unverified.

## Acceptance criterion, from the operator

**The corpus lowering set must not shrink**: 67 of 69 modules end to end, 1072 of 1074 chunks, 89854
of 89940 opcode instances. A shrink means the added refusals reach a site the corpus needs, and the
operator's instruction is that the corpus keeps performing its purpose — so a shrink is a stop and a
report, not a thing to absorb.

**And each site the verdict newly refuses is to be CLASSIFIED, not counted**: whether its loop has a
static trip bound, in which case per-iteration placement is expressible later, or is divergent, in
which case reclamation is the only route and that is a separate increment.

## Outcome, written after the build

**Landed as scoped, and the interesting result is what it does NOT do.**

`lower_module` computes `module_confinement` once, the form carrying callee summaries, and passes each
chunk's verdicts through `BodyCfg`. The `NewComposite` arm refuses a site whose iteration-scope
verdict is anything but `Confined`. `lower_chunk` passes `None`, deliberately: the per-chunk form
assumes every call leaks and would refuse sites the module-level answer confines, so a single-chunk
lowering keeps exactly the behaviour it had.

**The acceptance criterion holds exactly.** 67 modules lower end to end, 1072 of 1074 chunks, 89854 of
89940 opcode instances, and the same two refusals with the same reasons. Nothing shrank.

### It adds nothing today, and the reason is measured rather than assumed

**The reference compiler refuses an early return inside a loop, and refuses reassignment.** So
`yield` is the only route by which a loop-built composite escapes its iteration, every chunk carrying
that shape is refused for `Stream` first, and the new refusal is unreachable from source.

**That is the accidental protection the obligation names, now measured.** It expires the day `Stream`
lowers, which is exactly when this guard starts earning its place.

### Reach, proved by mutating real bytecode rather than by a source program the language will not accept

Replacing `Stream` and `Reset` with `PopN(0)` in the telemetry module — index-preserving, so site
addresses stay valid — leaves the site refused, **by the pre-existing syntactic check**, which
precedes the verdict check by design. Disabling that check makes the confinement refusal fire on the
same site, naming it `Escapes its iteration (Yielded { ip: 25 })`.

**So the guard has reach, and its UNIQUE contribution is the interprocedural case** — built in a loop,
returned, yielded by the caller — which is the residual the obligation names and which no source shape
currently reaches. Stated rather than dressed up as coverage.

### The planner's documentation was corrected, not deleted

It said this lowering needs no verdict at all, and that a wrong verdict cannot miscompile anything
here. **Placement still consults no verdict and nothing shares storage**, so the property survives —
but in an exact form that had to be written down, because the refusal ordering is what preserves it
and the overlap use would forfeit it.

### And the skippable-tests scanner had its third false positive

The probe carries **Keleusma source inside Rust string literals**, including
`for c in ch { return [c, c]; }`, and the scanner matched another language's `return` as a Rust
statement. Its two earlier false positives were prose in comments and a `return` in a closure, and
the recorded repair for the closure was to rewrite the source. **This one is different in kind**:
rewriting a fixture to dodge an instrument, or adding the test to the pin, would record a false claim
that the test can skip. The scanner now strips string literals before inspecting a line, which fixes
the class.
