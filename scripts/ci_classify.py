#!/usr/bin/env python3
"""Classify a continuous-integration run as full or light from its changed files.

Read by the `changes` job of `.github/workflows/ci.yml`. A pull request whose every
changed file is read only by the documentation guards, the link check and the script
checks runs those alone. Everything else, and every run that is not a pull request,
runs the full matrix. See docs/process/kaizen/2026-10-25-ci-load-proposal.md.

The rule fails closed. An unknown event, an empty or unreadable file list, or any path
outside the light set yields `full`. Adding a path here widens what a pull request can
skip, so `tests/ci_light_path_coverage.rs` checks the guarded documents against the
heavy tests that read them.
"""

import argparse
import sys

# Light prefixes. Each is read only by the guard binaries, the `docs-links` job and the
# `scripts` job, which a light run executes. `native_codegen/` is a detached package that
# no job here builds. `native.yml` on `v0.3.0` covers it, and V0.3.X agreed on 2026-10-25
# on the condition that `documentation_links` and `comment_citations`, which walk its
# files, stay in the light run. `test_ci_classify.py` checks that condition.
LIGHT_PREFIXES = ('docs/', 'book/', 'scripts/', 'assets/', '.github/ISSUE_TEMPLATE/', 'native_codegen/')
# The guard binaries a light prefix depends on, which must stay in the `guards` job.
LIGHT_PREFIX_GUARDS = {'native_codegen/': ('documentation_links', 'comment_citations')}
# Light single files that no job builds from.
LIGHT_FILES = frozenset({'.github/PULL_REQUEST_TEMPLATE.md', 'CITATION.cff', 'LICENSE', 'flake.nix'})
# Documents a heavy test compiles in or reads, so a change to one needs the full run.
# `selfhost_codegen` includes the roadmap with `include_str!`.
GUARDED = frozenset({'docs/roadmap/V0_2_X_ROADMAP.md'})
# A `.kel` file anywhere may be a stage source or a corpus member of a heavy test.
FULL_SUFFIXES = ('.kel',)
VSCODE_PREFIX = 'editors/vscode/'


def is_light(path):
    """True when no job outside the light run reads `path`."""
    if path in GUARDED or path.endswith(FULL_SUFFIXES):
        return False
    if path in LIGHT_FILES or path.startswith(LIGHT_PREFIXES):
        return True
    # A top-level Markdown file. Crate READMEs sit in crate directories, which this
    # excludes, because two of them are compiled into crate documentation.
    return '/' not in path and path.endswith('.md')


def classify(event, paths):
    """Return the job outputs as a dict of strings."""
    paths = [p.strip() for p in paths if p.strip()]
    vscode = any(p.startswith(VSCODE_PREFIX) for p in paths)
    if event != 'pull_request' or not paths:
        mode = 'full'
    else:
        mode = 'light' if all(is_light(p) for p in paths) else 'full'
    # An empty list means the diff could not be read, so the extension check runs too.
    return {'mode': mode, 'vscode': 'true' if vscode or not paths else 'false'}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--event', required=True)
    args = parser.parse_args(argv)
    try:
        paths = sys.stdin.read().splitlines()
    except (OSError, UnicodeError):
        paths = []
    for key, value in sorted(classify(args.event, paths).items()):
        print(f'{key}={value}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
