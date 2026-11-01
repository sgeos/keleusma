# Order 5 plan, Workstreams G and H

> **Navigation** [Decisions](./README.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

## Scope

The operator set Order 5 as this line's goal on 2026-10-25. The roadmap gives it
the gate "The flat-machine ISA in native; native hot swap". Workstream G is the
flat-machine ISA redesign of backlog entry B28, landed "where native makes it
free". Workstream H is hot replacement at the native level, the counterpart of
bytecode hot swap.

`BYTECODE_VERSION` stays 2, no opcode is added, and root `src/` and `tests/`
are not edited. Publication is held.

## The premise this plan takes for Workstream G

B28's deferred redesign has two halves. One is the bytecode instruction set
itself, where an arithmetic or comparison opcode would carry its operand kind
rather than dispatching on a tag. The other is native code that carries no
run-time kind tag and dispatches on no kind. The first half changes the
instruction set and the wire format, which needs operator authorization, and
roadmap open decision 3, whether the redesign lands in V0.3.x or V0.4.x, is the
operator's. This plan therefore takes the second half only, the flat machine
**in native**, and records the first half as an open question.

## Starting state, measured on 2026-10-25

- Native operands are untagged `i64` words. Composites are pure bytes at
  offsets the bytecode bakes. Outside a coroutine an operand's kind is resolved
  statically, and an unknown kind refuses the operation that needed it.
- Inside a coroutine, `coroutine::types` joins possible kinds over a tag
  lattice and the emitter keeps run-time kind and length metadata where a join
  leaves several kinds possible. Kind guards then check that metadata.
- The VM hot-swaps through `Vm::replace_module`, which refuses a new module
  whose `schema_hash` differs from the current one. The host owns the shared
  buffer across the swap and supplies the new private data.
- No native artefact can be replaced while its host runs.

## Requirements

| Id | Requirement | Planned evidence |
|---|---|---|
| G1 | The native operand representation is untagged | A census of every emitted function over the corpus and the stage sources records whether it carries run-time kind metadata, with the reason for each that does |
| G2 | A tag-free lowering exists and is proven tag-free | An option refuses any coroutine that would need run-time kind metadata. Its output contains no kind metadata, and the admitted subjects still agree with the VM |
| G3 | Every kind-polymorphic instruction is resolved statically or refused | A census over the corpus classifies each instance of an opcode the VM dispatches on by kind as statically resolved, resolved by a run-time guard, or refused |
| H1 | A native library is replaceable while its host runs | A library is linked as a shared object. A host loads it, runs it, swaps to a second version at a step boundary, and unloads the first once no instance is live |
| H2 | The acceptance rule is the VM's | The library exports its `schema_hash`. The host refuses a swap whose hash or shared size differs, as `replace_module` does |
| H3 | The swapped run agrees with the VM | The same two versions run through `Vm::replace_module` at the same step, and every yield and every shared byte agree |

## Known limits accepted in advance

- The bytecode instruction set is unchanged, so the flat machine exists in
  native code only.
- A swap replaces whole libraries at a step boundary. A live instance is
  released and the new version starts from its entry, as the VM's swap does.
  State migration beyond the shared buffer is the host's, under Replace
  semantics.
- `schema_hash` covers slot names and visibility, not slot types, as the VM's
  documentation states.
