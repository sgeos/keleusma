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

Slice 3b merged as pull request 540 at `83aabf16`, CI run 40252531803.

**Slice 3c, 2026-10-25, three more dropped tokens.** A `use` path joined any two
consecutive identifiers, so `use host::open` followed by `require word >= 64;` read as
one path, which swallowed the directive and left the Order 5 framing program's data block
misparsed. A segment now continues the path only after `::`, and `src/crypto_frame/frame.kel`
compiles byte-identically. A leading shebang line is skipped as the reference lexer skips
it. And the operator dispatch's last arm returned without a record for any token it did
not claim, which dropped a `.` after an untyped value and had made a float literal diverge
by accident; it now refuses, and the full suite shows no stage source reaching it. The
corpus reads 53 Identical.

Slice 3c merged as pull request 541 at `1d944ee0`, CI run 40256188941.

**Slice 3d, 2026-10-25, `for` over a static scalar array.** The reference copies the array
into a fresh local, stores its length and then a zero counter, and binds the loop variable
to the element at the counter before the body. `parse.kel` writes exactly that: a hidden
`let` of the iterable, a bare range loop over the counter with the length and zero as its
bounds, and `let x = tmp[ctr]` as the body's first statement, queued through the record
queue. The build record marks the array form, and codegen then initialises the end before
the counter. `04_for_in.kel` and `13_telemetry_stream.kel` became Identical, and the
corpus reads 55 of 75.

**Slice 3e, 2026-10-25, the function and import modifiers.** The self-hosted lexer reads
the reserved words `signed`, `pure`, `ephemeral` and `external` as keywords. `signed`
before the entry sets the module's requires-signature flag, as the reference does, and on
any other head the reference's rejection stands. `pure` and `ephemeral` change nothing the
reference emits for a program it accepts, so the parser consumes them and the reference
keeps their rejections. `use external` marks its import row, and its calls lower to
CallExternalNative through a new transport tag, 68, for that existing instruction.
`11_signed.kel` became Identical.

**Slice 4, 2026-10-25, trait impls and method calls.** The reference compiles each method
of `impl Trait for Head` as the chunk `Trait::Head::m`, sorted among the plain chunks, and
resolves `recv.m(args)` to the first such chunk for the receiver's type head, passing the
receiver first. The host's first pass now composes those names and seeds a method table
into the parser's token block, the parser reads the impl header and parses each method as
a function, and a method call on a scalar or struct receiver resolves through the table
and opens like a pipe. A default trait method body, which the skip machine used to drop,
and an inherent or generic impl are refused. The host names every head by its chunk name,
so the type stage declares and calls methods consistently. `08_method_dispatch.kel`
became Identical, and the corpus reads 57 of 75.

**A defect in the name-cap doubling.** `nout_capacity` stayed at 2048 when `nout` doubled,
so `wire.kel` refused any module past 1,024 names. No module reached that until slice 4
took `parse.kel` to 1,029 names, and it is now 4096.

**Slice 2b, 2026-10-25, `use` signatures.** A signature's parameter types change nothing
the call lowers to, and the reference checks them, so the parser skips them. Its return
type fixes the native's return shape, Scalar kind 3, 2, 1 and 0 for `Word`, `Byte`,
`bool` and `()`. A tuple or other composite return is refused. On the V0.3.X line's
request, the shared `rogue_dungen.kel` now declares `rng_range`'s signature as that line
has since 2026-08-14. Its outcome stays NoVerdict, at the parser's expression-layout
capacity.

**Slices 5, 5b and 5c, 2026-10-25, Float arithmetic, literals and negation.** The
reference lowers `+`, `-` and `*` on two Floats to the unchecked Add, Sub and Mul, `/` and
`%` to Div and Mod followed by `PopN(2)`, and a Float negation to the unchecked Neg. The
parser marks two Float operands, scalar kind 5, on the Byte arithmetic record flagged so
the result stays a Float, and flags a Float negation. A float literal is lexed as
`digits.digits`, Tok 73, carrying its interned text, which the host converts with the
`f64` parser the reference lexer uses rather than reimplementing a correctly rounded
conversion in a stage. Codegen interns it with pool tag 3 and emits Neg through a new
transport tag, 69. A suffixed float literal is refused. The boundary table reads 117
cases, 112 SOk, 2 Refuses, 2 Diverges and 1 RefRejects.

