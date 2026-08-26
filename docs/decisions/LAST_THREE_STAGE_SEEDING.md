# What `analyze.kel`, `codegen.kel`, and `verify_yield.kel` actually need to be seeded

**Status**: requirements established, **no stage seeded**. Written by the `v0.3.X` line, 2026-08-26.

**Scope**: `src/selfhost/` is the `v0.2.3` line's and is read-only here. Nothing in this document is
a change to it. Every fact below is read out of their sources or measured against this tree.

## Why this document exists

The handoff's instruction, verbatim: *"Do not assume the generic slot route reaches them just
because it reached four others. Establish what each actually needs before planning an increment
around it."* The three are the Order-1 gate's self-declared weakest part — each is currently one run
of sixty ticks with **no input at all**.

## The headline, and it points against the cautious framing

**All three are reachable by the name-resolved shared-slot route, and none of them requires an
accessor from `src/selfhost/mod.rs`.** The caution was correct to demand establishment; the
establishment says the route reaches them.

The concern recorded earlier was that these three take *marshalled module structure* rather than a
synthetic table, and that the accessors for that live in a read-only file. **Measured, the structure
they take is a set of NAMED SLOTS in an ordinary `shared data` block**, which is exactly what the
generic route already writes for `lexer.kel` and `parse.kel`.

## Per stage

### `analyze.kel`

`shared data wa`: eight scalars (`op_count`, `stream_pos`, `reset_pos`, `local_count`,
`value_slot_bytes`, `arena_capacity`, `region_start`, `region_end`) and nine parallel
`[Word; 1536]` op tables (`cost`, `class`, `arg`, `growth`, `shrink`, `heap`, `opk`, `slot`,
`cval`).

**THE ENCODING IS DOCUMENTED IN THE FILE'S OWN HEADER**, not in the accessors:

- `class` — control-flow role: `0` plain, `1` If (`arg` = branch target), `2` Else (`arg` = matching
  EndIf), `3` EndIf, `4` Loop (`arg` = exit target), `5` EndLoop, `6` Break, `7` BreakIf, `8` Trap.
- `opk` — fine opcode kind for bound extraction: `1` GetLocal, `2` SetLocal, `3` Const, `4` CmpGe,
  `5` BreakIf, `6` CheckedAdd, `7` PopN, `8` EndLoop, `9` Loop, `0` other.
- `slot` — the GetLocal/SetLocal slot; `cval` — the Const integer or PopN count; `cint` — 1 when a
  Const resolves to an integer; `cost` — WCET cycles; `growth`/`shrink` — operand slots pushed and
  popped; `heap` — heap bytes.

**So a seed builder needs the reference `Op` table and this enumeration. Both are available here.**

### `verify_yield.kel`

`shared data yv`: `op_count`, `region_start`, `region_end`, the tables `class`, `arg`, `mark`, `cay`,
and the outputs `out_fell`, `out_hy`. Plus a frame block (`sp`, and `[Word; 128]` frame arrays).

`class` is documented as **"as in `analyze_class`"**, `mark` as **1 for Yield**, and `cay` as the
already-always-yielding fixpoint variable. **So it needs the same enumeration as `analyze.kel` plus
two one-bit marks** — the smallest requirement of the three.

### `codegen.kel`

Carries its op encoding **inline**, in a `const data wire { konst: Word = 1, ret: Word = 2, ... }`
block, with the header stating the tags **match the reference bytecode's opcodes**. So its encoding
is readable as text and cross-checkable against `keleusma::Op` rather than taken on trust.

## THE REAL CONSTRAINT IS THE STEP BUDGET, AND IT IS NOT THE SLOT ROUTE

**Measured caps against measured harness budgets:**

| | own step cap | |
|---|---|---|
| `analyze.kel` | **16384** | `for step in 0..16384 limit 16384` |
| `verify_yield.kel` | **8192** | `for step in 0..8192 limit 8192` |
| `codegen.kel` | no single driver cap; inner loops at `limit 64` | |

| harness | ticks |
|---|---|
| `corpus_differential` | **60** |
| `stage_differential` | **400** |

**A CAP IS NOT A COST.** This exact trap is already recorded for `verify_types.kel`, whose
`ty_max_steps()` is 1801 while a fold over `k` populated rows finishes in about `k + 9`. The three
subjects there are small *for that published reason*, not arbitrarily.

**So the requirement is: the subject must be sized so its verdict is REACHED, and the seed builder
must REQUIRE the verdict rather than assume it.** A run that stops early compares a prefix of a fold
while reporting as seeded — the truncated-fold failure this line has already made once.

**Recommended route: `stage_differential`, not `corpus_differential`** — 400 ticks against 60, a
6.7x budget, and it is the harness that already seeds `lexer.kel` and `parse.kel` by name resolution
with nothing needed from the other line.

## ⚠ A FINDING FOR THE `v0.2.3` LINE: TWO HAND-MAINTAINED TAG TABLES, GUARDED ONLY FOR TOTALITY

`codegen.kel`'s `const data wire { konst: Word = 1, ... }` numbers 63 op tags. `decode_op` in
`src/selfhost/mod.rs` maps those same tags back to `Op`. **They are two independently maintained
tables of the same 63 numbers**, one in Keleusma source and one in Rust.

**The existing guard does not cover the drift.** `all_wire_op_tags_decode` reads, in full:

```rust
for tag in 1..=63i64 {
    // operand 0 is the minimal representative word for each tag; decode must not panic.
    let _ = decode_op(tag);
}
```

**That asserts decode is TOTAL over the band. It does not assert that tag `N` means the same opcode
on both sides.** A transposition — two tags swapped in either table — passes it. The comment beside
`decode_op` describes the test as guarding "against drift", and against *absence* it does; against
*disagreement* it does not.

**AND NOTHING ELSE COVERS IT EITHER, because `codegen.kel` is UNSEEDED.** It runs in the Order-1
gate as sixty ticks with no input, so the differential never exercises the tags. The one stage whose
correctness depends on this table is the one stage nothing drives.

**This line cannot close it.** `decode_op` is private and `src/selfhost/mod.rs` is read-only here.
The check needs either `decode_op` reachable, or the assertion written on their side as
`decode_op(tag) == expected_op_for(tag)` over a table derived from the `.kel` text.

> **Stated as a finding, not a defect.** No disagreement has been observed — the tables have not
> been compared, which is the whole point. **The claim is about what is CHECKED, not about what is
> WRONG**, and those are separated deliberately.

## What is NOT established here

- **That any seed actually drives any of the three to a non-trivial verdict.** No stage was seeded.
- **Whether the 1536-entry tables fit the harness's shared-segment handling in practice.** The
  shared ceiling is 16 MB and nine tables of 1536 words is far below it, so no obstacle is expected
  — but expected is not measured.
- **Whether the two tag tables agree.** See the finding above: not checkable from this line.
