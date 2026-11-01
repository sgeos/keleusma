# Parity Correction and Signature Verification — Composition Order

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Settled 2026-08-13, in two passes. **Status: DECIDED.** The order is fixed, the verbs that make both
responses expressible are implemented, and what remains open is scheduling policy, named at the end.

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

**Superseded by the section below, and the original text is worth keeping because the conclusion it
reached was wrong.** It said no load path holds a mutable reference, therefore a scrub could not run
before verification without new API, therefore the interface made the unsound order the convenient
one. The first two clauses are correct and the inference is not. **The fix was not to add a mutable
LOAD path**, which would have pushed `&mut` into the common path and cost the zero-copy and
worst-case-memory properties the reader exists for. It was to add a separate mutating VERB that the
load path never calls.

## Report and scrub are separate verbs, and scrub returns no artifact

**Settled 2026-08-13, second pass.** The response is exposed as two verbs a host may use or ignore,
so the runtime supplies mechanism and the host supplies policy.

```
report:  &[u8]      -> Option<EccReport>     WireView::verify_all, verify_region, EccPlane::scan
scrub:   &mut [u8]  -> Option<EccReport>     keleusma_wire::scrub
```

**Report already existed.** The reading verbs were built first and deliberately do not repair,
because the read path borrows the caller's buffer. Only the scrub verb was missing, so this finishes
a split rather than inventing one.

**Scrub returns counts, not an artifact.** That is the structural condition. Handing back repaired
bytes would make the unsound order convenient, which is the defect this document exists to prevent.
There is nothing to load, so the repaired buffer must be re-authenticated by whatever authorised it
originally.

**The `&mut [u8]` signature makes the unsound order unrepresentable on the zero-copy path.** A live
`WireView`, or a runtime reading the artifact in place, holds `&[u8]`, so `&mut [u8]` cannot be
obtained while either exists. Scrubbing must precede construction, and constructing again re-runs
the checks. **The guarantee is weaker where the artifact is copied out**, since an owned decode no
longer borrows the buffer; there the invariant is the caller's to honour and the documentation says
so.

**Optional means optional.** A host on read-only storage cannot scrub, and report-only is also the
bounded-time choice. A host that calls neither verb gets exactly today's behaviour, which the
documentation states rather than implying that a plane's presence confers protection.

**`EccReport::is_clean` is documented as not an integrity check.** It means the code noticed nothing.
5,133 of 635,376 four-bit patterns are reported clean while the word is wrong, and a caller skipping
a cryptographic check on the strength of it would accept every one.

## Scheduling is the host's, decided 2026-08-13

**A host may scrub on its own schedule.** Operator decision. Nothing in the runtime scrubs, nothing
requires a host to, and no schedule is imposed.

What is **not** the host's choice is what follows a repair. The invariant stands: a repair must be
followed by a fresh verification. So the module-level surface is a pair, and the sound composition is
the shorter call:

```
scrub_module_bytes(&mut [u8])                    -> Option<EccReport>
scrub_and_verify_signed(&mut [u8], &[key])       -> Result<EccReport, LoadError>
```

`scrub_module_bytes` exists because the container is the auxiliary body **alone**, and a host that
passed the whole framed buffer to the container-level verb would have the parse fail on its magic and
the scrub silently repair nothing. That is not hypothetical; it happened in this project's own test
before the module-level verb existed.

`scrub_and_verify_signed` repairs and then verifies what the repair produced, and **returns the
report only if verification passed**. There is no path through it that yields a report without
having verified, which inverts the convenience the earlier design had backwards.

**What each outcome means to a host.** A clean report means the module was undamaged. A report with
corrections means the module had taken repairable damage, the buffer now holds the publisher's bytes,
and **writing it back restores the margin** before a second fault lands in a word that already
carries one. An error means the module is not the one that was signed, and it deliberately does not
distinguish unrepairable damage from a wrong repair from tampering, because none of them may load.

**One operational consequence, stated because it constrains scheduling.** On the zero-copy path a
running virtual machine borrows the artifact, so a scrub cannot run underneath it; the cycle is drop,
scrub, reconstruct, and reconstruction re-runs the checks. That is the invariant being enforced by
the borrow checker rather than a limitation, and it means a scrub is a reload rather than a
background touch-up. A host wanting to scrub without interrupting execution must hold its own copy of
the artifact bytes and swap it in.

## Held by the operator

Nothing. The order is fixed, the verbs are implemented, scheduling is the host's, and the unsound
order is unrepresentable wherever the reader borrows the buffer.
