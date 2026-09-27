# MILESTONE 1 OF THE V0.3.X ROADMAP IS NOT MET, AND THIS IS WHAT IT NEEDS

**Measured 2026-09-27.** No document in the tree claimed this milestone before or after; this
one states where it stands so the next session does not have to re-derive it.

## The gate

`docs/roadmap/V0_3_X_ROADMAP.md` order 1: *"Subset bytecode lowers to native — Workstream A
(first pass) — the self-hosted compiler's own bytecode runs correctly as native code,
differential-tested against the VM."*

## What is true

The self-hosted compiler's twelve stage sources under `src/selfhost/kel/` are in the
differential corpus, they lower, and they agree with the virtual machine. **That much is
real and is not being diminished here.**

## Why the gate is not met

**Agreement on degenerate input is not the gate.** Measured by
`probe_stage_vacuity::how_far_does_each_stage_get_on_the_differentials_own_input`, over 60
ticks each:

| | count |
|---|---|
| stage sources | **12** |
| yielding a SINGLE repeated value under the corpus drive | **8 of 12** |
| driven on a meaningfully SEEDED shared segment | **2 of 12** |
| reporting `seeded: no len/bytes pair` | **10 of 12** |

The two that are seeded are `lexer.kel`, which produces 10 distinct token codes instead of
one, and `wire.kel` at 4. A tokenizer emitting one value for sixty ticks has reached end of
source immediately: `lexer.kel` documents 62 as end-of-source and 63 as pending, and the
corpus drive yields `62` sixty times.

**So eleven or ten of twelve stages are verified against the VM on input that exercises one
path.** The differential is sound; its INPUT is not representative.

## What would close it, concretely

The seeder understands one convention — `src.bytes` with its length in `src.len`, which is
`lexer.kel`'s. Every other stage consumes a differently named shared block, and the probe
already prints each one's leading slots:

| stage | block prefix |
|---|---|
| `analyze.kel` | `wa.op_count`, `wa.stream_pos`, `wa.reset_pos`, `wa.local_count`, … |
| `codegen.kel` | `ast.root`, `ast.kinds`, … |
| `parse.kel` | `toks.len`, `toks.packed`, … |
| `reconstruct.kel` | `io.rec_count`, `io.in_category`, `io.in_param_count`, … |
| `verify_datalayout.kel` | `dl.phase`, `dl.count`, `dl.n_slots`, `dl.buffer`, `dl.pool`, … |
| `verify_depth.kel` | `dv.op_count`, `dv.class`, … |
| `verify_structural.kel` | `sv.op_count`, `sv.local_count`, `sv.const_count`, … |
| `verify_typed.kel` | `tv.op_count`, `tv.resume_tag`, `tv.resume_size`, … |
| `verify_types.kel` | `ty.cmd`, `ty.verdict`, `ty.n`, `ty.lhs`, … |
| `verify_yield.kel` | `yv.op_count`, `yv.region_start`, `yv.region_end`, `yv.class`, … |

**Ten per-stage shared-block seeders is the work**, and the slot names are already measured
rather than guessed. `selfhost_host.rs` carries the layouts the stages are seeded through,
which is where the conventions live.

## Wrong turns

**Do not claim the milestone on a count of agreeing modules.** That is the error this
document exists to prevent: nine or ten stages "executing and agreeing" reads like coverage
and is agreement on a zero segment.

**Do not seed a stage with arbitrary bytes to move its distinct-value count.** A seeder that
produces values the stage would never receive proves the two implementations agree on
nonsense. The seeded input must satisfy the stage's documented contract, which is why the
slot layouts matter.

**Do not treat `wire.kel` as solved because it shows 4 distinct seeded values.** Its
zero-input run FAILED — `resume failed at tick 19: IndexOutOfBounds(1570812, 65536)` — so its
seeded column describes a different situation from the others and deserves its own look.

## A stale figure corrected alongside this

`stage_differential.rs` said "nine of the **ten** stage sources". There are **twelve**, and
`probe_stage_vacuity` already reports 12 correctly. The doc comment had not followed the two
stages added since it was written.
