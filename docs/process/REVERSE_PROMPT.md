# Reverse prompt

## Order 4 completed for its first-pass scope, 2026-10-25

The goal is to absorb kaizen work as it arrives and complete Order 4. Kaizen PRs
508, 512 and 516 merged into v0.2.3 during the session.

The operator ruled first-hand that the Order 4 gate is the subset corpus run
under the interpreter with results matching the Rust virtual machine, and that
bounds compose by fuel-sliced ticks. The hosted runtime landed in five slices on
feat/v02-order4-hosted-runtime: scalars and control flow, flat composites, the
data segment, calls and streams. The interpreter is src/hosted/interp.kel, its
driver src/hosted/mod.rs behind the self-host feature, and its harness
tests/hosted_runtime.rs.

Evidence. Over 125 corpus programs compiled by the shipping self-hosted compiler,
116 of the 117 non-stream cases are admitted and agree with the Rust virtual
machine on all 1066 runs, and all nine stream cases agree on 541 events. The one
refused case has no static loop bound, and the Rust virtual machine refuses it
too. Every run stays within its computed instruction and tick bounds. The
interpreter's own per-tick bound is a constant 55,824,637 nominal cycles. The
audit is docs/decisions/ORDER4_COMPLETION_AUDIT.md.

Two soundness fixes landed first. PR 517, the yield type check and the checked
flat Text decode, reported by pykeleusma, merged as 50b20ef6. PR 519, the time
bound's uncharged structural markers, found by the Order 4 instruction count and
ruled first-hand by the operator, merged as c2733d0a.

Verification. The full local release matrix passed: workspace 3524, self-host
3264, no-default 338, signatures 2504, signatures with shell 2521, doctests 12
with 3 ignored, and the detached compiler 86, with both Clippy configurations
and the documentation build clean. Remote continuous integration has not yet run
the branch.

Questions for the operator.

1. The VM's own flat Text field reads still trust their words for a hand-built
   module the typed verifier admits. Should a layout-aware typed pass, a range
   check alone, or a recorded trust assumption apply?
2. Should a 0.2.2 patch release carry the soundness fixes? Publishing needs your
   authorization.
3. pykeleusma reports for 0.2.2, unverified on v0.2.3. `replace_module` accepts
   unsigned code once a key is trusted. `verify::module_runtime_footprint`
   overflows the stack on an unverified module. A whole-array store to a private
   array fills only its first slot. Const-generic and type-generic instances
   have wrong layouts for host-built values. A host that builds its own
   RefContext has no accessor for the bytecode image after PR 517. Which should
   this line take first?

Concerns. The interpreter's tick bound is loose, and the self-hosted compiler
refuses interp.kel. A resume value inside a composite literal is refused at
load. HANDOFF.md is stale. Kaizen's relayed positions on HANDOFF.md and
REVERSE_PROMPT.md are unconfirmed first-hand.

Next. Open the pull request for Order 4, record the remote run in the audit, and
merge when green.
