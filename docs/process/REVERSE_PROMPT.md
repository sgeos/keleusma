# Reverse prompt

## Order 6 in progress, 2026-10-25

The goal is to absorb kaizen work as it arrives, complete Order 6, and accumulate open
questions for the operator. Order 5 merged as PR 526. Kaizen PR 525 merged into v0.2.3
just before it and was absorbed. Kaizen and V0.3.X were told that Order 6 has begun.

Order 6 is full-language support, Workstream F with the A, B and C widening, and it
defines V0.3.0. The plan is docs/decisions/ORDER6_FULL_LANGUAGE_PLAN.md. Its gate is my
reading of the roadmap gate and you have not ruled on it.

Slice 0 adds tests/selfhost_full_language.rs, which pins the shipping self-hosted outcome
for all 75 Keleusma sources in the repository and checks the self-hosted validator
against the reference on every compiled one. Slice 1 repairs six parser states that
skipped a token instead of refusing it. Two of them produced a divergence and a panic,
one produced a false rejection, and three hung or faulted the parser. The corpus moved
from 34 to 36 identical sources, and the divergence, panic, no-verdict and false
rejection classes are now empty.

## Accumulated questions for the operator

Each question is independent and none blocks the next slice.

1. Order 6 gate. Do you accept the gate in the plan, byte identity over every repository
   source the reference compiles and over a construct corpus covering every reference AST
   variant, reference verdicts from the validator including a rejection corpus, the
   trap-free analysis on the widened corpus, and shipping integration?
2. Order 5 gate. Do you accept the Order 5 gate as stated in its plan?
3. Order 5 decryption. The decrypt path takes ciphertext to the end of the buffer rather
   than the declared total, so an encrypted artifact with trailing bytes cannot be
   decrypted while a signed one loads. Should it use the declared total?
4. Order 4 leftovers. The flat Text read trust question, a 0.2.2 patch release, which
   needs your authorization, and which pykeleusma finding to take first.
5. V0.3.X findings on this line's files, peer data I have not verified.
   docs/spec/RUNTIME_FAULTS.md still specifies native default values for an unhandled
   partial operation while the compiler emits an explicit Trap, and the whole-chunk time
   bound refuses intern_id in the self-hosted lexer as a text operation on two Word
   results. Should either be taken before Order 6 finishes?
6. pykeleusma finding, unverified here. A script's `use` declaration of a library native
   is not compared with the library's own signature, so a mismatched result type compiles
   and decodes wrongly, which pykeleusma infers is a route to the mistyped-decode crash.
   Should this line reproduce and repair it, or state that hosts must check?

7. V0.3.X Workstream I, peer data with V0.3.X's own measurements, not verified here.
   Native lowering surfaced language questions that are this line's to own. Should a
   local be allowed to hold Unit after Reset on one path and a value on another? Should
   a composite built inside a loop and yielded from it be admitted? And the host string
   ABI and the opaque-handle ABI each await your decision.
8. Two capacities surfaced by slice 3 refuse cleanly but are not guarded where they
   bind. Type-input extraction faults with IndexOutOfBounds(262, 256) on three piano
   rolls, and reconstruct.kel's 1,024-record input refuses two others. Should either
   be raised, or should the fault become a named diagnostic first?

9. reconstruct.kel holds sixteen heads per multiheaded function, and the host now refuses
   more by name instead of faulting. `rogue_gear.kel` has 21 heads per function and
   `rogue_bestiary.kel` 101, so both stay refused. May the head cap be raised, to 128 for
   example, with its per-head arrays and loop bounds? Its cost is linear in the cap.

10. Information-flow labels. The reference type checker enforces label flow through
   `classify`, `declassify` and labelled types, and compiles both operators to their inner
   expression unchanged. The self-hosted pipeline refuses them today. Dropping the labels in
   a token filter would make every such program byte-identical, but the standalone
   self-hosted verdict would then accept programs the reference rejects for a label
   violation. Should Order 6 port label checking into the type stage, or is admission under
   the shipping cross-check enough? I am leaving them refused until you rule. The same
   question now covers newtypes. The reference types a newtype nominally, rejecting
   `let w: Word = C(5)`, a `Word` passed for a `C`, and `C + Word`, measured 2026-10-26.
   A transparent lowering would compile `07_refinement.kel` byte-identically but would let
   the self-hosted type stage accept those programs, which gate 3's verdict agreement would
   expose. A refined newtype also needs the reference's interval-based elision of the
   predicate check. I am leaving newtypes refused until you rule.

