# Brief: A Pipeline Analogue of `decl_call_rows`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Drafted**: 2026-08-27 (session 55)
**Status**: working brief

## The goal, and the number that frames it

Order 1 item 3 is the type checker's INPUT. Its resolution moved into the stage; **the
extraction did not**. Five Rust functions still walk the reference parser's abstract syntax
tree to feed `stage_verdict`.

**Measured, and the handoff does not state this figure: ONE of the five is moved.**

| extraction | real body | pipeline analogue |
|---|---|---|
| `decl_call_rows` | **50 lines** | no |
| `field_sets` | 80 | no |
| `occurrence_rows` | 100 | no |
| `expression_nodes_and_derived`, behind the thin `expression_nodes_resolvable` | **142** | no |
| `binding_rows` | 98 | **yes** |

## Why this one, departing from the handoff's recommendation

**The handoff names `expression_nodes_resolvable` as the slice**, and by CAPABILITY it is
right: `let d = 1 + 2` needs the initialiser's node index, and that row comes from there.

**By TRACTABILITY it is the worst starting point.** It is a six-line wrapper over the largest
body in the family, and the handoff itself calls it "bigger than it looks". `decl_call_rows`
is the smallest at 50 lines, and moving it takes the ratio from one in five to two in five
while establishing the pattern the remaining three follow.

**Both readings are stated because the choice is a judgement, not a fact.** A reader who
wants the capability sooner should take the larger slice knowingly, not by accident.

## Why it is feasible

`decl_call_rows` extracts, per function, the parameter count and each parameter's type tag,
plus the call sites. **The driver already holds all of it**: `ParsedFn` carries `params` and
`param_types` from the header records, and a Call record carries the callee chunk and its
argument count. This is a re-projection of data already in hand, not a new walk.

## The design decision, settled before writing any code

**The two sides carry different things, and this is the id-space trap in a new costume.**

| side | what a parameter's type is |
|---|---|
| reference | a semantic TAG from `type_tag` — `Word` is 1, `Bool` is 2, `Byte` 3, `Float` 4 |
| pipeline | the type-NAME id from the header records |

Comparing those directly compares two unlike things. **Carry the type NAME on both sides**,
exactly as the previous slice carried the callee's name as a string so that "neither side's id
space enters the comparison".

**And here that is the SAFER choice, not merely the easier one.** `type_tag`'s own comment
records that an earlier revision mapped `Named("Bool")` to the boolean tag and was wrong:
`Word`, `Byte` and `Float` are primitives and capitalised, `bool` is a primitive and
lowercase, and `Bool` is an ordinary named type. Making the pipeline reproduce the tag would
mean re-implementing a mapping **this repository has already got backwards once**. Comparing
names skips that hazard entirely.

## Prior failures this work must not repeat

- **A number in a message is not a cause**, and a claim in a document is not a fact. The
  handoff's open item 4 asserted a gap that was already closed and cited a test that does not
  exist; three live comments repeated it and a debt register excused all three. **Check each
  named entity before building on it.** That is how this brief's own figure was found.
- **The two id spaces do not match.** The blocker on the previous slice was that a form-1 row
  carried a NAME ID and the two extractions do not share a numbering, so comparing them would
  have compared the numbering rather than the content. **Carrying a string removed the
  question rather than answering it.** Expect the same trap here and prefer the same escape.
- **I derived a family of three that was four, and of seven that was 26.** Derive sets from the
  source and assert the derivation is non-vacuous.
- **Guessing failed seventeen times across the `wire.kel` causes; bisection succeeded three
  out of three.** Reach for an instrument before a hypothesis.
- **Read cargo's own exit status from the log.** A run reported "exit code 0" while its log
  said 101, and killed runs have twice looked like passes.

## The specific wrong turns to avoid

1. **Do not compare id spaces.** If the pipeline row and the reference row carry numbering
   from different interners, the comparison tests the numbering. Carry a string, or compare a
   projection that is id-free.
2. **Do not call the type checker self-hosted.** The resolution is; the extraction is one of
   five, becoming two of five. **No claim should say "self-hosted" without saying which half
   is meant** — the test file's own header says exactly this.
3. **Do not widen the agreement test into a vacuous one.** A comparison over an empty row set
   passes while establishing nothing; assert the row count is non-zero.
4. **Do not move the ratio and describe it as the item.** Two of five is two of five.
5. **No new opcode. No `BYTECODE_VERSION` change.**

## What a complete increment looks like

The analogue exists, agrees with the reference on the corpus, the agreement is non-vacuous,
and the count of moved extractions is stated as a measurement wherever it is claimed. **If
the id-space trap bites and the honest answer is that a projection cannot be compared, saying
so with the measurement is a complete increment.**
