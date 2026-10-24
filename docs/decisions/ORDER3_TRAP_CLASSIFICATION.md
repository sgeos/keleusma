# Order 3 trap classification

## Contract

The reference entry point is trap_analysis::classify_traps. The Keleusma entry
point is selfhost::classify_traps_via_kel. Both take a module, validate it and scan
all chunks in module order. They do not execute subject instructions.

A completed TrapFree verdict means that no explicit Trap occurs and every
instruction belongs to the audited first-pass language-trap profile. MayTrap
reports the first explicit Trap, including traps in unreachable instructions or
unused chunks. This is a conservative syntactic classification. It is not a
claim that a particular execution will reach that instruction.

Unsupported forms return an error before any completed verdict. They include
GetIndex outside flat Word and Byte, boxed struct, tuple and enum field
reads, and native calls. Raw FixedDiv was among them until 2026-10-22, when a
zero divisor became an outcome and FixedDiv joined the clear instructions.
Native-error widening remains a later roadmap obligation.
The total arithmetic and bounds contracts already implemented are prerequisites
for the admitted profile. Rejecting unsupported forms does not replace that work.

InvalidModule reports rejected structure, types, header metadata or known invalid
immediates. Capacity reports a chunk above the shared 2048-instruction profile.
Unavailable reports incomplete verification, stage failure or an invalid response.
None of those errors means that a completed proof established possible trapping.

This verdict is informational. It is not an execution certificate and cannot
justify deleting guards. Unknown operand shapes, host input and result contracts,
resource availability, storage representation and lifetime checks remain separate.
The host policy and actual proof-gated check-elision paths are still required by
the completion plan.

## Reference and stage ownership

The reference uses structural and typed verification, followed by an exhaustive
Rust match over the instruction enumeration. Adding an opcode therefore requires
an explicit reference-profile decision. Width and entry-point metadata are checked
before invoking verifier code that shifts by a declared width.

The Keleusma path uses verify_module_via_kel for validation. Mode two of the
existing verify_structural.kel stage then scans transported raw wire identifiers
and operand forms. It determines both profile availability and Trap presence.
The host supplies neither a safety label nor a reference trap verdict. It checks
response structure, location bounds and agreement between stored and yielded
verdicts. An unsupported instruction remains unavailable even when another
instruction is an explicit Trap.

The stage uses existing shared arrays in its separate mode. There is no new stage
file, public opcode, bytecode version or fingerprint. Existing structural modes
zero and one retain their protocols. Mode-two requests reset all output state,
including after malformed requests.

## Private mode-two transport

| Shared field | Meaning |
|---|---|
| sv.mode | Two selects trap analysis |
| sv.op_count | Number of subject instructions, from zero through 2048 |
| sv.class | Raw wire opcode identifiers |
| sv.arg | Access form, or the Trap or PushImmediate immediate |
| sv.opb | Scalar-kind tag for a flat scalar field or index operand |
| sv.out_reject | Zero trap-free, one explicit Trap, two unsupported, three invalid subject, four invalid protocol |
| sv.o1[0] | First explicit Trap index, or negative one |
| sv.o2[0] | First unsupported or invalid index, or negative one |
| sv.o3[0] | Unsupported reason |

Access forms are one for flat scalar, two for flat nested and three for boxed.
Other instructions carry zero except Trap and PushImmediate, which carry their
immediate. Scalar tags are zero for non-scalar forms. Unsupported reasons are
two for indexed values, three for boxed fields and four for native
calls. Reason one denoted FixedDiv until 2026-10-22 and is no longer produced, and
the Rust reader treats it as unknown. Unknown wire tags, malformed forms and inconsistent response sentinels
cannot produce a completed verdict.

## Acceptance evidence

Focused controls compare both classifiers on bare and fully handled Word and Byte
arithmetic and indexing, including shared storage. Other controls cover guarded
handlers, hand-built total arithmetic, unreachable code, unsupported forms,
malformed modules, exact capacity and one instruction beyond it, and malformed
stage requests followed by valid recovery.

Two interventions alter only the stage. One suppresses Trap recognition and the
other changes admission of boxed field reads, which replaced raw FixedDiv as its
subject on 2026-10-22. Each changes the resulting classification,
establishing that the Keleusma implementation owns those decisions. A separate
control rejects a fabricated stage response with inconsistent metadata.

Whole-stage classification, complete artifact identity, resource admission and
feature checks are recorded in REVERSE_PROMPT.md. The broader Order 3 audit must
still establish host policy, resume and replacement preservation, actual check
elision and all final verification requirements.