11. The parser's field table. `parse.kel` counts every runtime data field of the program it
   parses against a table of 640, and `parse.kel` itself now uses 638, measured 2026-10-26.
   Its self-compile failed at 641 during the destructuring slice until the new state was
   packed into two arrays. Each further slice that needs parser state must pack again or
   wait. May the table, `fields.ffield` and its siblings, be raised, to 1,024 for example? Its
   cost is linear in the cap, like the name table you doubled on 2026-10-25.

12. The per-chunk opcode capacity of the self-hosted validator and trap analysis, 2,048
   (`TRAP_OPCODE_CAP`). Order 6's gate 3 asks for no capacity refusal on the repository
   corpus, and four sources exceed it in one chunk, measured 2026-10-26.
   `rogue_bestiary.kel` reaches 3,905 ops, `piano_roll_3.kel` 4,702, `piano_roll_8.kel`
   2,056 and `piano_roll_9.kel` 2,454. Both analyses refuse those four in agreement, and
   agree with the reference on every other compiled source. May the capacity be raised,
   to 8,192 for example? The stage arrays and the analyses' own bounds grow linearly with
   it.

13. A native index past the native table. `verify` and `Vm::new` both admit a module whose
   `CallVerifiedNative` names an index beyond `native_names`, measured 2026-10-26 on
   fourteen corrupted corpus modules. The self-hosted validator agrees, so this is not a
   self-hosted defect. Should admission reject it, or is the call-time failure the intended
   contract? I have not changed either side.
14. The boundary of self-hosted monomorphization. Generic functions are now instantiated
   with the decision in `parse.kel`, which derives each call's type arguments and parses
   the template once per instantiation by seeking to it. The host recognises template
   headers syntactically, keeps the chunk table, and repeats the parse in bounded probe
   rounds until no new instantiation is requested, as it already derives the chunk table
   from the tokens. Is this an acceptable reading of the roadmap's specialization table in
   the compiler's persistent state, or should the rounds and the header scan move into a
   stage as well? The design is in the Order 6 plan under slice 7.

Questions 11 and 12 are answered by the ruling below, and both raises have landed, the
field table at 1,024 and the opcode capacity at 8,192. Questions 8 and 9 are next.

## Rulings received this session

The operator ruled first-hand on 2026-10-26, in reply to the status summary that listed the
capacity questions, "Limits may be raised." I read it as answering questions 8, 9, 11 and
12, the type-extraction table, reconstruct's record input, the reconstruct head cap, the
parser's field table and the per-chunk opcode capacity. It does not cover a
`BYTECODE_VERSION` change, a new opcode or a publication. Each raise will land as its own
slice with its cost measured and recorded.

The operator ruled first-hand on 2026-10-25 to free the dead names in `parse.kel`,
continue with slice 2, and double the wire stage's name cap. The parser stood at 1,023
of 1,024 names. The ruling is recorded in the Order 6 plan.

## Concerns

The full-language boundary test takes about seven minutes in the release profile and
runs in the heavy self-host tier. Order 6 is large, and its slices will land as several
pull requests. HANDOFF.md is stale, and the interline checkpoint must be exported and
reviewed before any handoff.

## Slice 2

Native imports and qualified native calls compile byte-identically, and the corpus
reads 39 of 75 sources identical. The name cap is doubled and parse.kel holds 1,007
names. The cap doubling also doubled the wire stage's deduplicated name pool, `bout`,
from 16,384 to 32,768 bytes, because 1,023 names already filled most of it. That is my
reading of "double the name cap", and it is flagged here so you can overrule it.

## Slice 3a

Tuple-destructuring and wildcard `let` compile byte-identically, through a token filter
in lexer.kel, and the corpus reads 50 of 75 sources identical, including the Order 4
interpreter. Stale equality flags in parse.kel, which the shipping cross-check had been
refusing, are cleared.

## Slice 3b

The pipe operator and single literal parameter heads compile byte-identically, and the
corpus reads 52 of 75 identical.

## Slices 3d, 3e and 4

The array `for`, the `signed`, `pure`, `ephemeral` and `external` modifiers, and trait
impls with method calls compile byte-identically. The corpus reads 57 of 75 identical.

## Slice 5d and the AST census

The census holds one program or more for every reference AST variant and reads 54
Identical of 70 rows. The saturation words, which the parser rejected as undefined names,
compile byte-identically. The whole isolated self-host population found ten tests that
slices 5 to 5c had made stale, and they now pin the new float boundary. Two tooling defects
went to the kaizen line, the `cargo tns` alias, whose quoted filterset nextest cannot parse,
and `scripts/gate_summary.py`, which is not executable here.

