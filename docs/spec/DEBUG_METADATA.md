# Debug Metadata

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

Authoritative specification of the strippable debug metadata format (backlog item B29). This document defines the record catalogue, the operand encoding of each record kind, the canonical byte form of the section, the read and query interface, and the runtime fault-localization path. The design rationale and decision history live in B29 of [`../decisions/BACKLOG.md`](../decisions/BACKLOG.md); this document is the reference.

## Model

Keleusma carries debug information in an optional, chunk-local section called the debug pool. There is no debug opcode. Debug information never appears in the opcode stream. A debug build and a release build therefore share a byte-identical opcode stream, and stripping debug information is the removal of a separable section rather than a transform of the program.

A debug pool holds four sub-pools.

| Sub-pool | Element | Purpose |
|----------|---------|---------|
| String pool | UTF-8 string | File names, variable names, label names, pass and property identifiers, and the optional assertion message |
| Span pool | `(file, byte_offset, byte_length)` | Source spans, where `file` is a `u16` index into the string pool |
| Type pool | length-prefixed byte blob | Compact type representation referenced by a `TypeAnnotation`. Version 1 stores a UTF-8 string-form rendering |
| Record pool | debug record | The op-index-keyed annotations that reference the three data sub-pools by index |

A debug record has three fields. `op_index` is the opcode-stream position the record annotates, as a `u32`. `kind` is the record kind, serialized as one byte. `operands` is a list of `u16` indices whose meaning is fixed per kind, as catalogued below. An operand is an index into one of the data sub-pools, or an inline small integer, never a pointer.

An absent debug pool and an empty debug pool are distinct. An absent pool omits the section from the chunk entirely. A producer that has gathered no records collapses the empty pool to absent.

## Record catalogue

The twelve record kinds and their stable wire discriminants follow. The discriminants must not be renumbered. New kinds append at the end.

| Kind | Byte | `op_index` | Operands |
|------|------|------------|----------|
| `CallSite` | 0 | the call op | `[span]` |
| `SourceSpan` | 1 | a statement start, a block tail expression, or a partial-operation op | `[span]` |
| `LineNumber` | 2 | a statement start | `[line]`, the source line clamped to `u16` |
| `VariableName` | 3 | the declaration op | `[slot, name]`, a local slot and a string-pool index |
| `TypeAnnotation` | 4 | the declaration op | `[slot, type]`, a local slot and a type-pool index |
| `AssertionContext` | 5 | the assertion trap op | `[span]`, or `[span, message]` with a string-pool index |
| `BreakpointCandidate` | 6 | a statement start, a block tail expression, a partial-operation op, or `0` for function entry | `[span]` |
| `GenericInstantiation` | 7 | `0`, chunk-level | `[origin, type_args]`, two string-pool indices |
| `IfcLabelAnnotation` | 8 | the `classify` or `declassify` op | `[label, ...]`, a variable-length list of string-pool indices |
| `WcetMarker` | 9 | `0`, chunk-level | `[block_id, cycles_low, cycles_high]`, where `block_id` is `0` for the whole chunk and the two halves reconstruct a `u32` cycle count |
| `OptimisationMarker` | 10 | the optimisation site | `[name]`, a string-pool index |
| `VerifierWitness` | 11 | the construct op, or `0` for a chunk-level fact | `[pass, property]`, two string-pool indices |

