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
