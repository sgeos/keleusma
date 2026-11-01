# Order 6 plan, full native code generation

> **Navigation** [Decisions](./README.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

## Scope

The operator set Workstreams A to I and Orders 1 to 6 as this line's goal on
2026-10-25. Orders 1 to 5 are complete. Order 6 is the roadmap's last rung,
**full native code generation, which defines V0.4.0**, with the gate "the full
language lowers to native across the target set; native is a first-class
deployment shape". Its workstreams are "A (full), plus widening". Workstream I,
language feature additions and revisions surfaced by native lowering, runs
alongside it.

`BYTECODE_VERSION` stays 2, no opcode is added, and root `src/` and `tests/`
are not edited. Publication is held, so V0.4.0 is not released by this work.

## Starting state, measured on 2026-10-25

- **The instruction set.** 61 of the 66 opcodes lower with corpus evidence.
  The other five each carry a disposition in `isa_lowering_census.rs`, and none
  is a missing lowering.
- **The corpus.** 66 of 76 compiled modules execute natively and agree with
  the virtual machine. One is refused for the yield-escape hazard, and the rest
  are exempt for recorded reasons.
- **The language.** No census walked the grammar, so a construct no corpus
  program uses was invisible.
- **`Text`.** Literals, locals and returns lower. A `Text` inside a composite
  is refused, and a `Text` result has no differential, because native code
  carries a string as one address while the virtual machine's flat `Text` is
  the two words `(data_ptr, len)`. `TEXT_FIELD_VERDICT.md` records it.
- **Opaque handles.** Refused on every route. The virtual machine holds them as
  host-owned references with no flat layout.
- **The target set.** Only `aarch64-apple-darwin` was executed. Tier 1 for
  V0.4.0 is x86-64 Linux, AArch64 Linux and macOS on both architectures.
- **The deployment shape.** A library could be produced only by an example
  program.

## Requirements

| Id | Requirement | Planned evidence |
|---|---|---|
| A1 | Every opcode a compiler can emit lowers | The ISA census, unchanged, and its five dispositions |
| A2 | Every construct of the grammar lowers or is accounted for | A construct census over `GRAMMAR.md`, each construct agreeing with the VM, refused with its reason, or covered by a named file |
| A3 | `Text` is carried as the virtual machine carries it | A two-word native string, so a `Text` field packs, and `Text` results are compared with the VM |
| A4 | Every refusal the census finds is closed or dispositioned | Each construct the census refuses is lowered or recorded with its cause |
| W1 | Tier 1 targets build, and those this machine can run are executed | The lexer built for all four Tier 1 triples, with symbols checked, and run on both macOS architectures |
| W2 | Native is a deployment shape of its own | A `kel-native` command builds a library, archive and shared object, and inspects the contract, with every exit status tested |
| I1 | Language constraints native lowering surfaced are recorded and sent to their owner | A record of each, sent to the V0.2.X line |

## Dependencies outside this line

- The roadmap's correction notes that a claim of the full language resting on
  a full-language corpus needs the complete self-hosted compiler, which is the
  V0.2.X line's Order 6, in progress.
- Executing the Linux targets needs a Linux runner, a continuous integration
  change for the operator.
- Opaque handles need an ABI for host references, a host-contract design for
  the operator.
