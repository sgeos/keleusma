# Security Policy

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Operator-facing guide for the V0.2.1 strict-mode signing and encryption policies. Covers key generation, policy activation, deployment scenarios, and the trust model.

## Audience

Operators deploying `keleusma-cli` in environments where bytecode execution must be cryptographically constrained. Examples include locked-down production servers, air-gapped workstations, regulated workflow execution, and embedded fleet deployments. The mechanisms described here are optional and additive; the V0.2.0 permissive behaviour remains the default.

## The four policy states

The CLI carries two independent strict modes. Either may be active in any combination.

| Signing gate | Encryption gate | Policy summary |
|---|---|---|
| Inactive | Inactive | V0.2.0 permissive default. Accepts source files, unsigned bytecode, signed bytecode with `--verifying-key`, encrypted bytecode with `--decryption-key`. |
| Active | Inactive | Source files and unsigned bytecode rejected. Signed bytecode admitted only when the signature validates against an enrolled signer. The `--verifying-key` command-line argument is rejected. |
| Inactive | Active | Unencrypted bytecode rejected (signed or unsigned). Encrypted bytecode admitted only when an enrolled decryption key matches the artefact's recipient identifier. The `--decryption-key` command-line argument is rejected. |
| Active | Active | Strict signing AND strict encryption. Bytecode must be both signed by an enrolled signer and encrypted to an enrolled recipient. |

## Activating strict signing

Three knobs activate strict signing, in precedence order.

1. **`KELEUSMA_TRUSTED_KEYS_DIR` environment variable** points at a directory of `*.pub` files. Each file holds a 32-byte Ed25519 verifying key.
2. **Platform-conventional directory**: `/etc/keleusma/trusted_keys` on Unix-like systems, `%PROGRAMDATA%\keleusma\trusted_keys` on Windows. Used when the environment variable is unset.
3. **`KELEUSMA_REQUIRE_SIGNED=1`** environment variable forces strict mode even with an empty trust store. Fail-closed for everything.

Strict signing activates when the trust store is non-empty OR when the force-strict variable is set.

Discovery is fail-closed. A malformed key file (wrong size, invalid Ed25519 encoding) causes the CLI to refuse to start with a clear diagnostic. This prevents partial-trust-list edge cases.

## Activating strict encryption

The encryption gate uses parallel mechanisms. Three knobs in precedence order:

1. **`KELEUSMA_DECRYPTION_KEYS_DIR` environment variable** points at a directory of `*.seed` files. Each file holds a 32-byte X25519 private key.
2. **Platform-conventional directory**: `/etc/keleusma/decryption_keys` on Unix-like systems, `%PROGRAMDATA%\keleusma\decryption_keys` on Windows.
3. **`KELEUSMA_REQUIRE_ENCRYPTED=1`** environment variable forces strict encryption mode even with an empty decryption-key store.

Strict encryption requires the `encryption` Cargo feature to be enabled on the runtime. The `keleusma-cli` binary ships with the feature on; embedders building their own runtime opt in explicitly.

## Key generation

The `keleusma keygen` command generates 32-byte seed and 32-byte public-key files for either signing (Ed25519) or encryption (X25519). The two key kinds are not interchangeable.

```sh
# Ed25519 signing keypair (default).
keleusma keygen --seed sign.seed --public sign.pub

# Equivalent explicit form.
keleusma keygen --kind signing --seed sign.seed --public sign.pub

# X25519 encryption keypair.
keleusma keygen --kind encryption --seed enc.seed --public enc.pub
```

The seed file is the private half and must be treated as a secret. On Unix systems, `keygen` tightens permissions on the seed file to mode 0600 (owner read/write only) as a defence-in-depth measure. The public-key file is safe to distribute to verifiers (for signing) or compilers producing artefacts for this host (for encryption).

`keygen` refuses to overwrite an existing seed or public-key file. Rotation requires explicit removal of the old files first.

## Compiling artefacts

The `keleusma compile` command produces unsigned, signed, or signed-and-encrypted artefacts.

```sh
# Unsigned. Will not run under strict signing.
keleusma compile script.kel -o script.kel.bin

# Signed. Source must declare the entry function with the `signed` modifier.
keleusma compile script.kel --signing-key sign.seed -o script.kel.bin

# Signed and encrypted to a specific recipient.
keleusma compile script.kel \
    --signing-key sign.seed \
    --encryption-key recipient.pub \
    -o script.kel.bin
```

