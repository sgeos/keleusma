# Self-Hosting Wire-Format Serialization — Scoping

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Scoping for the Order-1 residual "self-host wire-format serialization". Probed 2026-08-03,
**re-scoped 2026-08-08**.

Status: **UNBLOCKED. The blocker below was removed by changing the wire format, which is what the
six-step wire-format programme did.**

## 2026-08-08: the blocker is gone. Read this before the 2026-08-03 text.

The analysis below concluded that full self-hosting was out of reach because the auxiliary body was
an rkyv archive, that reproducing rkyv's byte layout in Keleusma was disproportionate and fragile,
and that proceeding needed an operator decision to change the wire format so the auxiliary body used
an encoding the self-hosted compiler could produce.

**That decision was taken and executed.** The auxiliary body is now the wire format v2 container,
specified in [`../spec/WIRE_FORMAT.md`](../spec/WIRE_FORMAT.md) and implemented by the standalone
`keleusma-wire` crate. It was designed for this purpose: no recursion, statically bounded loops, no
allocation on the read path, fixed-size records, unrolled place-value field access, no traits or
generics in the codec core, and state in explicit structs. `BYTECODE_VERSION` moved to 2 with
operator authorization on 2026-08-06.

So the recommendation below to defer this item and prefer the monomorphizer and type checker is
**withdrawn**. The row reading "Auxiliary body — `rkyv::to_bytes` — NO" is obsolete; every region is
now self-hostable in principle.

### Re-slicing against the v2 container

Smallest first, each independently verifiable against the Rust implementation by byte identity.
Slice 1 is unchanged from the original plan and is still the right place to start, because it
establishes the byte-emission harness the rest depends on.

