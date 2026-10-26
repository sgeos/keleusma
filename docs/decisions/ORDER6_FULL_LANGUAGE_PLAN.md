# Order 6 full-language plan

> **Navigation** [Order 5 audit](./ORDER5_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md) | [Self-hosting design](../roadmap/V0_3_0_SELF_HOSTING.md)

## Scope and its provenance

Order 6 of the V0.2.X roadmap is full-language support, Workstream F with the widening
of Workstreams A, B and C. Its completion defines V0.3.0. The roadmap gate reads "The
self-hosted compiler and validator accept the full grammar and the toolchain compiles
arbitrary conforming programs, not only its own subset." Success criteria 1 and 2 of the
roadmap add that the compiler's output is byte-identical to the reference over a
full-language corpus, and that the validator reproduces the whole `verify()` verdict,
including the trap-free analysis.

**The concrete gate below is this line's reading of that text. The operator has not
ruled on it.** The session that began Order 6 had a goal to complete it and no gate
ruling. The question is recorded in the reverse prompt.

## Measured baseline, 2026-10-25

`tests/selfhost_full_language.rs` runs the shipping entry `self_hosted_compile` over every
Keleusma source in the repository, 75 files, and pins each outcome. At the start of Order 6
the outcomes were these.

| Outcome | Files | Meaning |
|---|---|---|
| Identical | 34 | The self-hosted module equals the reference |
| Unsupported | 27 | The pipeline refuses a construct outside the subset |
| Diverges | 1 | The self-hosted output differs from the reference and is refused |
| StagePanic | 2 | A stage failure recovered into a refusal |
| NoVerdict | 5 | The type stage could not run, here a parser capacity |
| PipelineRejects | 2 | The self-hosted type stage rejects before the reference runs |
| RefRejects | 4 | The reference rejects the source as given |

The 34 identical files are the fourteen stage sources, three numbered examples, fourteen
rogue scripts and three others. The Order 4 interpreter and the Order 5 framing program
are not among them. The
validator side is far wider. Of the 70 sources the reference compiles, the self-hosted
validator gives the reference verdict on 66 and reports a capacity of 2,048 opcodes per
chunk on the other four. That corpus holds no source the reference rejects, so it says
nothing about false acceptance.

Four findings sit inside the baseline and are not language gaps.

1. `06_multiheaded.kel` diverges at the first operation of `classify`, a constant where the
   reference reads a local. The shipping cross-check refuses it. It is a miscompile in the
   self-hosted pipeline that the cross-check contains.
2. `05_pipeline.kel` is rejected by the self-hosted type stage although the reference
   accepts it. The pipe operator is outside the subset, so the honest outcome is
   Unsupported. Reporting it as a source rejection is a false rejection.
3. Five sources stop at `IndexOutOfBounds` inside `parse.kel` on resume, a parser
   capacity of 32 or 8 that a stage table does not guard.
4. Two rogue scripts reach a recovered `unwrap` on a shared slot index equal to the slot
   count, an off-by-one or a missing bound in the self-hosted pipeline.

## The gate

1. Every repository source that the reference compiles is `Identical` through
   `self_hosted_compile`, and every source the reference rejects is rejected by both.
2. A construct corpus with at least one program for every variant of the reference AST,
   `Stmt`, `Expr`, `Pattern`, `TypeExpr`, every declaration kind and every operator and
   literal form, is byte-identical to the reference. `self_hosted_construct_support_boundary`
   in `tests/selfhost_codegen.rs` is the table this widens.
3. The self-hosted validator gives the reference verdict on every compiled source and on
   a rejection corpus, with no capacity refusal on the repository corpus.
4. The trap-free analysis agrees with the reference on the widened corpus.
5. The shipping command line uses the widened toolchain, as it does now for the subset.

## Slices

The order is by measured corpus impact, with defects first because a widening built on a
miscompile inherits it.

