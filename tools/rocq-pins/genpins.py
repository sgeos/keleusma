#!/usr/bin/env python3
"""Regenerate a Rocq directory's Statements.v as fully qualified pins.

Usage: genpins.py <srcdir> <namespace> <builddir> <requires> <flags...>

<srcdir>    the directory under docs/proofs/rocq holding Statements.v and check.sh
<namespace> the directory's logical namespace, for example RegionCore
<builddir>  where its check.sh left compiled output, the working directory
            for rocq compile
<requires>  semicolon-separated "NS:Mod1,Mod2" groups to Require, the
            directory's own modules listed without Statements
<flags...>  the -Q flags check.sh uses, relative to <builddir>

The current Statements.v, written with bare names under Import, must already
be compiled in <builddir> as <namespace>.Statements. The script prints each of
its items with Set Printing Fully Qualified, renames Statements.v to Claims.v,
writes a new Statements.v that imports no project module and pins each item of
Claims.v by its printed type, and updates check.sh to compile and scan
Claims.v and to expect the new pin count.

Why. An audit of 2026-11-18 showed by execution that pins written with bare
names under Import can be bypassed by redefining a used name in an imported
module. Fully qualified pins in a file importing no project module cannot be.
The printed text comes from the tree as it is when the script runs, so running
it on a tampered tree records the tampering. Review the diff it produces.
"""
import re
import subprocess
import sys
from pathlib import Path


def main() -> None:
    src = Path(sys.argv[1])
    ns = sys.argv[2]
    build = Path(sys.argv[3])
    requires = [g for g in sys.argv[4].split(';') if g]
    flags = sys.argv[5:]

    old = (src / 'Statements.v').read_text()
    items = []
    for line in old.splitlines():
        m = re.match(r"^(Definition|Lemma|Example|Theorem|Corollary)\s+([A-Za-z0-9_']+)\b", line)
        if m:
            items.append(m.group(2))
    if not items:
        sys.exit('no items found')

    req_lines = []
    for g in requires:
        gns, mods = g.split(':')
        req_lines.append(f"From {gns} Require {' '.join(mods.split(','))}.")

    printer = build / 'PrintPins.v'
    printer.write_text('\n'.join(req_lines + [f'From {ns} Require Statements.',
                                             'Set Printing Fully Qualified.',
                                             'Set Printing Width 100000.']
                                 + [f'Check {ns}.Statements.{i}.' for i in items]) + '\n')
    res = subprocess.run(['rocq', 'compile', *flags, 'PrintPins.v'], cwd=build,
                         capture_output=True, text=True)
    if res.returncode != 0:
        sys.exit('printing failed:\n' + res.stdout + res.stderr)

    blocks = {}
    cur = None
    for line in res.stdout.splitlines():
        hdr = re.match(rf"^{re.escape(ns)}\.Statements\.([A-Za-z0-9_']+)$", line.strip())
        if hdr and not line.startswith(' '):
            cur = hdr.group(1)
            blocks[cur] = []
        elif cur is not None:
            blocks[cur].append(line.strip())
    missing = [i for i in items if i not in blocks]
    if missing:
        sys.exit(f'no printed type for {missing}')

    pins = []
    for i in items:
        t = ' '.join(blocks[i]).strip()
        if not t.startswith(': '):
            sys.exit(f'unexpected output for {i}: {t[:80]}')
        t = t[2:]
        name = i if i.startswith('pin_') else 'pin_' + i
        pins.append(f'Definition {name} :\n  {t}\n  := {ns}.Claims.{i}.\n')

    header = '''(** * Statements.v -- the pinned statements, fully qualified.

    Each pin is a definition whose stated type must match the type of one
    item of [Claims.v] up to conversion, so a statement weakened by an added
    premise or a changed conclusion fails to compile.  Each stated type is the
    type Rocq printed for the item with fully qualified names on the date of
    regeneration, by tools/rocq-pins/genpins.py.  This file imports no module
    of the project, so no definition elsewhere can shadow a name it uses.  An
    audit of 2026-11-18 showed by execution that pins written with bare names
    under [Import] could be bypassed by redefining such a name in an imported
    module.  [Claims.v] holds the statements as first written, with their
    proofs, including the proved equations that fix definitions. *)

'''
    new_req = list(req_lines)
    body = header + '\n'.join(new_req + [f'From {ns} Require Claims.', '']) + '\n' + '\n'.join(pins)

    claims = re.sub(r'^\(\*\* \* Statements\.v -- ', '(** * Claims.v -- formerly Statements.v, ', old, count=1)
    claims = re.sub(r'The script requires the number of\s+pins\s+below\.',
                    'Statements.v pins every item of this\n    file with fully qualified names, and the script counts the pins there.',
                    claims, count=1)
    chk = (src / 'check.sh').read_text()
    chk2 = re.sub(r'^(modules=\([^)]*?)\bStatements\)', r'\1Claims Statements)', chk, count=1, flags=re.M)
    chk2 = re.sub(r'^for m in Statements (\w+); do', r'for m in Statements Claims \1; do', chk2, count=1, flags=re.M)
    chk2 = re.sub(r'\bStatements\.(?!v\b)(?=[A-Za-z_])', 'Claims.', chk2)
    chk2 = re.sub(r'^pins_expected=\d+', f'pins_expected={len(items)}', chk2, count=1, flags=re.M)
    if chk2.count('Claims Statements)') != 1 or 'for m in Statements Claims' not in chk2:
        sys.exit('check.sh edits did not apply, nothing written')
    (src / 'Claims.v').write_text(claims)
    (src / 'Statements.v').write_text(body)
    (src / 'check.sh').write_text(chk2)
    printer.unlink()
    print(f'{src}: {len(items)} pins regenerated')


if __name__ == '__main__':
    main()
