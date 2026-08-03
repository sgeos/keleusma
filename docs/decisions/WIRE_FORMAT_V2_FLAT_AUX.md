# Wire Format Version 2 — Flat Auxiliary Body

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Design for replacing the `rkyv`-archived auxiliary body with a flat, self-hostable encoding, and
bumping `BYTECODE_VERSION` to 2.

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
