# Wire Format Version 2 — Flat Auxiliary Body

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Design for replacing the `rkyv`-archived auxiliary body with a flat, self-hostable encoding, and
bumping `BYTECODE_VERSION` to 2.

> **SUPERSEDED on record structure (2026-08-04)** by
> [`WIRE_FORMAT_V2_WORD_ORIENTED.md`](./WIRE_FORMAT_V2_WORD_ORIENTED.md). The operator added two
> requirements this document did not design against — bit-level corruption tolerance and suitability
> for direct use in silicon — and both condemn its length-prefixed variable-length records: a variable length
> makes the next field's position data-dependent (hostile to hardware parsing) and a corrupted length
> destroys all following framing (hostile under corruption).
>
> **Still valid and not repeated in the successor**: the rkyv-displacement argument, the measured
> scope (59 `Archived*` references, 3 zero-copy entry points), and above all the **P10 analysis** —
> that string constants materialise as `KStr` aliasing the image, so the accessor layer must be a
> borrowed view and not an owned decode. Read that section there; the successor assumes it.

Status: **DESIGN. Operator-authorized 2026-08-03** (both the encoding change and the version bump).

## Why

The self-hosted compiler cannot produce the current artifact. The aux body is `rkyv`-archived, and
reproducing rkyv's zero-copy layout byte-for-byte in Keleusma — relative pointers, alignment and
padding rules, its own versioning — is disproportionate and would be silently invalidated by an rkyv
upgrade. See [`WIRE_FORMAT_SELFHOST_PLAN.md`](./WIRE_FORMAT_SELFHOST_PLAN.md) for that finding.

## Scope, measured before design

This is NOT confined to the aux body's framing. The VM executes against archived types, so the change
replaces the zero-copy representation the runtime reads:

| Surface | Count |
|---|---|
| `Archived*` references in `src/vm.rs` | 22 |
| `Archived*` references in `src/bytecode.rs` | 35 (incl. 15 `derive(Archive, ...)`) |
| `Archived*` references in `src/wire_format.rs` | 2 |
| Distinct archived types | 7 (`WireAuxBody`, `Module`, `ConstValue`, `DataLayout`, `BlockType`, `SlotVisibility`, `Vec`) |
| Zero-copy entry points | 3 (`vm.rs:781` `access`, `vm.rs:1181` `access_unchecked`, `bytecode.rs:3885` `access`) |

## The property that must NOT be lost

**In-place reads.** The VM reads the aux body without deserializing it. A naive replacement that
decodes into owned structures at load time would allocate per load, which works directly against the
crate's WCMU guarantee and its `no_std + alloc` embedded story. The flat format is therefore designed
for in-place field access, and the accessor layer must preserve that: no `Vec` materialization on the
load path, only bounds-checked reads at computed offsets.

Fixed offsets are in fact a BETTER fit here than rkyv's relative pointers: every read is a bounds
check plus an addition, which is auditable and trivially WCET-bounded, and it removes an unsafe
`access_unchecked` call from the hot path.

## Why not keep rkyv — engaging the recorded rationale (P10 check, 2026-08-03)

The rkyv choice was NOT incidental, and this design initially asserted "fixed offsets beat relative
pointers" without engaging it. `RESOLVED.md` records the deliberate switch: *"The body format is rkyv
rather than postcard. The choice was made to enable the planned zero-copy execution path (P10, path
B). Rkyv produces a self-relative addressable layout that supports in-place access without
deserialization."* Displacing rkyv therefore has to answer P10 on its own terms.

**P10 is complete, including its true-zero-copy phase.** `PRIORITY.md` records `Vm<'a>` with
`BytecodeStore<'a>` (an owned `AlignedVec` or a borrowed `&'a [u8]`), and
`unsafe Vm::view_bytes_zero_copy` as a constructor that borrows the buffer and executes with no
owned `Module` materialized. Two comments still say the opposite — `RESOLVED.md` and the doc comment
on `Vm::view_bytes` (`src/vm.rs` ~1911) both call true zero-copy "the next iteration of P10". Both
predate Phase 2 step 2 and are STALE; `view_bytes_zero_copy` exists. Measured, not inferred.

**What the runtime actually reads in place today**, which is what the replacement must preserve:

| Data | Path | In place? |
|---|---|---|
| Opcodes | `chunk_op` reads `self.decoded_ops`, a pre-decoded owned `Vec<Vec<Op>>` | **No** — decoded once at load |
| Constants | `chunk_const` / `chunk_const_str` read `self.archived().chunks[i]` | **Yes**, per access |
| Struct templates, native names, local counts, widths | archived accessors | **Yes** |

So rkyv's remaining zero-copy role is METADATA AND CONSTANTS, not the hot opcode fetch: the V0.2.0
ISA reset already moved ops into the flat opcode stream plus operand pool, and the per-op decode
cache moved them again into `decoded_ops`. The original P10 rationale has been substantially overtaken
by later work — but NOT entirely, because constants remain a live in-place read.

