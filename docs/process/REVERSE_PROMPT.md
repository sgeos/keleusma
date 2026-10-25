# Reverse prompt

## WCET structural markers, 2026-10-25

The goal is to absorb kaizen work as it arrives and complete Order 4. The
operator ruled first-hand that the Order 4 gate is the subset corpus run under
the interpreter with results matching the Rust virtual machine, and that bounds
compose by fuel-sliced ticks. Slice 1 of the interpreter is on
feat/v02-order4-hosted-runtime as a local WIP commit. It admitted 47 corpus
cases and agreed with the virtual machine on all 698 runs.

The same run counted each program's executed instructions against its verified
time bound, and seven runs exceeded the bound by one. The time walk and its
Keleusma mirror charged nothing for Else, EndIf and EndLoop, and a counted loop's
final header pass went uncharged. The operator ruled first-hand to fix the bound
before Order 4 continues. The repair is on fix/v02-wcet-structural-markers, and
the record is docs/decisions/WCET_STRUCTURAL_MARKERS.md. Four hand-derived
bounds are pinned in tests/wcet_marker_charging.rs.

PR 517, the flat Text decode soundness fix, merged into v0.2.3 as 50b20ef6 after
green CI. V0.3.X and pykeleusma were notified.

Verification. The release workspace run passed 3504 of 3506. The two failures
were the analyze.kel record-count pin and three moved line citations, both
updated, and the affected binaries then passed. This branch has since merged
v0.2.3, and remote continuous integration has not yet run it.

Questions for the operator.

1. The VM's own flat Text field reads still trust their words for a hand-built
   module the typed verifier admits. Should a layout-aware typed pass, a range
   check alone, or a recorded trust assumption apply?
2. Should a 0.2.2 patch release carry the soundness fixes? Publishing needs your
   authorization.
3. pykeleusma reports for 0.2.2, unverified on v0.2.3: `replace_module` accepts
   unsigned code once a key is trusted, `verify::module_runtime_footprint`
   overflows the stack on an unverified module, const-generic and type-generic
   instances have wrong layouts for host-built values, and a host that builds its
   own RefContext has no accessor for the bytecode image after PR 517.

Concerns. Bounds declared in artifacts built before this fix stay understated. A
resume value inside a composite literal is refused at load. HANDOFF.md is stale.
Kaizen's relayed positions on HANDOFF.md and REVERSE_PROMPT.md are unconfirmed
first-hand.

Next. Open the pull request for this fix, then resume Order 4 slice 1 on the
corrected bound and continue to slice 2, flat composites.