Encryption requires signing. The wire format ties the two together because the signature covers the encrypted body; an adversary cannot strip the encryption layer and substitute cleartext without invalidating the signature.

## Running artefacts

The `keleusma run` command executes a compiled artefact. The CLI auto-detects the bytecode shape from the framing header.

In permissive mode (no enrolled keys, no force-strict flag):

```sh
# Unsigned bytecode runs unconditionally.
keleusma run script.kel.bin

# Signed bytecode runs if the signature validates against --verifying-key.
keleusma run script.kel.bin --verifying-key sign.pub

# Encrypted bytecode runs if the signature validates AND the decryption key matches.
keleusma run script.kel.bin --verifying-key sign.pub --decryption-key host.seed
```

In strict mode, the command-line key flags are rejected. The CLI uses only enrolled keys from the system-managed trust stores.

## Deployment scenarios

### Air-gapped office distribution

A head office distributes operational scripts to remote employees on air-gapped workstations.

**Initial provisioning** (trusted personnel, on-site):

1. Generate per-employee X25519 keypairs at the head office. Deliver each employee's private key to that employee's workstation through trusted personnel.
2. Generate the head office's Ed25519 signing keypair. Keep the seed at the head office. Distribute the public key to each workstation.
3. Install the `keleusma-cli` binary on each workstation.
4. Enrol the head office's verifying key into each workstation's `/etc/keleusma/trusted_keys/` directory.
5. Enrol that workstation's specific X25519 private key into `/etc/keleusma/decryption_keys/`.

Both strict modes are now active on each workstation.

**Per-script distribution** (head office):

```sh
keleusma compile script.kel \
    --signing-key head_office.seed \
    --encryption-key workstation_42.pub \
    -o script_for_42.kel.bin
```

The artefact is encrypted to workstation 42 specifically. It will not decrypt on any other workstation even if intercepted in transit.

**Delivery**: courier-delivered storage media (USB sticks, removable drives). The shebang-equipped artefact is executable directly through the operating system's shell.

**Execution** (workstation 42):

```sh
./script_for_42.kel.bin
```

The CLI enforces both strict modes automatically. The script runs if signed by the head office and decryptable with workstation 42's enrolled key. Otherwise it is rejected.

**Captured artefact**: a stolen artefact on the delivery channel is opaque ciphertext. The adversary cannot read its contents.

**Compromised workstation**: a compromised workstation reveals only its own private key. Artefacts intended for other workstations remain confidential.

### Production server fleet

A production environment runs only release-team-signed builds.

```sh
# On each production server, install the build server's verifying key:
sudo cp /tmp/release_key.pub /etc/keleusma/trusted_keys/release.pub

# The CLI now enforces strict signing.
keleusma run /opt/keleusma/scripts/job.kel.bin
```

Local operators on the production server cannot run unauthorised scripts; the strict-signing policy rejects them. The `--verifying-key` command-line argument is rejected, preventing local relaxation.

### Regulated workflow execution

A medical informatics pipeline must demonstrate to auditors that only validated workflow scripts ran on patient data.

1. Each workflow version is signed by the approval team.
2. Production processing nodes carry only the approval team's verifying key in their trust store.
3. Audit logs (host-side, outside the script) record which signed bytecode hash ran which input.

The strict-signing gate ensures rogue scripts cannot bypass the approval process.

### Kiosk or quarantine deployment

A kiosk that should run only specific pre-installed scripts (and reject everything else):

```sh
export KELEUSMA_REQUIRE_SIGNED=1
export KELEUSMA_REQUIRE_ENCRYPTED=1
# No keys enrolled. No bytecode admissible. The kiosk is locked.
```

Combine with an enrolled key store to allow specific signed and encrypted bytecode while keeping the strict-mode posture.

## Daemon deployments and tick-interval cadences

The CLI's productive-divergent loop runner is the primary path for long-lived signed-and-encrypted daemon workloads. The `--tick-interval <duration>` flag rate-limits the loop. See the [CLI README](../../keleusma-cli/README.md) for the flag reference and the script-side natives `shell::set_tick_interval` and `shell::tick_interval`.

### Fail-fast configuration

The setter native can fail at runtime if the supplied string is not a valid humanized duration. Call `shell::set_tick_interval` at the top of the loop body so a malformed argument surfaces on the first iteration and the daemon terminates before any operational state is built up. The recommended pattern is:

```keleusma
loop main(tick: Word) -> Word {
    let _ = shell::set_tick_interval("1s");
    // Operational logic from here.
    ...
}
```

