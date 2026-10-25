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

## Rulings received this session

The operator ruled first-hand on 2026-10-25 to free the dead names in `parse.kel`,
continue with slice 2, and double the wire stage's name cap. The parser stood at 1,023
of 1,024 names. The ruling is recorded in the Order 6 plan.

## Concerns

The full-language boundary test takes about seven minutes in the release profile and
runs in the heavy self-host tier. Order 6 is large, and its slices will land as several
pull requests. HANDOFF.md is stale, and the interline checkpoint must be exported and
reviewed before any handoff.

## Next

Slice 2, native imports and qualified native calls, which gate about twenty corpus
sources.