| Slice | Content | Corpus files it gates |
|---|---|---|
| 0 | The instrument, `tests/selfhost_full_language.rs` | All |
| 1 | The four baseline defects above | 10 |
| 2 | Native imports and qualified native calls, `use host::f` and `host::f(...)` | About 20 |
| 3 | Binding patterns and `for` over a non-range iterable | About 10 |
| 4 | The pipe operator, method calls, `impl` and `trait` | 3 and more |
| 5 | Newtypes and refinement, `signed`, `pure`, `ephemeral` and `external` | 2 and more |
| 6 | `Float`, `Fixed<N>` and `Text` operations | Construct corpus |
| 7 | Generics, const generics and the full monomorphizer | Construct corpus |
| 8 | Closures, checked arms, `assert`, saturation, classify and declassify | Construct corpus |
| 9 | Validator capacity and a rejection corpus, then trap-analysis widening | Gate 3 and 4 |

Each slice lands as a feature branch and pull request with the boundary tables updated
deliberately, so a widening is a pinned move and a regression is a test failure.

## Constraints

**Operator ruling, 2026-10-25, received first-hand in the session that began Order 6.**
"Free the dead names and continue with slice 2. Also, feel free to double the name cap."
The name cap is `nm_max_names` in `wire.kel`, 1,024 at the time of the ruling, with
`parse.kel` holding 1,023. Doubling it to 2,048 quadruples the interner's static
worst-case comparison bound, because `nm_find` scans every interned name with no early
exit. That cost is the reason earlier increments folded new parsing into existing names
rather than raising the cap.


`BYTECODE_VERSION` stays 2 and the instruction set stays at 66 opcodes, unless the
operator authorizes otherwise. The self-hosted pipeline never supplies a field from the
reference, and the reference stays the differential oracle. A divergence establishes that
the two disagree, not which is wrong.

## Slice record

**Slice 0 and slice 1, 2026-10-25.** The instrument landed with the baseline above. Every
one of the four baseline findings traced to a token the parser skipped rather than
refused, so each was repaired by refusing or lowering that token.

| Skipped token | Effect before | Repair |
|---|---|---|
| A non-name parameter, `fn f(0)` | The head had no parameter and no test, the `06_multiheaded` divergence, and the shared-slot panic in two rogue scripts | Refused as a parameter pattern |
| An unrecognized byte, such as the pipe bar | Lexed as the arrow, which the body walk skips, so `6 \|> f()` read as `6 > f()` | Lexed as its own token and refused |
| A `}` after a trailing comma in a struct literal | The literal never closed, so the parse hung, faulted or miscompiled to a data write | Closes the literal without counting a field twice |
| Any token after `E::V` other than `(` | Skipped up to the next `(`, faulting an eight-entry stack | Lowered exactly as `E::V()`, which is the reference's AST |
| A `.` after a data-field read | The projection became an undefined identifier, a false rejection | Refused as unsupported |
| An unresolved data-field name | Read slot 0 | Refused as unsupported |

After slice 1 the corpus reads 36 Identical, 34 Unsupported, 1 PipelineRejects and 4
RefRejects, with no Diverges, StagePanic or NoVerdict. The one pipeline rejection is
`heartbeat.kel`, which the reference also rejects because its natives are supplied by a
prelude the corpus does not prepend. `skipped_tokens_are_refused_or_lowered_rather_than_dropped`
pins each repair with its controls, and checks that every pipeline rejection it names is
also a reference rejection.

**What slice 1 does not establish.** A refusal is not support. Literal parameter heads,
the pipe and data-field projections remain outside the subset, and slices 3 and 4 own
them. The parser may skip tokens in states this slice did not reach. The repairs were
found by the corpus, not by a census of every parser state.

Slices 0 and 1 merged as pull request 532 at `cf9870b7`, CI run 40234368533.

**Slice 2, 2026-10-25, native imports and qualified native calls.** Under the operator's
ruling above, twenty names that nothing read left `parse.kel`, and the wire stage's name
cap doubled to 2,048. Then the pipeline learned the native call.

- `parse.kel` keeps a table of `use` declarations in declaration order, which is the order
  the reference numbers natives in. A two-segment call `host::f(...)` resolves against it
  and reuses the call machinery, closing as record 80.
- `reconstruct.kel` folds record 80 as it folds a call, into node kind 81. Kind 80 was
  already the checked binding, and the first attempt reused it, which broke every checked
  arm. The library's checked-outcome tests caught that, not the native tests.
