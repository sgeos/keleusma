# Parity Correction and Signature Verification — Composition Order

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Settled 2026-08-13. **Status: DECIDED for the artifact layer. One consequence is left open and is
named at the end.**

An artifact may carry a (72,64) SECDED parity plane, which repairs a flipped bit, and an Ed25519
signature, which refuses a changed byte. Composing them forces an order that neither feature
announces, and the two orders are **indistinguishable on every undamaged artifact**. They differ
only on damaged input, which is the case the plane exists for and the case no ordinary test
produces.

## The decision

**A repair must precede the verification that authorises the bytes it produced, and every later
repair must be followed by a fresh verification.**

Stated as the invariant it protects: **no byte is executed which has been modified since the last
successful verification.** A scrub is a modification.

## Why the shorter rule "scrub first" is wrong

The first version of this analysis said verify-then-scrub is a hole outright. **It is not**, and the
correction matters because it changes what has to be built.

Write $\mathrm{Ver}(X)$ for the signature predicate, $\mathcal{S}$ for the scrub, and $M$ for the
honest artifact. Evaluated at a single instant, verify-then-scrub is safe: an adversary without the
private key cannot produce $X \neq M$ with $\mathrm{Ver}(X)$, so $\mathrm{Ver}(X)$ forces $X = M$,
and scrubbing an undamaged artifact is the identity, which is pinned by a control in
`tests/ecc_signature_ordering.rs`.

**The defect is that verification is a statement about a moment.** What a deployed system does is

```
Ver(X at t0)  at load,        install S(X at t1),        t1 > t0
```

and between $t_0$ and $t_1$ the bytes are exposed to exactly the fault process the plane was
installed to survive. **The assumption verify-then-scrub needs is that no fault occurs between the
check and the scrub, and the parity plane exists because that assumption is false.** A design cannot
rest on the negation of its own motivation.

This is the time-of-check-to-time-of-use shape, with the modifying party being the system itself
rather than a racing adversary.

## What was measured

`tests/ecc_signature_ordering.rs` signs a module whose auxiliary body carries planes, damages it,
scrubs it, and observes verification. Three results, each carrying a control.

**A single fault fails verification.** Under verify-first the module is refused, so **the parity
plane is dead weight for signed modules** and the cost of the wrong order is availability, not
integrity.

**A scrub reproduces the original bytes exactly**, so the signature computed over them verifies
against the repaired artifact. This is the precondition that makes the correct order available at
all, and it is not automatic: a scrub that normalised padding or touched a byte outside the code's
coverage would break verification on honest input.

**A three-bit fault is mis-repaired and the signature still refuses it.** The corrector reports a
successful repair and hands back bytes that are not the module. Enumerated over one 64-bit word:

| flipped bits | patterns | repaired exactly | wrongly "repaired" | invisible |
|---|---|---|---|---|
| 1 | 64 | 64 | 0 | 0 |
| 2 | 2,016 | 0 | 0 | 0 |
| 3 | 41,664 | 0 | **23,364** | 0 |
| 4 | 635,376 | 0 | 0 | **5,133** |

**The corrector is not an authority on whether it corrected, and a clean report from it is not
evidence of integrity.** Both facts are structural rather than defects of this implementation: a
distance-four code makes no claim beyond two errors, and weight-four codewords exist.

## The rules that follow

1. **Every repair is followed by a fresh verification.**
2. **The corrector's report is advisory and the signature is the authority.** A clean scan must
   never license skipping verification, however tempting the saving, because 5,133 four-bit patterns
   are reported clean.
3. **The parity plane is inside the signed span, asserted rather than inherited.** It holds because
   the signature covers the whole framed buffer and the plane lives in the auxiliary body. Nothing
   enforced that until `the_signature_covers_the_auxiliary_body_where_a_parity_plane_would_live`.
   Without it, a hostile plane steers corrections; integrity survives under the correct order, so
   the exposure is an availability attack requiring no key.
4. **For an encrypted module the scrub operates on ciphertext, before decryption.** Correcting
   plaintext modifies data the authenticated-encryption tag has already blessed. Ciphertext also
   does not tolerate a flipped bit, so for encrypted modules the plane is not an optimisation but
   the only thing between one fault and a module that cannot be decrypted.
5. **A path that repairs before it authenticates must exist**, or rule 1 is unimplementable.

## What is implemented, and what is not

**Implemented.** `module_to_signed_wire_bytes_with_ecc` produces a signed artifact whose auxiliary
body carries planes, so the composition is exercisable and is exercised. `WireView::verify_all`
scans. The plane-inside-signature property is pinned. Planes remain **off by default**, because they
change artifact bytes and byte identity is the oracle the self-hosted compiler is verified with.

**Not implemented, and it is the open consequence.** *Nothing calls a scrub at module load.* Reading
the load paths, **no path holds a mutable reference to the artifact bytes**: the virtual machine is
constructed from an owned module, the deserialiser takes a shared reference, and the zero-copy path
yields a shared slice. So a scrub cannot run before verification without new API, and the only place
it fits today is afterwards, which rule 1 forbids.

**The interface makes the unsound order the convenient one**, which is a design defect even though
no code has taken that path.

## Held by the operator

**What a host should do about a corrected word, or an uncorrectable one, is policy rather than
fact.** The decision above fixes the ORDER and leaves the RESPONSE open. The realistic options are
report-only, which needs no new API and no mutable path, and report-plus-scrub, which needs both and
buys the availability the plane was added for. This document does not choose.
