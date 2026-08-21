# Brief — reachability of `Len` and `IsStruct`, then nested arrays

Written 2026-08-20 afternoon, after an overnight run that fixed four silent
miscompiles and recorded a fifth. **The value of this document is the list of
wrong turns, all of which were taken at least once in the last day.**

## G1 — are `Len` and `IsStruct` producible from any source?

Raised by the `v0.3.0` line's opcode census, which stands at 64 of 66 with these
two unwitnessed after eight construct attempts. **They reframed it correctly and I
should not lose that framing**: the question is not "which construct" but "whether
one exists".

**Both fire only when a static type is UNKNOWN**, so the target is making
INFERENCE FAIL, not finding an unusual shape:

- `Op::Len`, `src/compiler.rs:7647`, fires when `fc.static_for_in_length(expr)`
  returns `None` for a `for`-in source. Its own comment calls the fallback
  "admissible at the bytecode level but may be rejected by the verifier in strict
  mode", which reads like a defensive path nobody expects to take.
- `Op::IsStruct`, `src/compiler.rs:11364`, fires when
  `named_type_name(ty) != Some(type_name)` for a struct pattern — a struct pattern
  whose scrutinee is NOT statically that struct. `src/compiler.rs:12631` already
  asserts the fold-out in the ordinary case.

**EITHER ANSWER IS A RESULT.** If a construct reaches them, the census closes at
66 of 66. If nothing can, two opcodes exist that no program produces, and for an
ISA whose opcode count is a stated rad-hard constraint that is the more valuable
finding. **Do not treat "I could not find one" as "none exists"** — that is the
distinction the census exists to make, and the peer already declined to collapse it.

## G2 — nested array literals

Measured, recorded as `Diverges`, not fixed:

```
fn main() -> Word { let a = [[1, 2], [3, 4]]; 1 }
  reference outer: NewComposite(Flat { kind: Array, count: 2, byte_size: 32 })
  self-hosted:     NewComposite(Flat { kind: Array, count: 2, byte_size: 16 })
```

The outer array of two 16-byte arrays is sized as 16 — the element size is not
propagated. With a chained index the body is additionally TRUNCATED: no
`SetLocal`, no `GetLocal`, neither `GetIndex`. A FLAT array is byte-identical.

**TWO DEFECTS, NOT ONE.** A byte-size computation and a dropped index chain. Treat
them separately; fixing the size may not touch the truncation at all.

**THE STOPPING RULE, STATED IN ADVANCE SO IT IS NOT NEGOTIATED LATER.** This sits
in composite-layout machinery that the flat-byte representation makes load-bearing
for worst-case memory bounds. **If the change reaches beyond the size computation
and the index lowering — into `NewComposite` semantics, the arena, or WCMU — STOP
and record.** Overnight I declined this on depth grounds, and depth has not changed
because the hour has.

## The wrong turns, from the last twenty-four hours

**THREE OF MY CLAIMS YESTERDAY WERE PLAUSIBLE RATHER THAN CHECKED**, and none of
them cost anything only by luck:

- "the harness needs six accessors" — it needs three
- "the stale changelog sentence is an earlier release's, superseded later" — same
  release section, and the opposite reading order
- "expect the cherry-picked hunk to need re-anchoring" — it applied cleanly

**A plausible story is not a cause.** Before asserting a mechanism, ask what
evidence distinguishes it from the alternatives. If none does, say so.

**COMPARE BYTES, NOT OPS.** The string-literal divergence has identical ops and a
different constant pool. An ops-only comparison calls it clean.

**CLASSIFY THREE WAYS.** Identical, refuses loudly, DIFFERS. Only the third is
dangerous; a loud refusal is an honest gap. `Support::Gap` conflated the last two
and hid three silent miscompiles.

**A DIFFERENTIAL ORACLE ONLY CATCHES A DEFECT INTRODUCED ON ONE SIDE.** The
`Bool`/`bool` regression shipped because the same wrong change was made to both
extractions in one increment. **When editing both sides of a comparison, stop.**

**GENERATE PROBES FROM CORPUS SOURCES, NOT FROM MEMORY OF THE GRAMMAR.** Two
probe-syntax errors in two sweeps: `let mut`, which this language does not have,
and a checked-arithmetic form I guessed at.

**THE TEST HARNESS CARRIES ITS OWN COMPILER**, and it has measurably diverged from
the library's. If a result differs between `keleusma::selfhost::self_host_compile`
and `tests/selfhost_codegen.rs`'s local one, that is not a mystery — it is the
duplicate. Say which compiler a claim is about.

**CAPTURE EXIT CODES OUTSIDE THE PIPE**, and re-run clippy rather than trusting a
green from earlier in the session: the toolchain moved under this session
yesterday and turned every prior green into no evidence.

**A GUARD THAT FIRES ON ITS OWN DOCUMENTATION MEASURES NOTHING.** My must-fire
check for boolean literals in stage code fired on the word `true` inside the
comment explaining the fix.

**AN ITEM IS ITS ATTRIBUTES AND DOC BLOCK, NOT ITS `fn` LINE.**

**ROOT `cargo fmt`/`clippy` DO NOT REACH `compiler/`**, which declares its own
workspace.

**THE FEATURE MATRIX IS FOUR ENTRIES**, and `--all-features` is not one this
project passes.

## What to do when blocked

Stop and record, naming the specific decision required and the evidence for it. A
workaround that widens scope unattended is worse than a clean stop, because the
operator loses the choice. Three items are already waiting on them; do not add a
fourth by proceeding on a guess.