## Slices 6 and 7

Struct-typed private data fields compile byte-identically, with the private composite pool
the host used to assume empty, and so do natives that return an enum. `14_frame_log.kel`
is Identical. The RTOS task scripts are now also measured as the RTOS crate compiles them,
prelude first, and all five are refused, four only because the prelude declares
tuple-returning natives they never call. That is the next slice.

## Slice 8 and a time-bound defect

The RTOS prelude and its five task scripts compile byte-identically once the prelude's
tuple-returning signatures are accepted. The last of them exposed a defect in the
self-hosted time analysis, which gave every stream matching an enum directly a bound below
the reference's. The shipping cross-check had refused each such program, so nothing was
admitted with the lower bound. The analysis now agrees with the reference on those
programs. A separate overestimate on a counted loop with a conditional break remains, in
the safe direction, and is next.

## A `break` in a bare `for`

The overestimate I reported after slice 8 was a codegen defect, not an analysis one. A
`break` in a `for` without `limit` was lowered as if the loop had one, writing into the
first parameter's slot and leaving its branch unpatched. The cross-check refused every
such program. It now compiles byte-identically, and the earlier wording is corrected in
the plan.

## Builtin generic signatures, 2026-10-26

PR 566, the pipe placeholder, merged as e48b3a62. `Option<T>` and `Multiword<N>` in a
function signature now take the reference layouts, and their census rows are Identical. The
census reads 63 Identical, 8 Unsupported and 3 RefRejects of 74 rows. Three residuals are
pinned by a test rather than hidden. An `Option` over a struct is refused, because the host
has no struct flat size. The self-hosted type stage does not compare generic arguments, so
only the reference cross-check rejects a mismatch. Option values, their construction and
their match, are not yet lowered and have Unsupported census rows.

A match over an `Option<T>` parameter with a scalar `T` now compiles byte-identically
too, through two reserved rows of the parser's enum table. The census reads 64 Identical of
74. The enum row arrays were widened under your ruling that limits may be raised.

Option construction compiles byte-identically too. `Option::Some` is sized from its
payload's scalar type, and `Option::None` becomes the reference's boxed composite with a
per-chunk template, the first template the self-hosted driver produces. The census reads 65
Identical of 74. The six Unsupported rows are the five awaiting question 10 and generic
functions.

A match over an Option call result or binding compiles byte-identically as well. While
widening it I found and fixed a defect of my own making before it merged. A payload kind
carried stale from a later binding made the self-hosted type stage reject sources the
reference accepts. The regressions are pinned.

## Generic functions, 2026-10-26

Generic functions over the scalar types `Word`, `Byte` and `bool` compile byte-identically,
including templates declared after their use, several type parameters and several
instantiations. The census reads 66 Identical of 74, and the five Unsupported rows that
remain all await question 10. Question 14 asks whether the design's division
between stage and host is acceptable. Residuals are pinned, among them an uninstantiated
template, which the reference compiles with its type parameter unresolved.

## Gate 5 and the Order 6 audit, 2026-10-27

A command-line test now runs the shipping `keleusma compile --compiler self-hosted` on every
Identical census row, read from the census table, and each artifact equals the reference
serialization. The Order 6 completion audit is
[ORDER6_COMPLETION_AUDIT.md](../decisions/ORDER6_COMPLETION_AUDIT.md). Its disposition is
that Order 6 is not complete. Every gate condition is met except the information-flow and
newtype forms, which wait on question 10, and one repository source, `07_refinement.kel`,
is among them. I also corrected an earlier record of mine that claimed gate 1 on the whole
corpus. Question 1, the Order 6 gate acceptance, now has the audit as its evidence.

## Labels and newtypes, 2026-10-27

The five forms that waited on question 10 now compile byte-identically, and so does
`07_refinement.kel`. I concluded that compiling them does not decide question 10, and I
would like you to check that reading. The reference erases labels and newtypes from the code
it emits, so either answer needs the same code generation. The open question is only
whether the self-hosted type stage should also check label flow and newtype identity. Until
you answer, a violation is rejected by the reference cross-check alone. A related finding,
offered as data: the reference lowers `x as Word` for a `Word` x to `FloatToInt`, while the
same cast of a newtype value emits nothing. I have not changed the reference.

## Next

The pinned residuals in the audit, starting with Option struct payloads and generic
arguments in the self-hosted type stage. The newtype and IFC rows await question 10.
