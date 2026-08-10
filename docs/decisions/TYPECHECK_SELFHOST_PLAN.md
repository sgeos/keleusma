# Self-Hosting the Type Checker and the Monomorphizer — Scoping

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Scoping for the two Order-1 blockers that had no plan document. Written 2026-08-09.

Status: **SCOPED. Spikes A and B are both RUN and RESOLVED, and both Order-1 obligations shrank.**

**Spike A's result: monomorphization is an identity transform on all ten stage sources**, so the
first-pass monomorphizer obligation is effectively empty. Pinned permanently by
`tests/selfhost_monomorphize_identity.rs`, with a must-fire control proving the comparison
detects a change when one occurs. **One of the roadmap's three Order-1 blockers is therefore
retired for the first pass**, and the roadmap's implication that the three are comparable in
size is wrong.

## Why this exists

[`../roadmap/V0_2_X_ROADMAP.md`](../roadmap/V0_2_X_ROADMAP.md) states the Order-1 gate and names
exactly three things standing in the way:

> The gate is NOT met while **the type checker, the monomorphizer, and wire-format
> serialization** remain in Rust — those three are the whole of what stands between here and
> Order 1.

Wire-format serialization has [`WIRE_FORMAT_SELFHOST_PLAN.md`](./WIRE_FORMAT_SELFHOST_PLAN.md) and
is in progress. The other two had nothing. The roadmap sentence also implies the three are
comparable in size, and the evidence below suggests they are not.

## Finding 1 — the subset uses no generics, so the monomorphizer has nothing to do

The Order-1 gate is about the **first pass**, which the roadmap defines as "only the subset the
toolchain's own source is written in". Measured across all ten sources in `src/selfhost/kel/`:

| Construct | Occurrences |
|---|---|
| Generic function declarations `fn name<...>` | **0** |
| Generic struct or enum declarations | **0** |
| `trait` | 3, **all comments in `parse.kel`** describing how the parser skips them |
| `impl` | 1, likewise a comment |
| Const generic parameters | **0** (the 18 `const` hits are `const data` blocks) |

`src/monomorphize.rs` is 2287 lines, but for a program with nothing to instantiate the pass is an
identity transform. **The first-pass cost is therefore not a port of those lines.**

This is a lead, not a conclusion. Spike A settles it.

## Finding 2 — the emitter's dependency on inferred types is narrower than expected

This was the item the earlier scoping flagged as the real unknown and said to measure before
planning. Most of it turns out to be answerable by reading, and the answer is favourable.

The reference pipeline runs check → monomorphize → **check again with recording**
(`src/compiler.rs:3080`, `3091`, `3099`). The recording pass sets `ctx.record_types` and fills
`program.fn_expr_types`, a per-function map from expression span to resolved type.

**That table has exactly one consumption site**: `infer_expr_type` in `src/compiler.rs:5742`. Its
purpose, per the comment at `3095`, is **flat-access baking** (B28 P3 item 5).

And it is a *preference*, not a requirement:

```rust
fn infer_expr_type(fc: &FuncCompiler, expr: &Expr) -> Option<TypeExpr> {
    // Consult the authoritative per-function type table first ...
    if let Some(ty) = fc.expr_types.get(&expr.span()) {
        return Some(ty.clone());
    }
    match expr { /* structural inference from the AST */ }
}
```

The fallback is a defined structural path: `StructInit` yields its named type, `Call` looks up
`function_returns`, `Ident` looks up the local's type, `FieldAccess` looks up the struct field, and
so on. The call site at `5282` says as much: the table is "absent for a group the recording pass
did not table, in which case the structural inference path runs".

**What the self-hosted pipeline actually carries is declared type spellings, not inferred types.**
`assemble_chunk_metadata` (`src/selfhost/mod.rs:2347`) resolves a parameter's `type_id` by matching
the interned name against the literal strings `"Word"` and `"Byte"`. `src/selfhost/mod.rs` contains
no reference to `typecheck` or `monomorphize` at all.

So the self-hosted compiler already reaches byte identity on the ten stages **without any
inference**, which is consistent with the structural path sufficing for that subset.

### The two obligations, separated

1. **Rejection.** The self-hosted compiler compiles well-typed subset programs correctly but would
   not reject an ill-typed one. Closing this needs enough checking in Keleusma to reproduce the
   reference's verdict on a corpus of bad programs. The oracle is verdict-agreement, exactly the
   shape the `verify_*.kel` family already uses for `verify()` — that family is the precedent to
   copy rather than a new design.
2. **Inference sufficient for emission.** Bounded by Finding 2 to *one* site and *one* purpose.
   Spike B measures whether it binds at all for the subset.

## The two spikes, neither yet run

Both are cheap, both have an existing oracle, and both are decisive rather than indicative.

### Spike A — is the monomorphizer an identity on the subset?

**Method.** For each of the ten stage sources, parse, then compare the AST before and after
`monomorphize_with_provenance`. Equality of the serialized AST, or of the compiled module bytes
with the pass skipped, is the signal.

**Decides.** Whether the Order-1 monomorphizer is a near-empty obligation or a real port. If it is
an identity, the roadmap's Order-1 row should say so, because it currently implies otherwise.

**Cost.** One test, minutes.

**RESULT (2026-08-09): identity on all ten stages.** Compared with `Program`'s own `PartialEq`
after the same pre-monomorphization type check the pipeline performs. The must-fire control — a
generic program, which the same comparison does report as changed — is what makes the result
meaningful rather than a comparison that always returns equal. Promoted from a spike to a
standing test, because the property belongs to the stage sources and ends the day one of them
declares a generic.

### Spike B — does the emitter need the recorded type table for the subset?

**Method.** Force `program.fn_expr_types` empty between the recording pass and emission — one line
— and re-run the existing byte-identity corpus over the ten stages.