---

# CLOSED — AND THE CLOSURE CREATED AN ISA QUESTION (2026-08-21)

`Op::IsStruct` was witnessed, qualified, and the defect that produced the witness was repaired.

## The witness, and why seventeen attempts missed it

A struct pattern on a parameter with **no type annotation**: `fn g(P { a, b }) -> Word { a + b }`.

The guard is `named_type_name(ty) != Some(pattern_type)`. Every attempt across two lines tried to
make the two DIFFER — and **the type checker forbids that outright**, refusing with "struct pattern
`P` does not match scrutinee type". The inequality is satisfiable only when the scrutinee's type is
ABSENT, and a match scrutinee always has one. **The route was never an expression whose inference
fails; it was a declaration site with no type to lose.**

## What the witness did, which was the real finding

`verify()` accepted it, `module_wcmu` gave it a bound, it loaded, and it **trapped
`InvalidBytecode`** at call time. That is the class `verify()` exists to exclude, so it was a hole
in the load-time check rather than a bad program.

Of the three "should never have been emitted" refusals the virtual machine carries — two for
`Op::Len`, one for `Op::IsStruct` — **this was the only one a program that actually loaded could
reach.** `Op::Len`'s witness is refused at LOAD by the strict iteration-bound check, which is the
conservative-verification stance working as designed.

I was one step from reporting that the class generalised. Running both witnesses instead of one
caught it.

## The repair, and why in the compiler

Two sites were available. Rejecting in `verify.rs` would make a legal program fail EARLIER; folding
the irrefutable test makes it **work**. The fold already existed and was conditional on the
SCRUTINEE's type matching the pattern's — and an un-annotated parameter has no scrutinee type at
all.

**An absent type is not an unconfirmed one.** The type checker has already established the match, so
when the scrutinee's type is merely absent the pattern's own type is the answer.

The witness now returns `Int(3)`, **asserted as a value** rather than as the absence of a trap: a
fold that changed the program's meaning would be worse than the trap it replaced.

## RETRACTED — I CLAIMED THE OPCODE HAD NO PRODUCER, AND IT HAS FOUR

For about an hour this document said repairing the defect removed the only producer, and that the
opcode was a removal candidate for an ISA whose opcode count is a rad-hard constraint. **That was
wrong.** The `v0.3.0` line disproved it and I reproduced their counterexamples independently before
accepting them.

| construct | emits | verifies | runs |
|---|---|---|---|
| generic struct destructured in a parameter | yes | yes | **traps `InvalidBytecode`** |
| pattern `P` against annotation `Q` | yes | yes | **traps `InvalidBytecode`** |
| tuple-typed annotation | yes | yes | traps `NoMatchingHead` |
| array-typed annotation | yes | yes | traps `NoMatchingHead` |
| unannotated parameter | no | yes | `Int(3)` |

**The load-time hole is NARROWED, not closed.** The fold fixed exactly the construct it was tested
against.

## HOW THE OVERCLAIM HAPPENED, WHICH IS THE TRANSFERABLE PART

I found the original witness by **reading the guard's match arms for what they omit** — the method
that cracked `Op::Len` after fourteen guessed constructs failed across two sessions.

Then I validated my own repair by **guessing three constructs**, observing none emitted, and
generalising to "no producer found".

The other line applied my method to my code: read the emission condition
`ty.is_some() && named_type_name(ty) != Some(type_name)`, then enumerate which `TypeExpr` variants
make `named_type_name` return `None` **while a struct pattern is still accepted**. Four
counterexamples, inside an hour.

**A method used to find a defect is not automatically applied to validating its repair**, and the
repair is where the incentive to stop looking is strongest. That is the lesson, and it is mine
rather than theirs.

They also reported their own first attempt was eleven guessed constructs, six of which did not
compile — so both lines guessed first and both were rescued by reading. The difference is that they
were guessing about someone else's change.

## THE JUSTIFICATION WAS FALSE, NOT MERELY INCOMPLETE

The fold's comment claimed the type checker "refuses the mismatch outright". It does not.
`fn g(P { a, b }: Q)` compiles with two distinct structs. The "known and different" state the
condition treats as needing a runtime test is one the type checker admits today, and
`src/compiler.rs` now records that.

## WHAT IS ACTUALLY OPEN, AND IT IS A LANGUAGE QUESTION

A struct pattern matched against an unrelated struct, a tuple, or an array is arguably **ill-typed
at the source**. Closing it in the type checker would remove the emission rather than fold it, and
would close two load-time holes at once. That is a language decision rather than a lowering fix.

**No claim is made here about whether `Op::IsStruct` should exist.** It has producers; the removal
question is not live on this evidence.

## A SURVEY THAT CAME BACK CLEAN, RECORDED WITH WHAT WAS SEARCHED

The `v0.3.0` line built a witness-integrity guard because **a witness that stops witnessing is
invisible** when coverage is reported as a number — their census would have dropped from 66 to 65
with nothing going red.

Searched this tree for the same shape: coverage claims asserted as a bare count, and thresholds that
tolerate silent loss. **None found.** The construct-support boundary asserts per-case verdicts and
collects mismatches by label; the reachability tests assert per-construct; the four `>=` assertions
are non-vacuity guards on corpus size, which is the correct use of a threshold rather than a
coverage claim.

A negative result is worth recording only alongside what was looked at, which is why the search is
stated rather than just its outcome.