The information-flow control labels referenced by `IfcLabelAnnotation` are the audit trail of a `classify` or `declassify` operation. The pass and property identifiers referenced by `VerifierWitness` are described under [Verifier witness](#verifier-witness).

## Wire encoding

The debug pool serializes to a self-contained byte form with little-endian integers and `u32` length prefixes, matching the runtime wire format conventions. The four sub-pools are emitted in a fixed order.

```
debug_pool := string_section span_section type_section record_section

string_section := u32 count, then count times { u32 byte_length, UTF-8 bytes }
span_section   := u32 count, then count times { u16 file_index, u32 byte_offset, u32 byte_length }
type_section   := u32 count, then count times { u32 byte_length, bytes }
record_section := u32 count, then count times { u32 op_index, u8 kind, u16 operand_count, operand_count times u16 }
```

The encoded bytes are stored in the chunk wire format as the optional `WireChunk.debug_pool_bytes` field in the auxiliary body. See [`WIRE_FORMAT.md`](./WIRE_FORMAT.md). The field was added within the V0.2.x line without a `BYTECODE_VERSION` bump. A runtime built before B29 does not know the optional field.

### Determinism

The encoder sorts the record pool into a canonical order before emission. The order compares `op_index` first, then the kind byte, then the operand list lexically. The same logical set of records therefore produces byte-identical output regardless of the order in which the producer appended them. The three data sub-pools are emitted in their stored order, because records reference their entries by index. A producer that requires end-to-end byte-determinism builds those sub-pools deterministically. Sorting the records does not disturb sub-pool indices, because each record carries those indices in its operands. The decoder returns the records in the same canonical order, so `decode(encode(p))` round-trips and re-encodes to identical bytes.

## Read interface

A consuming tool resolves opcode-stream positions to debug information through the query interface on the debug pool. A debugger, a stack-trace formatter, or a runtime error decorator uses this interface.

| Query | Returns |
|-------|---------|
| `records_at(op_index)` | the records annotating one opcode position, in canonical order |
| `string(index)` | the string-pool entry, or `None` when the index is out of range |
| `span(index)` | the span-pool entry as `(file_index, byte_offset, byte_length)`, or `None` |
| `type_blob(index)` | the type-pool blob, or `None` |
| `source_location(record)` | a resolved `SourceLocation` for a span-bearing record, or `None` |

`source_location` applies to the record kinds whose first operand indexes the span pool, namely `CallSite`, `SourceSpan`, `AssertionContext`, and `BreakpointCandidate`. It returns the file name and the byte range. It returns `None` for any other kind, for a record with no operands, or when an operand index dangles.

## Runtime fault localization

The virtual machine maps a runtime trap back to source through the debug records. The machine records the opcode position it is about to dispatch in a fault-location field. The field is cleared on a successful run, so it names the faulting op only after a failed `call` or `resume`. The location is virtual-machine state rather than part of the error value, so the error type is unchanged.

| Query | Returns |
|-------|---------|
| `Vm::fault_location()` | the `(chunk, op)` that trapped, or `None` after success or before any run |
| `Vm::fault_source_location()` | an owned `FaultSource`, or `None` when there is no fault or no resolving record |

`fault_source_location` decodes the faulting chunk's debug pool on demand and resolves in two tiers. The first tier returns a span-bearing record sitting exactly at the faulting op, with the `exact` flag set true. The second tier falls back to the nearest enclosing statement, the `SourceSpan` with the greatest op index at or before the fault, with the `exact` flag set false. The resolver never fabricates a location. The owned `FaultSource` carries the file name, the byte range, and the `exact` flag, so a host does not over-trust a fallback.

The compiler emits a `SourceSpan` at every partial-operation op, so each partial-operation trap resolves exactly. The covered operations are division and modulo, array indexing and data-array indexing, and the newtype-construction refinement check. A failed debug assertion resolves exactly through its `AssertionContext`. A trap at an op that carries no operator-site span resolves to the tightest enclosing statement or tail expression with the `exact` flag false.

## Verifier witness

A `VerifierWitness` record names a proof the verifier discharged. The structural witness records one obligation per individual check of the three structural verification passes, produced inline by the verification routine so the trace cannot drift from the checks performed. The resource-bound witness records the worst-case-execution-time and worst-case-memory-usage bound proofs. A Stream chunk records per-iteration bounds. A `Func` or `Reentrant` chunk records per-chunk bounds. The witness records that a bound was proven. It does not carry the bound's magnitude, which for a Stream chunk lives in the adjacent `WcetMarker`. The pass identifier names the verification pass and the property identifier names the proven property. A chunk-level fact carries `op_index` zero. A construct-level fact carries the op position of the construct it concerns, so a reader groups facts with `records_at`.

## Emission and stripping

`keleusma compile --debug` emits the debug pool. Without the flag the compiler emits no debug metadata and produces output byte-identical to a release build. The compile-out debug `assert` construct is the one exception to byte-identity, because its runtime check is compiled in only under a debug build. See [`GRAMMAR.md`](./GRAMMAR.md) and [`RUNTIME_FAULTS.md`](./RUNTIME_FAULTS.md).

`keleusma strip <file>` removes the debug section, producing a release artefact byte-identical to a non-debug compile. Stripping refuses signed or encrypted input, because rewriting the body invalidates a signature. The supported order is compile, then strip, then sign.

## Stability

The record-kind discriminants are stable wire values. They are never renumbered. A new kind appends at the next free discriminant. A decoder rejects an unknown kind byte rather than guessing. The debug section is optional and self-describing through its length prefixes, so a reader skips or ignores it without affecting execution. The metadata neither pushes nor pops operand-stack values nor alters control flow, so the verifier's stack-effect and control-flow analyses are identical with or without the section, and the worst-case-memory pass treats it as zero runtime cost.

## Known limitations

The following refinements are deferred. Each is a tightening of an already-faithful result rather than a coverage gap.

- Breakpoint candidates emit at statement boundaries, block tail expressions, partial-operation ops, and function entry. A candidate at every operator op is unimplemented as high-volume and of marginal value.
- The `Reentrant` per-resume worst-case-execution-time bound is exact for top-level yields and a sound, productive-loop-clamped bound for nested yields. A finer bound for the nested case would require a structured maximum-yield-free-segment analysis.
- Source spans resolve every partial operation and every block tail expression exactly. Full per-op spans for non-trapping operations are unimplemented as high-volume.