1. ~~**CRC-32.**~~ **DONE 2026-08-09.** `src/selfhost/kel/wire.kel` plus the differential in
   `tests/selfhost_wire.rs`. See [Slice 1 as built](#slice-1-as-built-2026-08-09) below.
2. **Container primitives and the prologue.** Little-endian place-value writers and readers, the
   16-byte prologue, and the majority-of-three vote over its three copies. Oracle: the bytes
   `keleusma-wire` emits for the same input.
3. **The region directory.** Emission and lookup, triplicated and voted, word-indexed offsets.
4. **Record tables and byte pools.** Fixed-stride addressing, which is the shift-not-multiply case
   the format was shaped around.
5. **The schema layer.** The twenty region kinds and their record shapes.
6. **The opcode stream and operand pool.** The meaty part. `codegen.kel` already carries an internal
   op encoding whose tag values ARE the opcode ids, so the mapping to four-byte records is close to
   what it already computes. Verify against `encode_op` over every op form, including pool spills.
7. **The framing header.** 64 fixed bytes, needing the region lengths from the slices above.

### Constraints to carry into the implementation

- **Both directions are in scope.** The operator resolved on 2026-08-04 that self-hosting the wire
  format covers the encoder **and** the decoder.
- **`ConstTable::value` is NOT transliterable as written.** Added 2026-08-08, it uses `BTreeSet` and
  `BTreeMap` to walk one constant's reachable set. That is correct for the Rust VM and unavailable
  in Keleusma. The Keleusma decoder needs a bounded array-based walk instead. The forward-ordering
  invariant is what makes such a walk terminating, so the shape exists; it simply has to be written
  differently rather than transliterated.
- **Only a composite constant record carries a child range.** A scalar overlays its payload on those
  bytes. Getting this backwards reads an integer constant's value as a list of child indices, which
  has already happened once in the Rust implementation.
- **First thing to probe**, not yet established: how a `.kel` stage addresses a byte buffer for
  emission and for reading. The `secret/` prototype used a data segment. Settle this before slice 1,
  since every later slice inherits it.

### Slice 1 as built (2026-08-09)

`src/selfhost/kel/wire.kel` holds the Keleusma implementation; `tests/selfhost_wire.rs` holds the
differential. Eleven tests, 0.67 s. The file is **not** in `read_stage`'s table and the driver does
not run it, because it does not yet emit an artifact.

**The oracle is a published constant, not our own code.** `crc32("123456789") == 0xCBF43926` is the
standard CRC-32/ISO-HDLC check value, and both Rust implementations are already independently pinned
to it (`keleusma-wire/src/crc.rs`, `src/vm.rs:11696`). The test compares against
`keleusma_wire::crc32` rather than the `crate::bytecode::crc32` this plan named, only because the
latter is `pub(crate)` and unreachable from an integration test. Same algorithm, same polynomial.

**What the probe settled**, each executed against the reference rather than reasoned:

| Question | Answer |
|---|---|
| Are locals immutable? | Yes, rejected at **parse**: "assignment is only supported for data block fields" |
| Does a runtime-range `for` need `limit`? | Yes, rejected at **verify**, not at parse |
| Is `lsr` a logical shift over the full word? | Yes: `(0 - 1) lsr 1` is `2^63 - 1` |
| Does `Byte as Word` sign-extend? | No, it zero-extends: `0xFF` reads as 255 |
| Bounded `for` inside a `fn`? | Yes, including nested, and across a call boundary |
| Call in statement position? | Yes |

**No masking is required, and this corrects a recorded design note.** The handoff expected the
accumulator to need `band 0xFFFFFFFF` after each step. It does not. The accumulator is always in
`[0, 2^32)`: it starts at `2^32 - 1`, a folded byte xors in under 256, a logical shift right leaves
it under `2^31`, and the polynomial is under `2^32`. The invariant holds without help, so a mask
would be dead work.

**`require word >= 64`, not the `>= 32` every stage declares.** Copying the stages' directive by
analogy would have been a silent defect. A 32-bit signed `Word` cannot hold either the initial value
or the polynomial, and — verified against the reference — **a source carrying those literals compiles
for a 32-bit target without complaint when no `require` is present.** Nothing else catches it.

**One inherent blind spot, enumerated rather than estimated.** A polynomial mutation is undetectable
on the empty buffer and on the single byte `0xFF`, and on nothing else. `0xFFFFFFFF xor 0xFF` is
`0xFFFFFF00`, whose low eight bits are clear, so all eight iterations take the else branch and the
polynomial is never consulted; exhausting all 256 single-byte inputs confirms `0xFF` is unique. The
test asserts the blind set **exactly**, so a case that joins it fails loudly.

**A consequence worth carrying forward:** the range invariant makes `asr` and `lsr` compute the same
values here, so swapping them is *not* caught by the differential. That equivalence is pinned by its
own test so it reads as understood rather than as an untested assumption.

**Both control directions are encoded**, not run by hand: three independent must-fire mutations
(polynomial, initial value, inner iteration count), a must-not-fire pass over a corpus whose coverage
is itself asserted, a `mutate` helper that requires its anchor to occur exactly once so a stale
anchor cannot silently test the unmutated source, and hostile-input cases for a length beyond the
array capacity (traps, does not truncate) and a length shorter than the buffer.

### On the prototype

`secret/kel-format-probe/wirefmt.kel` proves the encoder and decoder are expressible in Keleusma, but
it **predates format lock-in** and encodes a 12-byte directory entry. The shipped entry is 16 bytes,
and the triplicated prologue postdates it entirely. Treat it as evidence of feasibility, not as a
starting implementation.

---

## The 2026-08-03 analysis, superseded above but retained

The correction it makes to the roadmap's cost estimate is still accurate and worth keeping. Only its
conclusion — that the item is blocked — has been overtaken.

## The correction

`V0_2_X_ROADMAP.md` describes the item as: "The framing header, operand-pool encoding, parity, and
CRC trailer must move into Keleusma so the emitted artifact is produced end to end by the self-hosted
path." That enumeration omits the dominant cost.

`module_to_wire_bytes` (`src/wire_format.rs` ~1591) produces four regions:

| Region | Encoding | Self-hostable? |
|---|---|---|
| Framing header | 64 fixed bytes | Yes, mechanical |
| Opcode stream | 4-byte records via `encode_op` (~206 lines) | Yes, mechanical |
| Operand pool | 8-byte flat entries | Yes, mechanical |
| CRC trailer | CRC-32, reflected, poly in `crc32` | Yes, ~15 lines |
| **Auxiliary body** | **`rkyv::to_bytes`** | **NO — see below** |

**The auxiliary body is rkyv-archived**, and it carries everything except the opcode stream and
operand pool: per-chunk metadata (name, constants, struct templates, local/param counts, block type,
param types, debug pool), enum layouts, signatures, native return shapes, native names, entry point,
data layout, the word/addr/float width fields, the WCET/WCMU header, flags, shared and private data
sizes, and the schema hash.

rkyv is a ZERO-COPY ARCHIVE format: relative pointers, alignment and padding rules, a resolver
protocol, and its own versioning. Reproducing its byte layout in Keleusma is not "serialization" in
the sense the roadmap implies — it is reimplementing a third-party archival format byte-for-byte, with
the byte-identical oracle demanding exact agreement including padding. That is disproportionate, and
it is also FRAGILE: an rkyv upgrade would silently invalidate the Keleusma implementation.

**Therefore "self-host wire-format serialization" as literally stated is NOT a bounded increment, and
it is not the cheapest Order-1 item.** The earlier recommendation to start here (recorded in
`REVERSE_PROMPT.md` and `HANDOFF.md` on 2026-08-03) was based on the roadmap's enumeration and is
withdrawn.

> **Superseded 2026-08-08.** True while the auxiliary body was rkyv. It no longer is. See the
> re-scoping at the top of this document.

## What IS bounded

The four non-rkyv regions are mechanical and have a clean byte-identity oracle: emit them from
Keleusma and compare against the same regions of the reference's buffer. Suggested slicing, smallest
first, each independently verifiable:

1. **CRC-32.** ~15 lines, a pure function over a byte range. Trivially oracle-checked against
   `crate::bytecode::crc32` on random and edge-case buffers. Good first slice to establish the
   byte-emission harness.
2. **The opcode stream and operand pool.** The meaty part, and exactly what the roadmap calls
   "operand-pool encoding". codegen.kel already carries an internal op encoding whose tag values ARE
   the opcode ids (`getfield` 47, `getindex` 49, `gettuplefield` 53, ...), so the mapping to 4-byte
   records is close to what it already computes. Verify against `encode_op` over every op form,
   including the pool-spill cases.
3. **The framing header.** 64 fixed bytes; needs the region lengths from 1 and 2 plus the aux length.

After those three, the artifact is Keleusma-produced EXCEPT the aux body, which remains host-supplied
as an opaque byte block. That is an honest partial result and should be described that way — it does
NOT meet the Order-1 gate's "no Rust scaffold borrow" wording.

## The open question for the operator — ANSWERED 2026-08-08

Fully self-hosting the artifact requires a decision this plan cannot make: either reimplement rkyv's
layout in Keleusma (disproportionate and fragile), or **change the wire format** so the aux body uses
an encoding the self-hosted compiler can produce — which is a format change, and therefore a
`BYTECODE_VERSION` question and an operator decision under the standing rules.

Until that is decided, treat Order 1 as reachable only in part, and prefer the other two remainders
(the monomorphizer, then the type checker) for closing it.

> **Answered.** The operator chose the second option. The wire format was changed, the auxiliary body
> now uses the v2 container, and `BYTECODE_VERSION` moved to 2 under authorization granted
> 2026-08-06. Order 1 is reachable in full, and the deferral advice above no longer applies.

## Note on the monomorphizer

Probed at the same time: the `.kel` stage sources use no generics (the four `impl`/`trait` hits in
parse.kel are its own parser code for those keywords, not uses). Monomorphization is therefore
IDENTITY over the self-hosting subset, which is why the pipeline omits the pass entirely and still
matches the reference byte-for-byte. Self-hosting it would tick the box without changing any output.
Its real cost arrives only with full-language generics (Workstream F), so its value here is formal
rather than functional — worth knowing before it is picked as "the cheapest".
