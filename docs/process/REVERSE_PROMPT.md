# Reverse prompt

## Flat Text decode soundness and Order 4 rulings, 2026-10-25

The operator set the goal of absorbing kaizen work as it arrives and completing
Order 4. Kaizen PRs 508, the release-gate fix, and 512, the CLAUDE.md trim,
merged into v0.2.3 as 1b8e9ea4 and ec536b28 after green CI. In review of 512
V0.2.X asked for "also validates that file" in place of "instead", because the
operator positions on startup files have reached this line only by relay.

The pykeleusma line reported a segmentation fault reached through safe
interfaces. V0.2.X reproduced it on v0.2.3. Two defects combined. The type
checker never unified a yielded value with the declared return type, and
`Vm::decode` rebuilt a flat Text field's two words as a pointer without checking
them. The second needs no type gap, since decoding a well-typed body with a
mismatched host type crashed the same way. Both are repaired on
fix/v02-flat-text-soundness. The record is
docs/decisions/FLAT_TEXT_DECODE_SOUNDNESS.md.

Verification. The full workspace suite in release passed 3512 with 2 skipped,
no-default features 338 and signatures with shell 2517. A reverted type checker
makes the new rejection test fail. Remote continuous integration has not yet run
the branch.

Order 4. The operator ruled first-hand on 2026-10-25 that the gate is the subset
corpus run under the interpreter with results matching the Rust virtual machine,
and that bounds compose by fuel-sliced ticks. The 107 Ok boundary cases are all
single chunks with one stream case, so the corpus needs a supplement for calls
and multi-iteration streams. The plan will land with the first slice.

Questions for the operator.

1. The VM's own flat Text field reads can still be reached with a wrong layout
   by a hand-built module the typed verifier admits. A range check is constant
   cost, and a UTF-8 check would make the read's time bound depend on the
   string's length. Which of a layout-aware typed pass, a range check alone, or a
   recorded trust assumption should apply?
2. Should a 0.2.2 patch release carry the decode repair? The published crate
   keeps both defects. Publishing needs your authorization.
3. pykeleusma also reports that a signed hot swap always fails and that
   `replace_module` accepts unsigned code after a verifying key is trusted.
   Neither is verified on v0.2.3 yet. The second is a policy bypass if it holds.

Concerns. A resume value inside a composite literal is refused at load with
`NewCompositeSizeMismatch`, before and after this repair. Kaizen relayed operator
positions on HANDOFF.md and REVERSE_PROMPT.md again, still unconfirmed
first-hand. HANDOFF.md is stale.

Next. Open the pull request for this repair, then cut the Order 4 branch and
land slice 1, the driver, the tick loop and scalar and control-flow instructions.
