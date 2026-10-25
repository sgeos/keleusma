# Signature verification bounds

> **Navigation** [Decisions](./README.md) | [Wire format](../spec/WIRE_FORMAT.md)

## Finding

V0.2.X found on 2026-10-25, while checking the Order 5 signature framing
against the reference, that `wire_format::verify_module_signature` panicked on
a short artifact. It was reproduced on v0.2.3 at `830bf20f`.

The function sliced the artifact to its declared total length and then read
the signature metadata at offsets 64 to 72, the signature at 72 to 136 and the
trailer before the total. Those offsets follow from the declared header length,
and nothing required the declared total to be at least that header plus the
four-byte trailer. A total below 72 indexed past the slice while reading the
metadata. A total below 136 indexed past it while reading the signature.

The panic was reachable from untrusted input through safe interfaces.
`Vm::load_signed_bytes`, its `_with_options` and `_unchecked` forms, module
replacement under a trust matrix and `decrypt_encrypted_signed_to_signed_bytes`
all call the verifier on bytes that carry the signature flag, before any
decode and before any key is consulted. Any artifact that set the flag and
declared a short total therefore ended the host instead of being refused. The
defect is a denial of service. It does not admit an unsigned module, since a
panic never returns success.

`parse_signature_metadata` is public and documented the precondition that the
header lies inside the buffer, and it indexed the buffer when a caller broke
the precondition.

## Repair

1. `verify_module_signature` refuses with `LoadError::Truncated` when the
   declared total is shorter than the declared header plus the trailer, which
   is the condition `module_from_wire_bytes` already applies.
2. `parse_signature_metadata` refuses with `LoadError::Truncated` when the
   buffer is shorter than the declared header.

A rejected artifact that previously failed verification may now report
`Truncated` in place of `InvalidSignature`. Both are refusals. No accepted
artifact changes.

## Evidence

`tests/signature_verify_bounds.rs` declares every total from 0 to 139 over a
signed artifact and every total from 0 to 227 over an encrypted one. Each is
refused with `Truncated` through the verifier, the signed load path and the
decrypt path, under `catch_unwind`. Against the unrepaired source three of its
four tests panic at `src/wire_format.rs:1207`. The fourth is the control. It
shows that the unmodified artifact still verifies and loads and that a wrong
key is still `InvalidSignature`.

## Observation not acted on

`decrypt_encrypted_signed_to_signed_bytes` takes the ciphertext up to the end
of the supplied buffer rather than up to the declared total, while the
signature covers only the declared total. Bytes appended after the total are
therefore unauthenticated and are fed to the cipher. This fails closed, because
the authentication tag then sits at the wrong position and AES-GCM refuses, so
it is not a vulnerability. It does make an encrypted artifact with trailing
bytes undecryptable while a signed one with trailing bytes loads. Changing it
would widen what the decrypt path accepts, so it is left for an operator
decision and recorded in the reverse prompt.