**The load-bearing case is string constants.** `chunk_const` materialises a non-empty top-level string
as a rodata-backed `KStr` pointing *directly at the immortal bytecode image* rather than copying it:
zero per-load allocation, WCET-flat, explicitly the "bake the ROM address" model. Any replacement that
copies strings out of the buffer would regress that, and it is exactly the kind of regression the WCMU
guarantee would not tolerate.

**Verdict: the flat format serves this at least as well, and for strings strictly better.** A
length-prefixed string is a DIRECT subslice of the buffer — a bounds check and an offset — whereas
`ArchivedString` requires following a relative pointer first. The constant-pool offset table this
design already carries is precisely what `chunk_const(idx)` needs for O(1) access. Nothing in the live
zero-copy surface depends on a property only rkyv provides.

**The constraint this places on stage 2, which is the reason for doing this check.** `decode_aux`
returns an OWNED `WireAuxBody`; if the cutover routed the runtime through it, every load would
materialise the whole aux body and string constants would stop aliasing the buffer — silently
undoing P10. **The accessor layer must therefore be a borrowed view over `&'a [u8]`, not an owned
decode.** `decode_aux` is for tooling, tests, and cold paths (`module_owned`) only. Stage 2 is where
this design is either honoured or lost.

**Unchanged interactions.** B9 (hot update of yielded static strings) is affected identically under
either format: a yielded string that aliases the buffer is invalidated by a hot swap, so B9's
resolution paths remain a prerequisite for the zero-copy path regardless of encoding. B10 (target
portability) improves slightly: rkyv was described as endian-stable, and an explicitly little-endian
flat format is endian-stable by specification rather than by a dependency's guarantee.

**Scope of research deliberately not done.** A broad survey of alternative formats (CBOR, protobuf,
FlatBuffers, Cap'n Proto) was considered and skipped. The binding requirement is that a Keleusma stage
must EMIT the format with no library, alongside in-place readability, determinism, and `no_std+alloc`.
CBOR and protobuf are not in-place readable; FlatBuffers and Cap'n Proto are, but their vtable and
pointer machinery carries the same emitter cost that ruled rkyv out. The requirement set forces a
hand-rolled flat layout, so a survey would re-derive the conclusion at real cost. Recorded here so the
omission is a decision rather than an oversight.

## Encoding principles

1. **Little-endian, fixed-width scalars.** No varints: a Keleusma stage writes bytes sequentially, and
   fixed widths keep both the emitter and the offset arithmetic trivial.
2. **No implicit padding or alignment requirements.** Every field is byte-addressed. This is what
   removes the aligned-copy the current loader performs before `rkyv::from_bytes`.
3. **Length-prefixed, offset-indexed regions.** Each variable-length region (chunk table, constant
   pools, name table, data layout) is preceded by a count and an offset table, so element *i* is
   reachable in O(1) without scanning — required for in-place access.
4. **All offsets relative to the aux-body start**, so the region is position-independent within the
   module buffer.
5. **Deterministic.** Identical modules must produce identical bytes; the byte-identity oracle depends
   on it.

## Sketch

```
aux body := aux_header, region*
aux_header := magic u32, aux_version u16, region_count u16, region_dir[region_count]
region_dir entry := kind u16, reserved u16, offset u32, length u32
```

Regions, each independently addressable: chunk table, constant pools, struct templates, param types,
name table, enum layouts, signatures, native return shapes, data layout, and the scalar header block
(entry point, word/addr/float widths, WCET/WCMU, flags, data sizes, schema hash).

A region directory rather than a fixed field order means later additions do not shift existing
offsets, which is what makes this format extensible without another version bump.

## Staging

The change is too large for one increment. Each stage must leave the gate green:

1. **Define the format and the encoder** in Rust, behind the existing `WireAuxBody` producer. Verify by
   round-tripping every existing test module through encode-then-decode.
2. **Write the accessor layer** replacing the `Archived*` read surface, with the same API shape the VM
   uses so the call sites change minimally.
3. **Cut the VM and loader over**, delete the rkyv path, bump `BYTECODE_VERSION` to 2.
4. **Drop the `rkyv` dependency** if nothing else needs it (nothing appears to), and update
   `Cargo.toml`, the tech-stack list, and `docs/spec/WIRE_FORMAT.md`.
5. **Self-host the emitter** in Keleusma — the original goal, now reachable.

## Consequences to record when this lands

- `BYTECODE_VERSION` becomes 2, so a version-1 module is REJECTED rather than accepted-and-mis-read.
  That closes the hazard `CLAUDE.md` documented and accepted under the no-public-adoption policy; that
  policy text must be updated, since it currently says the number stays 1.
- The signed-module path (`module_to_signed_wire_bytes`, `src/wire_format.rs` ~1848) also archives the
  aux body and must move to the same encoding; signatures cover the buffer, so the change is
  transparent to the signing scheme but the test vectors will change.
- Dropping rkyv removes a dependency from a crate whose value proposition includes auditability, which
  is a secondary benefit worth stating in the release notes.
