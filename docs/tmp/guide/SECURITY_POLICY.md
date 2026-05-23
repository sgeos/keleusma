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

1. Each workflow version is signed by the certification team.
2. Production processing nodes carry only the certification team's verifying key in their trust store.
3. Audit logs (host-side, outside the script) record which signed bytecode hash ran which input.

The strict-signing gate ensures rogue scripts cannot bypass the certification process.

### Kiosk or quarantine deployment

A kiosk that should run only specific pre-installed scripts (and reject everything else):

```sh
export KELEUSMA_REQUIRE_SIGNED=1
export KELEUSMA_REQUIRE_ENCRYPTED=1
# No keys enrolled. No bytecode admissible. The kiosk is locked.
```

Combine with an enrolled key store to allow specific signed and encrypted bytecode while keeping the strict-mode posture.

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

## Cross-references

- [R49 in RESOLVED.md](../decisions/RESOLVED.md) for the strict-mode signing gate design record.
- [R50 in RESOLVED.md](../decisions/RESOLVED.md) for the encryption layer design record.
- [R42 in RESOLVED.md](../decisions/RESOLVED.md) for the underlying Ed25519 signing infrastructure.
- [WIRE_FORMAT.md](../spec/WIRE_FORMAT.md) for the wire-format details of signed and encrypted artefacts.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) for the load-time pipeline including the decryption step.
- [B24 in BACKLOG.md](../decisions/BACKLOG.md) for the future hardware-isolation work.
