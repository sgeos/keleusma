# Body-into-parser merge plan (roadmap Step 2)

Merging the body-expression parser (`kel/body.kel`) into the declaration parser
(`kel/parser.kel`) so one streaming `loop` parses a whole top-level declaration
including its function body in a single pass. This retires the throwaway body
adapter and is the critical-path step toward a self-hosting parser stage. The body
parser is complete through increment 25 (the block-form statement family), so the
merge is now unblocked.

## Load-bearing decisions

**Name resolution: incremental table accumulation (strategy B).** At parse time the
program is not yet compiled, so the numeric tables `body.kel` consumes do not exist.
Rather than change body.kel's emission to carry names (strategy A, which would force
a coordinated cross-stage change to the codegen input and cannot land as one green
increment), the merged parser ACCUMULATES the tables as it parses earlier
declarations, keeping body.kel's emission byte-identical:

- Param table: exactly the `PARAM` elements the header scan already extracts, per
  function.
- Field-layout table (`fdata`/`ffield`/`fbase`/`flen`): a running prefix-sum over
  data-block field widths (scalar width 1, array `[T; N]` width N from `ASIZE`),
  accumulated as each `data` block closes. Data blocks are top-level and precede the
  bodies that read them.
- Enum table (`edata`/`evar`/`edisc`): accumulated from `enum` declarations, tracking
  the implicit-increment discriminant rule.
- Chunk-name table: the `START` name ids in declaration order; wires in last and may
  stay host-supplied longest (it is resolved-reference data, and mutual recursion
  needs the full list).

**New file `compiler/kel/parse.kel` and new harness `tests/selfhost_parse.rs`.** The
combined stage is grown construct by construct, exactly as codegen.kel was. parser.kel
and body.kel stay green under their existing harnesses throughout the merge and are
deleted only in the final cleanup increment.

**Unified token vocabulary.** body.kel's `enum Tok` is the superset (it already names
the operators, `::`=51, `[`=41, `]`=42, `as`=52). Extend it with the declaration-only
kinds (`Fn`, `Loop`, `Shared`, `Private`, `Const`, `Data`, `Use`, `Enum`). The moved
header logic is re-coded from parser.kel's codes (`[`=11, `]`=18, `::`=20, enum kw=21)
to the unified codes. This re-coding is the highest-risk mechanical part.

**Coexistence structure.** One combined `step()` with a top-level `in_body` flag:
`if pctrl.in_body == 1 { body_step() } else { header_step() }`. header_step is
parser.kel's mode machine minus the mode-3 brace-match; where it saw the body-opening
`{` it instead arms the body (reset the 8 body private blocks, seed the param table,
push the `{` back one cursor step, set `in_body`, emit `BSTART`). body_step is
body.kel's `step()`; when it yields `Node::Done` the driver emits `BEND`, clears
`in_body`, and returns to the header for the next declaration. Keep header_step and
body_step as separate `fn`s so the top-level `step()` stays a 2-way branch (depth).

**Token input.** One shared `toks` stream and one cursor (`ps.cursor`); the moved body
steps are re-pointed from `src.kinds`/`src.vals`/`ctrl.cursor` to
`toks.kinds`/`toks.vals`/`ps.cursor`.

**Record wire.** Header records stay `dkind + val*16`; body records stay
`kind + arg*64`. The `BSTART`/`BEND` bracket records switch the host decoder's mode
between the two, so both encodings stay byte-identical.

## Increment sequence (each independently green on the full gate)

1. Skeleton merge on the simplest body (atomic literal or param-ref). Functions only
   in the header. Proves the architecture: combined state, `in_body` dispatch,
   `BSTART`/`BEND`, param table seeded from the header. New harness.
2. Body operator grammar (shunting-yard, unary, parens, bitwise, short-circuit). No
   tables. First depth/stack risk.
3. `let` blocks and the statement fold.
4. `if`/`else` and nested branch blocks.
5. `for`/`match`/`yield` expression forms.
6. Data blocks in the header + field-table accumulation (prefix-sum over ASIZE widths).
7. Scalar and indexed data reads/writes in the body.
8. Calls + chunk table.
9. Enums in the header + enum-table accumulation, then enum casts/patterns in the body.
10. `use` imports.
11. Cleanup: delete parser.kel, body.kel, and their harnesses; update MILESTONES.

Increments 6-10 are re-orderable; a table's accumulation (header) must land before or
with its consumption (body).

## Highest-risk parts

1. The `{`-handoff cursor discipline between header and body (increment 1, off-by-one
   silently drops or double-reads the `{`).
2. Token-vocabulary re-coding (increment 2, when operators first appear).
3. Field-table prefix-sum correctness from ASIZE widths (increment 6), matching the
   reference `data_layout.slots` order exactly.
4. Mutual-recursion chunk indices (increment 8).

### Refinement: increment 1 needs no token re-coding

The plan originally rated token re-coding as increment 1's highest risk. On inspection
the two vocabularies already agree on every token an atomic body uses: Ident, IntLit,
`{`, `}` are 1, 12, 2, 3 in parser.kel and `Ident`=1, `IntLit`=12, `LBrace`=2,
`RBrace`=3 in body.kel's `enum Tok`. They diverge only on operators and brackets
(`+`=21 in body vs `enum`=21 in parser, `[`=41 vs 11, `]`=42 vs 18, `::`=51 vs 20),
none of which occur in an atomic body. So increment 1 moves the header verbatim (no
re-coding) and adds the atomic body walk against the shared codes; the vocabulary
reconciliation moves to increment 2, where operators first appear. This makes the
`{`-handoff cursor discipline, not transcription, increment 1's dominant risk.

## Depth and stack

`MAX_PARSE_DEPTH = 24` is per-`fn`; keep every combined `fn` shallow (header_step and
body_step separate, `step()` a 2-way branch). Bites first at increment 2, worst at
4-5. The host compiles the combined source on the 64MB thread from increment 1 (the
whole file is deeper than either original).