**Slice 5d and the AST census, 2026-10-25.** `tests/selfhost_full_language.rs` now holds
the gate 2 census, one program or more for every variant of `Stmt`, `Iterable`, `Expr`,
`Pattern`, `TypeExpr` and `PrimType` in `src/ast.rs`, with its measured outcome pinned. A
second test reads those enums from the source and fails when a variant has no row, so a
variant added later must gain one. The census reads 70 rows, 54 Identical, 13 Unsupported
and 3 RefRejects. The Unsupported rows are the gaps Order 6 still owes, among them
`classify` and `declassify`, the variable, struct and tuple match patterns, newtypes,
generics, `Multiword` and labelled types.

The census found a false source rejection. Inside a checked arm the reference admits the
arm words `saturate_max` and `saturate_min`, which it lexes as keywords and lowers to a
constant of the Word bound, and `parse.kel` reported them as undefined names. The host now
seeds their name ids into two appended token-block slots, and the parser lowers either word
in operand position inside a checked arm to an integer literal of that bound. Both rows are
Identical, and `heartbeat.kel` moved from PipelineRejects to RefRejects, since the only
rejection left is the reference's own, for a host function the RTOS prelude declares. A
name the parser cannot resolve inside a checked arm is now refused as unsupported rather
than as undefined.

Running the whole isolated self-host population for this slice found ten tests that slices
5 to 5c had made stale and that the targeted checks had not reached. Six characterised the
float boundary as it was, and now assert that float literals, arithmetic and negation
compile. Two used a float program as their out-of-subset witness, and now use a newtype
and a struct equality the boundary still records as divergent. The construct scan's
float-literal arm is gone, since it would blame a supported construct. The wire pins moved
to 1,035 names and 31,302 bytes.

**Slice 6, 2026-10-26, struct-typed private data fields.** The reference holds such a field
in one data slot as the whole composite, reads it with `GetData`, and projects it exactly
as a struct local. The parser now marks a private field whose type is an earlier struct
with kind 1024 plus the struct's index, so a read of it continues into the struct
field-access path, and queues the struct's flat size as record 112. From those records the
host places each composite in the private composite pool by slot, as the reference does,
and totals the pool, which it used to assume was empty. A data read also clears the Byte
flag a previous operand left, which had made a Word addition after a cast to `Byte` lower
to the unchecked Byte `Add`. A shared struct field, an array of structs and a struct
declared after its data block stay refused. `14_frame_log.kel` became Identical.

**Slice 7, 2026-10-26, natives that return an enum.** The reference requires every `use`
before the type definitions, so a returned enum is always declared later. The parser now
records each import's declared return type name and types a call's result as the enum
that name resolves to at the call, so a `match` on it binds the variant payload. The host
builds the native shape table after the enum table and gives such a native the enum's flat
shape. A tuple or struct return stays refused. The RTOS task scripts are now also measured
joined with their prelude, as the RTOS crate compiles them, and all five are Unsupported,
four of them only because the prelude declares tuple-returning natives they never call.
A wildcard in a variant's payload position, `Status::Err(_)`, is a separate gap with its
own census row.

**Slice 8, 2026-10-26, tuple returns in `use` signatures, and a time-bound defect.** A
signature may now return a tuple of named types, which the parser sends as one record 135
per element and a closing 136, and the host sizes flat, `(Status, Word)` at twenty-four
bytes as the reference does. A call to such a native stays refused until tuple results are
lowered. The RTOS prelude and four of its five task scripts became Identical.

The fifth, `led.kel`, agreed on every op and not on its execution bound, 93 cycles against
the reference's 114. An enum `match` lowers to a loop whose every arm breaks and whose
no-match path traps, and for such a single-pass loop the reference charges the larger of
the pass and its costliest break, where `analyze.kel` charged the break alone. Every stream
that matched an enum directly was given a bound below the reference's. The shipping
cross-check refused each such program, so none was admitted with the lower bound, but the
self-hosted analysis was unsound on that shape. It now charges as the reference does, and
`led.kel` is Identical. A counted `for` loop with a conditional `break` still gets a bound
above the reference's, 98 against 92, which is the conservative direction and predates this
change. It is the next item.

**A `break` in a bare `for`, 2026-10-26.** The overestimate recorded under slice 8, 98
cycles against 92 on a counted loop with a conditional `break`, was not in the time
analysis. Run on the reference's chunk, `analyze.kel` gives the reference's bound. The ops
differed, and the bound followed them. The parser took any `break` for a `for .. limit`
loop's, so in a bare loop it stored the user-break outcome code into slot 0, which held the
first parameter, and its `Break` was never patched and kept target 0. The shipping
cross-check refused every such program, so none was emitted. A bare loop's `break` is now
marked on its node and lowers to the `Break` alone, which the loop's `EndLoop` patches from
the same break list a `limit` loop uses. Nested bare and `limit` loops each keep their own
breaks, and the census carries a `Stmt::Break::BareLoop` row.
