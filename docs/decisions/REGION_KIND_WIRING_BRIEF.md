# Brief: wiring SHARED_LAYOUT and DATA_INIT into the windowed driver

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: implemented 2026-08-31. The goal itself is the operator's queued item, not mine.

**Outcome**: `SHARED_LAYOUT` routed for every stage and byte-matching the reference. `DATA_INIT`
routed for the eleven stages that elide. Skipped region kinds **6 -> 5**.

## What this is

The self-hosted driver assembles a stage's auxiliary body region by region. Six region kinds fall
into a `_ => continue` and are left as ZEROS. Two of them, `SHARED_LAYOUT` and `DATA_INIT`, carry no
name index, which is what makes them driver-only work: the other four wait on the undriven
`intern_index_of` route.

**The stage is not the gap.** `emit_at` in `wire.kel` already dispatches `emit_shared_slot_records`
and `emit_data_init_records`. What is missing is the driver supplying their fields.

## What was already established, so it is not re-derived

- The generic route is command 164, `emit_in_window`, reading `wire.warg` = kind, `warg2` = count,
  `warg3` = window offset, and per-record fields from `wire.fin`.
- **`Module.data_layout` carries `shared_layout` and `private_init` directly.** No layout
  computation is needed; the encoder uses the same structure.
- The run-grouping algorithm to mirror is in `src/wire_schema.rs`: a run is consecutive entries with
  equal `kind` and `len` whose `offset` advances by a constant delta, formed only when that delta
  fits `u16`, then chunked at `u16::MAX` slots. Seven fields per record.

## The two risks, both named before any code

**One: the elision, and it is the dangerous one.** `DataInitRecord` is trivially two `u32` fields.
The hazard is that `add_data_layout` ELIDES a wholly-default private-init pool, and a consumer
producing the region another way must predict that identically. **Call
`wire_schema::private_init_is_elided` rather than re-deriving the condition.** That predicate exists
precisely so the two sides cannot drift, and its own comment says a disagreement here "is not a
state anything would notice". Note its subtlety: only the WHOLLY default case is elided, and an
EMPTY pool is deliberately not, so the `ABSENT` sentinel keeps one meaning.

**Two: turning a gap into a difference.** `emit_shared_slot_records` refuses with `-204` when
`n * 7` overflows `fin`. That refusal is the HONEST outcome. A driver that wrote `&win[..len]` and
silently emitted fewer records than the reference reserved would convert a clean `Skipped` into a
`Differs` -- and `tests/selfhost_region_coverage.rs` separates those two precisely so a gap is not
mistaken for a defect. **Check the emitted length against the reserved length and error**, as the
CONSTS and SHAPES paths already do.

## Prior failures to avoid

**Do not size this by reading.** Three wrong sizings on this line came from reasoning about a
structure instead of reading its producer and consumer. The layout question above was answered by
finding `Module.data_layout`, not by inferring what the driver "must" need.

**A refusal is not a success.** If a stage's shared layout exceeds the batch bound, the honest
result is that the kind remains unreached for that stage. Report it; do not soften it.

**Do not chase the other four kinds.** They need the `intern_index_of` route, which is a separate
increment. Widening scope mid-increment is how the boundary between "done" and "partly done" gets
lost.

## MEASURED 2026-08-31, AND IT CORRECTS THIS BRIEF

Both risks above were sized by reading. Measuring them changed one and shrank the other.

| stage | shared slots | private-init elided | SHARED_LAYOUT records after grouping |
|---|---:|---|---:|
| `lexer.kel` | 395,778 | true | **9** |
| `wire.kel` | 144,391 | true | **8** |
| `verify_typed.kel` | 56,134 | true | 1 |
| every other stage | 3,084 - 41,997 | true | 1 |
| `verify_datalayout.kel` | 3,084 | **false** | 1 |

**THE BATCH BOUND IS NOT THE LIVE CONSTRAINT, AND THIS BRIEF SAID IT WAS.** The run grouping is
enormously effective: 395,778 slots collapse to nine records, because a shared layout is
overwhelmingly uniform arrays. Nine records is 63 field words against a `fin` capacity in the
thousands. **No corpus stage comes close to the bound**, and the `-204` refusal path, while still
worth writing correctly, will not fire on anything currently measured.

The correction matters because the brief planned around a constraint that does not bind, and would
have spent the increment's care in the wrong place.

**DATA_INIT IS VERY NEARLY TRIVIAL.** Eleven of twelve stages elide, so the record is the `ABSENT`
sentinel and a count. The twelfth, `verify_datalayout.kel`, reports NOT elided with an EMPTY pool --
which is the predicate behaving exactly as documented, since only the wholly-default NON-EMPTY case
elides and an empty pool must keep the sentinel meaning one thing. **That single stage is the whole
of the non-sentinel path in the corpus**, and it is the case a hand-written condition would most
plausibly get wrong, which is precisely why the shared predicate must be called rather than
reimplemented.

## What "done" is not

It is not "the code compiles and the tests still pass" -- the kinds were already being skipped
silently, so a no-op change passes everything. Done is the region-coverage measurement MOVING, and
the movement being visible in the tree.
