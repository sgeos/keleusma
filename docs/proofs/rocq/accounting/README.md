# Memory Accounting, an Excluded Case of Increment B2

> **Navigation**: [Increment B2](../region_core/README.md) | [The prose proof](../../COMPOSITE_REGION_REUSE_PROOF.md)

Increment B2 proved that reuse preserves observations and listed memory accounting, Theorem C of
the prose proof's Part I, among the cases it does not cover. Its interpreter does not count
memory, so the saving of reuse was shown on one module and not proved in general. This
directory proves it in general for B2's region core, in the Rocq Prover, version 9.3.0. **For
every module B2's checker accepts and every injective plan whose slots are below `K`, whenever
the reuse run and the baseline run both finish, every cell the reuse run holds is the cell of a
recorded allocation, and in every epoch the reuse run uses at most `K` slots plus one cell for
each allocation at a site the plan does not reuse, and never more cells than the baseline,
which uses one per allocation.** This holds for every host, every fuel and every number of
stream cycles. Every named result is checked with no axiom.

## Why the epoch

The quantity bounded is the per-epoch footprint, the number of distinct cells the allocations of
one epoch use. A cycle of the shipping virtual machine reclaims its bump storage at the reset
that ends it, so the per-epoch footprint is what a worst-case memory bound must cover. The
bound `K + unplanned` does not grow with the number of times a reused site allocates in an
epoch, which is the saving reuse exists for. A loop that allocates at a reused site a thousand
times in one cycle uses one slot.

## The results

| Result | Module | Statement |
|---|---|---|
| `baseline_footprint` | `Accounting.v` | The baseline uses exactly one cell per allocation in each epoch |
| `footprint_le_baseline` | `Accounting.v` | Any plan uses no more cells than the baseline, for any allocation record |
| `footprint_slot_bound` | `Accounting.v` | Theorem C at the core level. A plan with slots below `K` uses at most `K` plus the unplanned allocations, for any allocation record |
| `mem_within` | `Footprint.v` | For every plan, module, host and fuel, accepted or not, every cell holding a value after a run is the cell of a recorded allocation |
| `same_allocations` | `Footprint.v` | For an accepted module and an injective plan, the two runs record the same allocations |
| `reuse_footprint_bounded` | `Footprint.v` | The headline, combining the four above |

The counting in `Accounting.v` refers to no machine, so it holds of any allocation record. The
machine enters through `mem_within`, which ties the memory a run holds to its record, and
through `same_allocations`, which is B2's simulation read for the allocation record instead of
the log.

## How to check it

```sh
eval $(opam env --switch=rocq)
docs/proofs/rocq/accounting/check.sh
```

The script follows the hardened scripts of the other directories. It builds and checks B2's
sources as its dependency, then this directory's four modules. It fails unless no source holds
an admitted or assumed proof, a warning silencer or a `Redirect`, scanned with comments removed
and whitespace collapsed, the pin and control files open no section and declare no hypothesis,
variable or context, every module compiles with no output, `rocqchk` reports no axiom and no
unsafe construct, all 26 named results are closed, and `Statements.v` holds its 8 pins.

`Statements.v` pins the statements of the six results above and of two controls, and fixes the
definitions the conclusions rest on by proved equations, namely `footprint_at`, `cell_at`,
`allocs_at`, `unplanned_at`, `within`, and `indexed` by its two projections. Three planted
defects were run against the script on 2026-11-18, in copies under `tmp/`. An `Admitted` lemma
was refused by the scan. A premise added to `reuse_footprint_bounded`, with `Controls.v` patched
to supply it, was refused at its pin in `Statements.v`. A change to the definition of
`footprint_at` was refused, though by a proof in `Accounting.v` that no longer compiled and not
by its pinned equation, so a definition change that keeps every proof compiling was not
demonstrated against the equations. No log of these runs is kept in the tree. Continuous
integration does not yet run the script, under the operator's ruling that a new
continuous-integration job waits for the release or for agreement with the V0.2.X line.

## The controls

- B2's accepted loop `loop_ok` allocates at site 0 three times in each of two cycles. Under the
  plan `P0`, which reuses site 0 in slot 0, each epoch uses one cell, while the baseline uses
  three, so the saving is strict. `loop_ok_bound_is_tight` shows the bound `K + unplanned` is
  met with equality there.
- `slots_alone_do_not_bound` shows that the baseline run of the same module uses three cells in
  an epoch with no slot at all, so the term counting unplanned allocations is needed.
- `loop_ok_footprint_bounded` applies the headline to `loop_ok` under `P0`, proved injective,
  so its premises are satisfiable.

## The trusted base

The Rocq kernel, `rocqchk`, the standard-library modules `List`, `Arith` and `Lia`, B2's
definitions, and the statements and pinned definitions as written. The pins fix the definitions
named above. They do not fix B2's own definitions, among them `cycles`, `new_step`, `cell_of`,
`region_check` and `injective`, which B2's directory documents.

## What is not covered

- **Keleusma itself.** The region core is a model, and nothing here relates it to the shipping
  virtual machine, its arena or its verifier's worst-case memory figure.
- **Reclamation.** The region core never frees a cell. The per-epoch footprint models what a
  cycle uses on the assumption that the reset reclaims bump storage, which the shipping arena
  does and this model does not state.
- **Sizes.** A region here holds one number, so every cell has the same size, and the bound
  counts cells, not bytes. The prose proof's Theorem C provisions a shared slot at the size of
  its largest member, which is not modelled.
- **Shared slots.** The headline needs an injective plan, because `same_allocations` rests on
  B2's simulation. The counting lemmas do not, so extending the headline to the plans of
  [`../shared_slots/`](../shared_slots/README.md) needs only that directory's simulation read
  the same way.
- **The other exclusions of B2.** Refined confinement, the escape-copy discipline, sites in
  called functions, termination and underflow remain as B2 lists them.