A daemon that calls the setter mid-loop based on a runtime decision can mask a configuration error for an extended period. Operators should treat the interval as a static configuration knob.

### Memory residency as a feature

Deliberately keeping a Keleusma loop daemon in memory addresses a class of operational scenarios where allocation failures are expected. When the host is under memory pressure such that fresh process launches fail, an already-resident daemon retains its mapped pages and continues to execute. This is a documented use case for the CLI loop runner.

Pattern: run a Keleusma loop daemon with a small footprint (single-digit megabytes of resident set size; see [`METRICS.md`](./METRICS.md)) and a long tick interval. The daemon remains scheduled even when the system cannot launch new processes, and is available for diagnostic or recovery work that requires already-loaded code.

The default zero-interval behaviour spins as fast as the script yields, which is appropriate for batch processing but not for memory-resident-on-call deployments. Set an explicit interval (`--tick-interval 30s`, `--tick-interval 5m`, depending on cadence needs) when running as a memory-resident daemon.

### Failing cleanly under memory pressure

The runner also fails cleanly when a host genuinely cannot satisfy a program's memory. A verified program's worst-case arena is bounded and known, so the CLI sizes the arena to that bound and allocates it fallibly. A host that cannot provide it exits with an `out of memory: this program needs an N-byte arena` diagnostic and a non-zero status rather than aborting with `SIGABRT`, so a supervisor or orchestrator can observe and react. To provision or qualify a host in advance, `keleusma run <file> --print-memory` reports the program's worst-case arena footprint, the total along with its persistent and transient parts, and exits without running.

### Cadences longer than four weeks

The `--tick-interval` flag rejects intervals longer than four weeks. Operators with monthly or quarterly cadences have two options.

**External scheduler**. Use cron, systemd timers, or the equivalent on the deployment platform to invoke a one-shot Keleusma script at the desired cadence. This approach is appropriate when the only requirement is timing.

**Noop yield cycles**. Run a Keleusma loop daemon with a shorter interval (one hour, one day) and count internal ticks against the desired cadence. Most iterations do nothing but yield. Periodic iterations perform the actual work.

```keleusma
loop main(tick: Word) -> Word {
    let _ = shell::set_tick_interval("1d");
    // Tick counts days. Real work runs every thirtieth day.
    let _ = if tick % 30 == 0 {
        // Operational logic.
        ...;
    };
    let _ = yield tick;
    tick
}
```

This approach is appropriate when memory residency is part of the requirement (see above). It also preserves the signed-and-encrypted delivery model end-to-end; the script never exits and is never relaunched.

## Key compromise, revocation, and rotation

The model rests on two private keys whose consequences on compromise are very different. An operator should understand the asymmetry and protect each key accordingly.

### The two private keys and their blast radii

- **The signing seed (`sign.seed`, held by the producer) is the critical secret.** Every host that enrols the corresponding verifying key trusts anything signed by it. A leaked signing seed lets an adversary forge artefacts that pass strict-signing verification on the entire fleet, a total loss of authenticity across every deployment that enrolled the key. Protect it the most. Keep it on an offline or air-gapped signing host, restrict access to the smallest possible set of operators, and prefer a hardware security module or equivalent where the threat model warrants it. This is the single most consequential secret in the system, and the leaked-signing-key case is the one that undermines the whole model.
- **A decryption seed (`dest.seed`, held by a recipient) has a bounded blast radius.** A leaked decryption seed lets an adversary read artefacts encrypted to that one recipient, and only that recipient. Artefacts for other recipients stay confidential, as noted under the air-gapped scenario above. There is no forward secrecy at the recipient-key level, so a leaked decryption seed also exposes any past artefacts encrypted to it that an adversary retained, not only future ones. Generate each recipient's keypair on the recipient host so the seed never transmits, and rely on the mode-0600 permissions `keygen` sets on Unix.

### No expiry, and revocation is manual

Enrolled keys are raw Ed25519 and X25519 keys with no embedded validity period, so an enrolled key is trusted until an operator removes it. There is no certificate-revocation list and no online revocation check; that is inherent to the air-gapped enrolled-key model. Revoking a key therefore means removing its file from the trust store or decryption-key store on every affected host, one host at a time, and the revocation takes effect on a host only once that host has been updated. Plan for this latency. A fleet-wide revocation is a deployment operation, not an instant broadcast.

### Rotation procedure

Rotate on a schedule as a matter of policy, and immediately on suspected compromise. Signing-key rotation is:

