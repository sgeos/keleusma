# Security Policy

## Supported versions

V0.2.x is the actively supported line. V0.1.x circulated as a pre-release and is no longer receiving security updates.

| Version | Supported |
|---------|-----------|
| 0.2.x   | ✓ |
| 0.1.x   | ✗ |

## Reporting a vulnerability

Please report security issues privately so the maintainer can prepare a fix before public disclosure. Two paths.

**Preferred: GitHub private security advisory.** Open a draft advisory at https://github.com/sgeos/keleusma/security/advisories/new. The maintainer receives a notification and can collaborate on the fix in the same forum.

**Email.** sgeos@hotmail.com. Use PGP if you have a key on file; otherwise plain mail is fine.

Please include enough information for the maintainer to reproduce the issue:

- The affected crate version (run `cargo tree` on a project that depends on the crate, or report the crate revision under inspection).
- A minimal demonstrator. Either a small Keleusma script that exhibits the misbehaviour, or a Rust host program that exercises the failure mode, or both.
- The expected and observed behaviour, including which static guarantee (totality, productivity, bounded-step, bounded-memory, safe-swap) you believe is violated.
- Your assessment of severity, if known.

## Response

The maintainer will acknowledge receipt within a few days and either confirm the issue or explain why the report does not constitute a vulnerability. Confirmed issues are tracked under the GitHub Security tab and resolved through a patch release (`0.2.N`) coordinated with the original reporter on the disclosure timeline.

## Scope

The following areas are in-scope for security reports:

- **Verifier soundness.** A program that the safe verifier admits but that exhibits unbounded execution time, unbounded memory use, non-termination in `fn` or `yield` functions, missing yields in `loop` blocks, or arena overflow at runtime is a soundness violation.
- **Ed25519 module-signature verification.** A signed module that loads through `Vm::load_signed_bytes` against a trust matrix that should not admit it; conversely a legitimately signed module that the runtime refuses.
- **Information-flow label propagation.** A program where positive labels disappear at a host-boundary crossing or where negative labels admit a value the boundary should refuse.
- **Hot code swap discipline.** A swap that leaves the data segment in a state the new module's schema does not admit.
- **Arena allocator stale-pointer detection.** A program that observes arena memory across a reset boundary that the epoch counter should have invalidated.
- **Native function ABI.** A registered native that bypasses the WCET/WCMU attestation it declared at registration.

The following are *not* security issues:

- A program that the verifier rejects when you believe the verifier should admit it. File this as a regular issue.
- A native function attested with wrong WCET/WCMU bounds that the host did not measure. The host attests; soundness against host attestation is the host's responsibility.
- A V0.2.0 program that targets the runtime under `Vm::new_unchecked`. The unchecked constructor is intentional misuse outside the WCET contract per [`docs/architecture/LANGUAGE_DESIGN.md`](docs/architecture/LANGUAGE_DESIGN.md#conservative-verification).
- A program in V0.1.x. That line is unsupported.

## Cryptographic primitives

V0.2.0's Ed25519 module signing uses the `ed25519-dalek` crate version 2 under `no_std + alloc + zeroize`. The crate has had independent audits but is not formally verified. If a vulnerability is reported against the upstream `ed25519-dalek` crate, Keleusma will track the upstream advisory and ship a patched release.

The cryptographic message convention (full framed buffer with signature payload bytes and CRC trailer bytes zeroed) is documented in [`docs/spec/WIRE_FORMAT.md`](docs/spec/WIRE_FORMAT.md).