- `codegen.kel` lowers node 81 to `CallVerifiedNative(index, count | 0x80)` followed by
  the reference's default handling, a flag of one trapping with `HostError`.
- The driver joins each import's segments into `native_names` and gives each a `Top`
  return shape, as the reference does for an import without a signature.

Refused rather than skipped: a wildcard import, which the record stream cannot tell from
a single-segment one and which takes no index, a `use` carrying a signature, a
single-segment import called unqualified, and an `external` import, which the stage's
lexer has no keyword for. After slice 2 the corpus reads 39 Identical, 29 Unsupported,
2 NoVerdict, 1 PipelineRejects and 4 RefRejects. Three rogue scripts became Identical. The
two NoVerdict sources passed the native barrier and now stop at explicit parser
diagnostics rather than faults, an unavailable expression layout in `rogue_dungen.kel`
and the sixty-four-local cap in `piano_roll_3.kel`. Most of the remaining Unsupported
sources now stop at a binding pattern, which is slice 3.

**Slice 3a, 2026-10-25, destructuring and wildcard `let`.** The reference lowers
`let (a, _, b) = e;` by storing `e` in a fresh local and reading each element with
`GetTupleField`, binding it or popping it, which is exactly the plain statements
`let T = e; let a = T.0; T.1; let b = T.2;`. A filter in `lexer.kel` now rewrites the
pattern into those tokens, and the parser handles them as it always has. The hidden
binding `T` is interned from the bytes of the `let` keyword, so its name is `let`, which
no identifier can be. A wildcard `let _ = e;` is the expression statement `e;` operation
for operation, and the filter drops its three leading tokens. A nested or annotated
pattern, a `let` inside the destructured value, and more than eight elements are refused.

Slice 3a exposed a defect in `parse.kel`. The whole-value flags that make `==` fold as an
array, tuple or struct equality were cleared only when an identifier resolved, so a flag
left by `let a = [1, 2];` or by a tuple literal ending an `if` branch survived into a later
scalar comparison, which then folded as an equality loop. The shipping cross-check
refused every such program, so nothing miscompiled in shipping. The flags are now cleared
whenever a statement, block, condition or branch tail finishes folding, and indexing an
array clears its own flag.

After slice 3a the corpus reads 50 Identical, 13 Unsupported, 7 NoVerdict, 1
PipelineRejects and 4 RefRejects, with no Diverges or StagePanic. Eleven sources became
Identical, among them the Order 4 meta-circular interpreter `src/hosted/interp.kel`,
`09_big_numbers.kel`, `10_multbyte.kel`, four rogue scripts and four piano rolls. Five piano rolls now stop at clean capacity refusals, three
at an unguarded 256-entry table in type-input extraction and two at reconstruct's
1,024-record input.

**Slice 3b, 2026-10-25, the pipe operator and literal parameter heads.** The lexer reads
`|>` as its own token. In `parse.kel` a pipe drains its left operand as the loosest
operator would, and the call that follows opens with one argument already counted,
because that operand's records precede the call's own arguments in postorder. That is
the reference's `f(x, args)`. The placeholder `_` is refused.

A head whose sole parameter is a literal, `fn f(0)`, now records the parameter under no
name and queues the guard `p0 == 0`, five records for one token, through a small record
queue drained ahead of the next token. The reference lowers the pattern to `GetLocal(0)
Const(0) CmpEq If`, before any parameter is bound, and a literal binds nothing. The host
marks such a head as head mode 2, in which codegen tests first and copies no parameter.
A literal beside another parameter, two literals, a literal with a `when` guard, and a
negative literal are refused. `reconstruct.kel` holds sixteen heads per function, and the
host now refuses more by name; it used to write past the head arrays, which surfaced as
the shared-slot fault the baseline recorded for two rogue scripts.

After slice 3b the corpus reads 52 Identical, 11 Unsupported, 7 NoVerdict, 1
PipelineRejects and 4 RefRejects. `05_pipeline.kel` and `06_multiheaded.kel` became
Identical. `parse.kel` interns 1,016 names.

Slices 2 and 3a merged as pull request 538 at `84cbea1d`, CI run 40246546955.