1. Generate a new signing keypair on the signing host (`keleusma keygen --seed sign.v2.seed --public sign.v2.pub`).
2. Distribute the new verifying key and enrol it in every host's trust store, authenticated out of band (see the residual-risk note on enrolment authenticity), keeping the old key enrolled during the transition.
3. Re-sign the artefacts that must remain runnable with the new key.
4. Once every host carries the new key and every live artefact is re-signed, remove the old verifying key from every trust store. The old signing seed is then powerless and should be destroyed.

Decryption-key rotation is symmetric. Generate a new recipient keypair on the recipient host, distribute the new public key to producers, re-encrypt the artefacts that recipient still needs, and remove the old decryption seed once nothing in flight is encrypted to it.

## Trust model

**Trusted components**:

- The `keleusma-cli` binary itself. An adversary who replaces the binary defeats all policies; binary integrity is the operator's responsibility (filesystem permissions, executable signing at the OS layer, integrity-monitoring tools).
- The trust-store directories. An adversary who modifies the enrolled-keys files can extend the trust list. Use filesystem permissions (root-owned, mode 0644 on key files, mode 0755 on the directory) to prevent unprivileged tampering.
- The host operating system. Adversaries with root or kernel-level access can defeat any user-space mechanism.

**Untrusted components**:

- Delivery channels (network, USB, courier). Bytecode in transit is assumed to be inspectable and potentially substitutable.
- Bytecode files on disk. An adversary who replaces a bytecode file cannot get it to run unless the substitute also passes the active policies.
- Local operators on the deployment host. Unprivileged users cannot relax the policy; the system-managed trust stores override command-line arguments.

**Known residual risk**:

- An adversary with memory access on the running runtime can recover decrypted plaintext from RAM after the decryption step. Closing this gap requires hardware isolation (TrustZone-M on Cortex-M55, equivalent on other platforms). This work is tracked as B24 in [`docs/decisions/BACKLOG.md`](../decisions/BACKLOG.md).
- Side-channel attacks against the cryptographic operations (timing, power analysis) are out of scope for the current implementation. The pure-Rust crypto crates (`ed25519-dalek`, `x25519-dalek`, `aes-gcm`) provide constant-time implementations of the core primitives but the broader host environment may leak through other channels.
- No anti-replay or freshness binding. A signature attests origin and integrity, not recency. An artefact carries no timestamp, sequence number, or nonce that the host checks, and the host keeps no record of artefacts it has already run, so an adversary who retained a previously valid artefact can re-deliver it and the host will verify and run it. Where running a superseded but once-valid artefact is harmful, for example an old workflow script, enforce freshness outside the model: deliver over an integrity-controlled channel, rotate the signing key between supersessions so the old artefact stops verifying, or track artefact hashes host-side.
- Classical, not post-quantum, cryptography. Ed25519 and X25519 are not quantum-resistant. An adversary who records encrypted artefacts today could decrypt them once a cryptographically relevant quantum computer exists, the harvest-now-decrypt-later threat, which matters for long-lived confidential payloads. The wire format reserves a `scheme_id` byte for migration to a post-quantum scheme without an ABI break, but no such scheme is implemented today.
- Enrolment authenticity is the operator's responsibility. The trust stores protect against tampering after enrolment, but the initial public-key exchange must be authenticated out of band. Enrolling a verifying key an adversary substituted makes the fleet trust the adversary's signatures, and encrypting to a recipient public key an adversary substituted discloses the payload to the adversary. Verify key provenance, by fingerprint comparison over a separate channel, a trusted courier, or an existing trust anchor, before enrolment.
- Metadata is not concealed. Artefact size, delivery timing, and the `recipient_key_id` carried in an encrypted artefact's header are visible to anyone who observes the channel. The contents are protected; the fact and shape of a delivery are not. This is minor for physical air-gapped transfer and more relevant over an observable network.

## Cross-references

- [R49 in RESOLVED.md](../decisions/RESOLVED.md) for the strict-mode signing gate design record.
- [R50 in RESOLVED.md](../decisions/RESOLVED.md) for the encryption layer design record.
- [R42 in RESOLVED.md](../decisions/RESOLVED.md) for the underlying Ed25519 signing infrastructure.
- [WIRE_FORMAT.md](../spec/WIRE_FORMAT.md) for the wire-format details of signed and encrypted artefacts.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) for the load-time pipeline including the decryption step.
- [B24 in BACKLOG.md](../decisions/BACKLOG.md) for the future hardware-isolation work.
