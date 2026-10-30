#!/usr/bin/env python3
"""Check changelog fragments and assemble them into CHANGELOG.md at release.

Every pull request into a version branch used to add its entry at the top of the same
`[Unreleased]` section, so each merge made every other open pull request conflict there.
A change now adds one file under `changelog.d/` instead, and no two changes write the
same file. See docs/process/kaizen/2026-10-29-changelog-fragments.md.

A fragment is named `YYYY-MM-DD-<slug>.<section>.md`. The section is one of the Keep a
Changelog headings in lower case. The body is one or more Markdown list items with no
headings. `check` validates every file in the directory and fails closed on any file it
does not recognise, so a misnamed fragment cannot be silently left out of a release.
`preview` prints the assembled `[Unreleased]` section. `assemble` writes it into the
changelog and deletes the fragments it used.

Within a section, fragments go above the existing entries, newest date first and then
by slug, which keeps the file's newest-first order and makes the result independent of
the order in which files were listed.
"""

import argparse
import datetime
import os
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CHANGELOG = ROOT / 'CHANGELOG.md'
DEFAULT_FRAGMENTS = ROOT / 'changelog.d'
# Keep a Changelog 1.1.0 headings, in its order. A missing heading is created in this order.
SECTIONS = ('Added', 'Changed', 'Deprecated', 'Removed', 'Fixed', 'Security')
# Files in the fragment directory that are not fragments.
IGNORED = frozenset({'README.md'})
NAME = re.compile(r'^(\d{4}-\d{2}-\d{2})-([a-z0-9]+(?:-[a-z0-9]+)*)\.([a-z]+)\.md$')
UNRELEASED = '## [Unreleased]'


class FragmentError(Exception):
    """A fragment or the changelog is malformed. The message names the file."""


def parse_name(name):
    """Return (date, slug, section) for a fragment file name, or raise FragmentError."""
    match = NAME.match(name)
    if not match:
        raise FragmentError(f'{name}: expected YYYY-MM-DD-<slug>.<section>.md')
    date, slug, section = match.groups()
    try:
        datetime.date.fromisoformat(date)
    except ValueError:
        raise FragmentError(f'{name}: {date} is not a calendar date') from None
    heading = section.capitalize()
    if heading not in SECTIONS:
        raise FragmentError(f'{name}: section must be one of {", ".join(s.lower() for s in SECTIONS)}')
    return date, slug, heading


def parse_body(name, text):
    """Return the body without surrounding blank lines, or raise FragmentError."""
    lines = text.replace('\r\n', '\n').split('\n')
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    if not lines:
        raise FragmentError(f'{name}: the fragment is empty')
    if not lines[0].startswith('- '):
        raise FragmentError(f'{name}: the first line must start a list item with "- "')
    for number, line in enumerate(lines, 1):
        if line.lstrip().startswith('#'):
            raise FragmentError(f'{name}: line {number} is a heading, which the assembler would misplace')
    return '\n'.join(lines)


def load(fragments):
    """Return every fragment as (date, slug, heading, body, path), validated.

    Fails on the first malformed file, on any subdirectory and on any file that is not a
    fragment, so that `check` can never pass while a release would omit an entry.
    """
    if not fragments.is_dir():
        raise FragmentError(f'{fragments}: the fragment directory does not exist')
    loaded = []
    for path in sorted(fragments.iterdir()):
        if path.name in IGNORED:
            continue
        if not path.is_file():
            raise FragmentError(f'{path.name}: only fragment files belong in {fragments.name}/')
        date, slug, heading = parse_name(path.name)
        try:
            text = path.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            raise FragmentError(f'{path.name}: the fragment is not UTF-8') from None
        loaded.append((date, slug, heading, parse_body(path.name, text), path))
    return loaded


def ordered(loaded):
    """Sort fragments newest date first, then by slug, so the output is deterministic."""
    by_slug = sorted(loaded, key=lambda f: f[1])
    return sorted(by_slug, key=lambda f: f[0], reverse=True)


def unreleased_bounds(lines):
    """Return (start, end) of the `[Unreleased]` block, its heading line and the next release."""
    starts = [i for i, line in enumerate(lines) if line.rstrip() == UNRELEASED]
    if len(starts) != 1:
        raise FragmentError(f'the changelog must hold exactly one "{UNRELEASED}" heading, found {len(starts)}')
    start = starts[0]
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith('## ')), len(lines))
    return start, end


def assemble_text(changelog_text, loaded):
    """Return the changelog with the fragments inserted. Pure, so tests need no files."""
    lines = changelog_text.split('\n')
    start, end = unreleased_bounds(lines)
    for heading in SECTIONS:
        bodies = [f[3] for f in ordered(loaded) if f[2] == heading]
        if not bodies:
            continue
        block = []
        for body in bodies:
            block.extend(body.split('\n'))
            block.append('')
        marker = f'### {heading}'
        found = next((i for i in range(start + 1, end) if lines[i].rstrip() == marker), None)
        if found is None:
            # A new section goes at the end of the block, before its trailing blank lines.
            at = end
            while at > start + 1 and not lines[at - 1].strip():
                at -= 1
            insert = ['', marker, ''] + block[:-1]
        elif found + 1 < len(lines) and not lines[found + 1].strip():
            # Above the existing entries, after the heading's blank line.
            at = found + 2
            insert = block
        else:
            at = found + 1
            insert = [''] + block
        lines[at:at] = insert
        end += len(insert)
    return '\n'.join(lines)


def unreleased_section(changelog_text):
    """Return the `[Unreleased]` block of a changelog as text."""
    lines = changelog_text.split('\n')
    start, end = unreleased_bounds(lines)
    return '\n'.join(lines[start:end]).rstrip('\n') + '\n'


def write_atomically(path, text):
    """Replace `path` with `text` so an interrupted run leaves the old file whole."""
    handle, temporary = tempfile.mkstemp(dir=path.parent, prefix=f'.{path.name}.')
    try:
        with os.fdopen(handle, 'w', encoding='utf-8', newline='\n') as out:
            out.write(text)
        os.replace(temporary, path)
    except BaseException:
        Path(temporary).unlink(missing_ok=True)
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    parser.add_argument('command', choices=('check', 'preview', 'assemble'))
    parser.add_argument('--changelog', type=Path, default=DEFAULT_CHANGELOG)
    parser.add_argument('--fragments', type=Path, default=DEFAULT_FRAGMENTS)
    args = parser.parse_args(argv)
    try:
        loaded = load(args.fragments)
        text = args.changelog.read_text(encoding='utf-8')
        result = assemble_text(text, loaded)
    except (FragmentError, OSError) as error:
        print(f'changelog fragments: {error}', file=sys.stderr)
        return 1
    if args.command == 'check':
        print(f'{len(loaded)} changelog fragments are valid')
    elif args.command == 'preview':
        sys.stdout.write(unreleased_section(result))
    else:
        write_atomically(args.changelog, result)
        for fragment in loaded:
            fragment[4].unlink()
        print(f'assembled {len(loaded)} changelog fragments into {args.changelog.name}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