**RESULT (2026-08-09): UNCHANGED. The emitter does not need the table for the subset.**

Clearing `program.fn_expr_types` between the recording pass and emission leaves **all ten stage
modules byte-identical**, and leaves ten deliberately composite-heavy probe programs byte-identical
too: nested structs, arrays of structs, a struct in a tuple, an enum struct payload, a function
returning a struct, and a match binding its payload.

**The scoping overstated the cost, which is worth correcting.** This does NOT need the self-host
corpus. The question is whether the *reference emitter's* output changes, which is ten reference
compiles taking seconds rather than the Keleusma-written pipeline. The "needs a quiet machine"
caution applied to a larger experiment than the one actually required.

**Three controls, because "no change" is also what a broken intervention looks like:**

1. **The table is populated.** Replaying the pipeline by hand and counting gives **27,290 recorded
   entries** across the ten stages, so clearing it is a real intervention rather than a no-op.
2. **The table is consulted.** Instrumenting the single consumption site counted **322 hits and
   zero misses** while compiling `lexer.kel`, and 2 and 3 hits on the small cases. Every call to
   `infer_expr_type` found an entry, so the branch is live and the clear certainly reaches it.
3. **The digest discriminates.** The first attempt hashed each artifact with `crc32` and produced
   the SAME value for all ten modules despite wildly different lengths — because a CRC over a
   message with its own CRC trailer appended is a fixed residue. That is a real property of the
   format and a degenerate digest. Replaced with FNV-1a, which yields ten distinct values. Left
   unnoticed, the spike would have "passed" while measuring nothing at all.

**Conclusion.** The recorded table is consulted constantly, and its answers **agree with the
structural fallback everywhere the subset reaches**. For Order 1, obligation 2 is therefore empty
and **the self-hosted type checker reduces to obligation 1, rejection, alone.**

**What this does not show.** No program was found where the two paths differ, which is not proof
that none exists. The search was ten hand-picked candidates plus the ten stages, not an exhaustive
one, and the full language of Order 6 may well contain such a case.

**A caveat to state up front.** Byte identity on the stages shows the two paths *agree there*, not
that they agree in general. Spike B measures the subset, which is precisely what Order 1 is scoped
to, and says nothing about the full language of Order 6.

## Obligation 1 sized by execution, 2026-08-10

Reading gives 163 `TypeError::new` sites in `src/typecheck.rs`, of which about twenty mention
traits, bounds or impls and are therefore **outside Order-1 scope**, since the subset uses none of
them. But the number that matters is how many an ill-typed program in the SUBSET can actually reach,
and that needs execution rather than counting.

**Eighteen ill-typed subset programs, plus five well-typed controls.** Seventeen were rejected and
all five controls accepted.

| Rejected construct | Diagnostic |
|---|---|
| `1 + true` | cannot add Word and bool |
| body type versus declared return | function `main` returns Word but body produces bool |
| wrong argument type | argument to `f` expects Word, got bool |
| too few / too many arguments | function `f` expects N arguments, got M |
| undefined function / identifier | undefined function `nope` / undefined identifier `nope` |
| `if` branches differ | if branches have differing types Word and bool |
| non-bool condition | if condition must be bool, got Word |
| unknown field | type P has no field `b` |
| wrong field count | struct `P` expects 2 fields, got 1 |
| index a scalar | array index on non-array type Word |
| field access on a scalar | field access on non-struct type Word |
| `Byte` against `Word` argument | argument to `f` expects Byte, got Word |
| array elements differ | array elements have differing types Word and bool |
| calling a local | not a callable; V0.2.0 admits only direct calls |

**Three things this settles.**

1. **The obligation is an enumerable list of about fifteen shapes, not 163 sites.** That is a
   tractable target for a `.kel` checker and a corpus, and it is much smaller than the roadmap's
   framing implies.
2. **Every subset rejection is delivered at one place** — the type-check pass inside `compile` —
   and all but one carry the `type error:` prefix. The exception is "calling a local", which is a
   V0.2.0 surface restriction rather than a type error, so a self-hosted checker must reject it
   without being able to find it among the type diagnostics.
3. **The oracle is verdict agreement, not message agreement.** Accept-versus-reject is what the
   `verify_*.kel` family already uses, and it avoids committing the self-hosted checker to
   reproducing English.

**A mistake in my own corpus, recorded because it is the kind that inflates a result.** One case I
labelled ill-typed — a `Word` field read from a shared data block — is perfectly well-typed, and it
was reported as "accepted but should not be". It is a badly-constructed test, not a compiler defect.
The reason it did not mislead is that the corpus carried explicit well-typed controls, so an
unexpected acceptance had somewhere to be checked against.

## Suggested ordering

1. **Finish wire-format serialization**, slices 5 to 7. In progress, sliced, harness built.
2. ~~**Spike A.**~~ **Done. The monomorphizer obligation is empty.**
3. ~~**Spike B.**~~ **Done. The inference obligation is empty too.**
4. **Then write the implementation plan**, sliced the way the wire-format plan is, with rejection
   and inference as separate tracks and their real sizes known rather than guessed.

## Caveats

- Everything above is from reading. The counts come from `grep` over the stage sources with the
  obvious spellings (`fn name<`, `struct name<`), which would miss a construct spelled unusually.
- **None of this reduces the full-language work** for Order 6 / V0.3.0, where generics, traits and
  bounds are needed in full. It bounds Order 1 only.
- `src/typecheck.rs` is 8601 lines, of which a large share serves traits and bounds (110 mentions
  of `trait`, 104 of `Bound`). The Hindley-Milner core, `unify` and `Subst`, is not avoidable the
  way monomorphization appears to be, but obligation 1 may not need the whole of it.
