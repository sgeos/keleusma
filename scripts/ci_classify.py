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

It also decides `native`, whether the native gate jobs run. They run on any event when a
changed path matches `NATIVE_PATTERNS`, including a light pull request, because
`native_codegen/` is light for the root jobs and heavy for its own. An empty or
unreadable list yields `native=true`.
"""

import argparse
import fnmatch
import sys

# Light prefixes. Each is read only by the guard binaries, the `docs-links` job and the
# `scripts` job, which a light run executes. `native_codegen/` is a detached package that
# no root job builds. The native jobs of ci.yml cover it, selected by `NATIVE_PATTERNS`
# below. V0.3.X agreed on 2026-10-25 on the condition that `documentation_links` and `comment_citations`, which walk its
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
# Every input the native suite reads, plus its build inputs. It was the path filter of
# the separate native workflow until 2026-11-04, when the operator ruled that all
# continuous integration is unified and the native jobs moved under `CI result`. The read
# set is REACH in native_codegen/tools/gate-status.sh, which reach_census.rs checks
# against the suite's reads, and native_workflow.rs checks that every REACH entry is
# matched here. A `dir/**` entry matches everything under `dir/`. Any other entry is an
# `fnmatch` pattern over the whole path. Keep one quoted entry per line, because
# native_workflow.rs and comment_citations.rs parse this tuple.
NATIVE_PATTERNS = (
    'native_codegen/**',
    'src/**',
    'examples/**',
    'compiler/kel/**',
    'docs/decisions/ABSORPTION_*_BRIEF.md',
    'docs/decisions/NATIVE_REGION_SUPPLEMENT_FORMULA.md',
    'docs/process/REVERSE_PROMPT.md',
    'docs/process/handoffs/v0.3.0.md',
    'Cargo.toml',
    'Cargo.lock',
    '.cargo/**',
    'keleusma-arena/**',
    'keleusma-macros/**',
    'keleusma-wire/**',
    'keleusma-wire-derive/**',
    '.github/workflows/ci.yml',
    'scripts/ci_classify.py',
)


def is_light(path):
    """True when no job outside the light run reads `path`."""
    if path in GUARDED or path.endswith(FULL_SUFFIXES):
        return False
    if path in LIGHT_FILES or path.startswith(LIGHT_PREFIXES):
        return True
    # A top-level Markdown file. Crate READMEs sit in crate directories, which this
    # excludes, because two of them are compiled into crate documentation.
    return '/' not in path and path.endswith('.md')


def is_native(path):
    """True when `path` is an input of the native gate."""
    for pattern in NATIVE_PATTERNS:
        if pattern.endswith('/**'):
            if path.startswith(pattern[:-2]):
                return True
        elif fnmatch.fnmatchcase(path, pattern):
            return True
    return False


def classify(event, paths):
    """Return the job outputs as a dict of strings."""
    paths = [p.strip() for p in paths if p.strip()]
    vscode = any(p.startswith(VSCODE_PREFIX) for p in paths)
    if event != 'pull_request' or not paths:
        mode = 'full'
    else:
        mode = 'light' if all(is_light(p) for p in paths) else 'full'
    native = not paths or any(is_native(p) for p in paths)
    # An empty list means the diff could not be read, so the extension check and the
    # native gate run too.
    return {
        'mode': mode,
        'native': 'true' if native else 'false',
        'vscode': 'true' if vscode or not paths else 'false',
    }


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
