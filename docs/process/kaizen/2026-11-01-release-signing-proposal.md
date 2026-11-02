# Signing Public Releases with Minisign, 2026-11-01

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked on 2026-11-01 whether minisign should be added to the next public
release, meaning the signing of public releases, and noted that MCP Agent Mail uses it.
This proposal states what that project does, what a Keleusma release publishes, and a
recommendation. Nothing here is adopted until the operator rules, and the release
process document changes only through the line that cuts the release.

## What MCP Agent Mail does

**Fact**, read on 2026-11-01 from `Dicklesworthstone/mcp_agent_mail_rust`, the
maintained Rust rewrite. The original Python repository is marked deprecated, and its
releases carry no signature. Release v0.3.40 attaches six platform archives, a
`.sha256` file for each, a `SHA256SUMS` manifest and `SHA256SUMS.minisig`. The module
comment of `crates/mcp-agent-mail-cli/src/release_signing.rs` states that releases from
v0.3.31 sign the manifest with a maintainer-held key, that the installers verify the
signature over the exact manifest bytes before trusting any archive hash, and that the
self-updater verifies it in pure Rust against a pinned set of public keys selected by
key identifier, failing closed. Its stated reason is that a hash fetched from the same
place as the archive proves only that the two match, not that the release is the
maintainer's.

**Inference.** The signature earns its keep there because that project ships binaries,
installers and a self-updater, each of which downloads and runs code.

## What a Keleusma release publishes

**Fact**, from [the release process](../RELEASE_PROCESS.md). A release publishes up to
seven crates to crates.io, tags the commit with an annotated tag, and cuts a GitHub
Release whose body is the changelog section and the release record. Nothing is
attached to the GitHub Release, and no binary, installer or updater is distributed
from the project itself. `keleusma-cli` reaches users through `cargo install`.

**Fact.** crates.io records a SHA-256 checksum of each uploaded `.crate` file in its
index, and Cargo checks a download against it. **Fact, to this line's knowledge.**
crates.io offers no way to attach a publisher's detached signature to a crate. So
today a user can confirm that a crate matches the registry's index, but not that the
registry's copy is what the maintainer uploaded.

## Options

(a) **Sign a checksum manifest of the published crates.** After the publish step, the
release lists each `.crate` file that `cargo publish` packaged, with its SHA-256, in a
`SHA256SUMS` manifest, checks each sum against the crates.io index, and signs the
manifest with minisign. The manifest, its `.minisig` and the public key are attached
to the GitHub Release, and the public key is also committed, in `SECURITY.md` and as
`keys/minisign.pub`, so it can be compared across releases. A user verifies with
`minisign -Vm SHA256SUMS -P <key>` and then compares the sums with the crates Cargo
downloaded. This detects a registry copy that differs from the maintainer's upload,
and it extends without change to any binary or `.vsix` a later release attaches.

(b) **As (a), with a source archive.** Attach `git archive` of the tag and include it
in the manifest, so the source can be verified without Git.

(c) **Sign the annotated tag only**, with Git's SSH signing. This covers the source at
the tag with no new tool, but not the crates as uploaded.

(d) **GitHub artifact attestations**, keyless, through Sigstore. No key to guard, but
they attest that a CI workflow produced a file, and the crates are uploaded from the
maintainer's machine, not from CI, so they fit poorly here.

(e) **Defer** until a release first attaches binaries.

## Recommendation

(a) with (c) together, beginning at the first release after the operator has made a
key. The cost is one key and two commands per release. The key is the operator's to
create and keep, with `minisign -G` and a passphrase, held off this machine's synced
folders, because a leaked key signs anything and only a key rotation recovers from it.
No session of any line ever holds the secret key or its passphrase, and signing is a
step the operator runs by hand, like the publish. The kaizen line would supply a
script that builds and checks the manifest, with tests, and the release line would add
the steps to its process when it next cuts a release.

## Costs and limits

- **Key custody.** One more secret to keep, and a written rotation procedure, which
  Agent Mail's key identifiers make straightforward.
- **What it does not prove.** A signature shows that the key holder signed these
  bytes. It does not show that the source is free of defects, nor protect a user who
  never verifies. Most users will not verify, so the value is concentrated in
  downstream packagers and auditors, and in detection after the fact.
- **Reproducibility.** The manifest must record the `.crate` files exactly as
  uploaded. `cargo publish` writes them under `target/package/`, and the check against
  the index sum catches a mismatch. Whether `cargo package` output is byte-identical
  across machines was not checked, and the design does not rely on it.
- **Timing.** V0.2.X is preparing the V0.2.3 release now, through pull request 707.
  Adding a signing step to that release would touch its process while it is under
  way, so the operator may prefer to begin with the following release.

## Ruling and procedure

The operator ruled first-hand in this line's session on 2026-11-01, "Minisign.
Recommendation accepted.", which adopts (a) with (c) from the first release after the
operator has made a key. The ruling is recorded with the others of that day in
[the open decisions](./2026-11-01-open-decisions.md).

**Once, by the operator.** Make the key with `minisign -G -p keleusma.pub -s
<path outside synced folders>/keleusma.key`, with a passphrase. Commit the public key
as `keys/minisign.pub` and quote it in `SECURITY.md`. Set up Git's SSH signing for tags,
`git config gpg.format ssh` with `user.signingkey` naming a key the operator holds, and
register that key with GitHub as a signing key so the tag shows as verified.

**Each release, after the publish step.** The release line runs these commands, and the
operator runs the signing commands by hand.

```sh
# Every crate published in this release, as `cargo publish` packaged it.
python3 scripts/release_manifest.py target/package/<crate>-<version>.crate ... --output SHA256SUMS
minisign -Sm SHA256SUMS -s <path>/keleusma.key -t "Keleusma vX.Y.Z"   # operator
minisign -Vm SHA256SUMS -p keys/minisign.pub                          # check before attaching
gh release upload vX.Y.Z SHA256SUMS SHA256SUMS.minisig keys/minisign.pub
```

The tag itself is made with `git tag -s` in place of `git tag -a`. A user verifies with
`minisign -Vm SHA256SUMS -p minisign.pub`, then compares each line with the `.crate`
file Cargo downloaded, for example with `shasum -a 256 -c SHA256SUMS` in a directory
holding them, and checks the tag with `git tag -v vX.Y.Z`.

`scripts/release_manifest.py` refuses to write a manifest unless every file is a crate
whose packaged `Cargo.toml` names it, no crate appears twice, and its SHA-256 equals the
checksum crates.io records for that version, which must not be yanked. It never
overwrites an existing manifest. On 2026-11-01 it matched the published
`keleusma-arena` 0.3.0 against the live index, and refused the same file with one byte
appended.

These steps reach [the release process](../RELEASE_PROCESS.md) through the line that
cuts the release, which was told in the interline mailbox.
