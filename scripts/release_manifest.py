#!/usr/bin/env python3
"""Build the checksum manifest of a release's published crates, for the operator to sign.

The operator adopted release signing on 2026-11-01, in
docs/process/kaizen/2026-11-01-release-signing-proposal.md. After `cargo publish`, the
`.crate` files it uploaded remain under `target/package/`. This script reads each one,
takes the crate's name and version from the `Cargo.toml` inside it, and checks its
SHA-256 against the checksum crates.io records for that version in its index. Only when
every file matches does it write a `SHA256SUMS` manifest in the format `shasum -a 256 -c`
reads. The operator then signs the manifest with minisign. The script never signs, and
it never sees a key.

The script fails closed. A file that is not a crate, a crate named twice, a version the
index does not list or lists as yanked, a checksum that differs from the index, an
unreachable index, and an existing output file each stop it before anything is
written. A manifest is therefore a statement that these exact bytes are what crates.io
serves for these versions, and the signature adds that the operator vouches for them.

Trust model. The index is fetched over HTTPS from index.crates.io and is trusted only
as a cross-check that the local files are the uploaded ones. It is not a source of
trust for the manifest. The `.crate` files are the maintainer's own build output. The
tar reader opens one member by name and extracts nothing to disk.
"""

import argparse
import hashlib
import json
import sys
import tarfile
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

INDEX_URL = 'https://index.crates.io/'
FETCH_TIMEOUT_SECONDS = 30


class ManifestError(Exception):
    """A crate or its index entry does not support a manifest."""


def index_path(name):
    """Return the sparse index path of a crate name, by the registry's layout."""
    if not name or len(name) > 64:
        raise ManifestError(f'not a crate name: {name!r}')
    lower = name.lower()
    if len(lower) == 1:
        return f'1/{lower}'
    if len(lower) == 2:
        return f'2/{lower}'
    if len(lower) == 3:
        return f'3/{lower[0]}/{lower}'
    return f'{lower[0:2]}/{lower[2:4]}/{lower}'


def crate_identity(path):
    """Return (name, version) from the Cargo.toml packaged inside a `.crate` file."""
    try:
        with tarfile.open(path, mode='r:gz') as archive:
            names = archive.getnames()
            tops = {member.split('/', 1)[0] for member in names}
            if len(tops) != 1:
                raise ManifestError(f'{path} does not have one top-level directory')
            top = tops.pop()
            member = archive.extractfile(f'{top}/Cargo.toml')
            if member is None:
                raise ManifestError(f'{path} holds no regular {top}/Cargo.toml')
            manifest = tomllib.loads(member.read().decode('utf-8'))
    except (tarfile.TarError, KeyError, UnicodeDecodeError, tomllib.TOMLDecodeError,
            OSError) as error:
        raise ManifestError(f'{path} is not a readable crate: {error}') from None
    package = manifest.get('package', {})
    name, version = package.get('name'), package.get('version')
    if not isinstance(name, str) or not isinstance(version, str):
        raise ManifestError(f'{path} has no package name and version')
    if top != f'{name}-{version}':
        raise ManifestError(f'{path} unpacks to {top}, not {name}-{version}')
    return name, version


def fetch_index(name):
    """Return the text of a crate's sparse index file."""
    url = INDEX_URL + index_path(name)
    try:
        with urllib.request.urlopen(url, timeout=FETCH_TIMEOUT_SECONDS) as response:
            return response.read().decode('utf-8')
    except (urllib.error.URLError, TimeoutError, UnicodeDecodeError) as error:
        raise ManifestError(f'the index for {name} could not be read: {error}') from None


def index_checksum(index_text, name, version):
    """Return the recorded SHA-256 of one version, failing on a missing or yanked one."""
    for line in index_text.splitlines():
        if not line.strip():
            continue
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            raise ManifestError(f'the index for {name} holds a line that is not JSON') from None
        if entry.get('name', '').lower() == name.lower() and entry.get('vers') == version:
            if entry.get('yanked'):
                raise ManifestError(f'{name} {version} is yanked')
            cksum = entry.get('cksum')
            if not isinstance(cksum, str) or len(cksum) != 64:
                raise ManifestError(f'the index records no checksum for {name} {version}')
            return cksum.lower()
    raise ManifestError(f'the index does not list {name} {version}')


def build_manifest(paths, fetch=fetch_index):
    """Return the manifest text for the given `.crate` files, checked against the index."""
    if not paths:
        raise ManifestError('no crate files were given')
    lines, seen, seen_files = [], set(), set()
    for path in sorted((Path(p) for p in paths), key=lambda p: p.name):
        if path.suffix != '.crate' or not path.is_file():
            raise ManifestError(f'{path} is not a .crate file')
        name, version = crate_identity(path)
        if (name, version) in seen or path.name in seen_files:
            raise ManifestError(f'{name} {version} is given twice')
        seen.add((name, version))
        seen_files.add(path.name)
        local = hashlib.sha256(path.read_bytes()).hexdigest()
        recorded = index_checksum(fetch(name), name, version)
        if local != recorded:
            raise ManifestError(
                f'{path.name} has SHA-256 {local}, but crates.io records {recorded}')
        lines.append(f'{local}  {path.name}')
    return '\n'.join(lines) + '\n'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    parser.add_argument('crates', nargs='+', type=Path, help='the published .crate files')
    parser.add_argument('--output', type=Path, default=Path('SHA256SUMS'),
                        help='the manifest to write, which must not exist yet')
    args = parser.parse_args(argv)
    try:
        if args.output.exists():
            raise ManifestError(f'{args.output} already exists, and is never overwritten')
        text = build_manifest(args.crates)
        with open(args.output, 'x', encoding='utf-8') as out:
            out.write(text)
    except ManifestError as error:
        print(f'release_manifest: {error}', file=sys.stderr)
        return 1
    print(f'release_manifest: wrote {args.output} for {len(args.crates)} crates')
    return 0


if __name__ == '__main__':
    sys.exit(main())
