# Reverse prompt

## Order 5 completed for its first-pass scope, 2026-10-25

The goal is to absorb kaizen work as it arrives and complete Order 5. No kaizen
work arrived during the session. A status and request went to the kaizen line.

Order 5 is approach 1 of roadmap Workstream E, the signing and encryption
framing in Keleusma with the primitives as host natives. The program is
src/crypto_frame/frame.kel and its driver src/crypto_frame/mod.rs. The plan is
docs/decisions/ORDER5_CRYPTO_FRAMING_PLAN.md and the audit
docs/decisions/ORDER5_COMPLETION_AUDIT.md.

The gate is my reading of the roadmap gate, and you have not ruled on it. Over
20 compiled modules, signed artifacts with and without error-correcting planes,
encrypted artifacts and decrypted views equal the reference byte for byte.
Verification and decryption verdicts equal the reference's over 2746 runs, with
every refusal class reached. The pipeline with sections from the Order 1 wire
stages reproduces the reference, and keleusma compile --compiler self-hosted
now signs and encrypts through the program and refuses a divergence. No key
enters the program. An independent review found no divergence and four smaller
points, all acted on.

A defect was found on the way. verify_module_signature panicked on a signed
header over a short declared total, reachable from Vm::load_signed_bytes on
untrusted bytes. Repaired in PR 524, recorded in SIGNATURE_VERIFY_BOUNDS.md.

Questions for the operator.

1. Do you accept the Order 5 gate as stated in the plan, byte and verdict
   identity with the reference, the ordering held by the program, the
   self-hosted section path and the command line integration?
2. The decrypt path takes ciphertext to the end of the buffer rather than the
   declared total, so an encrypted artifact with trailing bytes cannot be
   decrypted while a signed one loads. Should it use the declared total?
3. Open from Order 4. The flat Text read trust question, a 0.2.2 patch release,
   which needs your authorization, and which pykeleusma finding to take first.
4. The V0.3.X line reported two findings on files this line owns, as peer
   data that I have not yet verified. docs/spec/RUNTIME_FAULTS.md still
   specifies native default values for an unhandled partial operation, while
   the compiler now emits an explicit Trap. The whole-chunk time bound also
   refuses intern_id in the self-hosted lexer as a text operation on two Word
   results. Should either be taken before the next order?

Concerns. The primitives' time is outside the program's bound, since they are
registered with the default native attestation. Approach 2 is not started.
HANDOFF.md is stale. Kaizen's relayed positions on HANDOFF.md and
REVERSE_PROMPT.md remain unconfirmed first-hand.

Next. The operator's answers decide the next increment.
