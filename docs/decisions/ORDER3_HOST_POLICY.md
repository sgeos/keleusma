# Order 3 host language-trap policy

## Contract

TrapPolicy is host-owned state on each virtual machine. Allow is the default and
preserves ordinary execution. RequireTrapFree admits only a completed TrapFree
classification of every chunk. MayTrap, unsupported contracts, malformed modules,
capacity exhaustion and unavailable verification all prevent strict admission.
The virtual machine computes the classification itself. A caller cannot supply
a verdict as authority to bypass that check.

VmOptions carries trap_policy alongside the existing overflow policy. The
new_with_options constructor enforces it before constructing an executable
instance. Runtime type, shape, lifetime, storage and resource checks remain active.
This policy does not enable unchecked execution or establish its certificate.

## Loading and configuration

| Entry point | Policy selection |
|---|---|
| new_with_options | VmOptions.trap_policy |
| load_bytes_with_options | Same options after bytecode decoding |
| view_bytes_with_options | Same options after view decoding |
| load_signed_bytes_with_options | Same options after signature authentication |
| load_encrypted_signed_bytes_with_options | Same options after authentication and decryption |
| Existing constructors | Default Allow behavior |
| Borrowed and trust-skip constructors | Apply with_trap_policy before the first call |

The options-bearing loaders return the instance and verification warnings.
Existing authentication, framing and resource checks continue to apply. Options
cannot bypass an earlier authentication or framing rejection.

set_trap_policy verifies the current immutable image before assigning a stricter
policy. A rejected change preserves the old policy and suspended state. The
with_trap_policy builder consumes the loaded instance and returns it only after
the requested policy is established. trap_policy reports the stored selection.
A host may explicitly choose Allow again. Bytecode cannot select or relax policy.

Without the verification feature, RequireTrapFree returns an error, including
when an unsafe loader was used. Host attestation of earlier verification does not
replace the required trap proof. Unsafe loading retains its existing documented
contracts and does not itself activate strict trap policy.

## Execution and replacement

Calls, yields, resumes, resets, breakpoints and runtime contract errors retain the
policy. Every module replacement path reaches the same policy gate before changing
the running image or coroutine state. The schema-bypassing replacement method
also enforces it. A trap-policy rejection therefore leaves the previous coroutine
available for resume. Successful replacement preserves the stored policy.

Signature trust and trap policy are separate host choices. Signed byte replacement
authenticates before the trap gate. Its internal image clears the signature-required
flag after authentication because it does not retain the signature extension.
The host verifying-key matrix remains installed and governs later byte loads.
This matches initial signed loading and repairs the former successful-replacement
failure caused by re-encoding that flag without its signature extension.
Changing verifying keys does not alter trap policy.

## Evidence and remaining work

Controls exercise accepted and rejected construction, unavailable proof, malformed
modules, opcode capacity, default permissive execution, serialized and view loading,
borrowed and trust-skip routes, failed policy changes, successful and rejected
replacement, suspended state, resets, breakpoints, runtime type errors and explicit
opt-out. Signature and encryption controls cover authentication, policy retention,
valid signed replacement, wrong keys, unsigned replacement and trust-skip routes.
Verification-disabled controls include authenticated loaders.

The current host uses the reference classifier. The independent Keleusma classifier
remains available through its public analysis API and is compared on the same
instruction contracts. The full Order 3 completion plan still requires actual
proof-gated check elision and its execution-state invariants, all feature and
shipping checks, relevant memory-safety evidence and the completion audit.
