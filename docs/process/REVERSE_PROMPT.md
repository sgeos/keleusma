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

## Rulings received this session

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

## Next

`use` signatures, newtypes and refinement, struct-typed data fields, and then the
construct corpus for the types the repository's sources do not use: floats, fixed-point,
text operations, generics and the remaining patterns.
