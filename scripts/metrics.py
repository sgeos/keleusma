#!/usr/bin/env python3
"""Collect the project's tracked metrics into one report, overwritten on every run.

The operator ruled on 2026-11-07 that all reasonable metrics are tracked. Their choice and
definitions are in docs/process/kaizen/2026-11-07-metrics.md. This script computes every
metric that a checkout, cargo, cloc and the GitHub interface can produce reproducibly, and
writes them with the commit and time to target/metrics/metrics.json, so the file always
describes the tree and the moment that produced it. The CI `footprint` job runs it on every
full run and keeps the file, which is the history.

Each collector either returns its figures or reports itself unavailable with a reason, and
the report keeps both. A collector never fails silently, because a metric that quietly
stopped being computed would read as a steady value. `--require` names collectors that must
succeed, and the run fails when one does not, which is how CI keeps them honest.

Sources are counted, not interpreted. A count of `#[test]` attributes is a source inventory
and not a run, and the GitHub figures cover a window of days, so a figure is only as fresh
as the run that wrote it.
"""

import argparse
import datetime as dt
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = Path('target/metrics/metrics.json')
LOC_CATEGORIES = ('runtime', 'crates', 'tests', 'detached', 'tooling', 'examples',
                  'example-docs', 'docs', 'package-docs', 'process', 'translations')
RUST_AREAS = {'runtime': ['src'], 'tests': ['tests'],
              'crates': ['keleusma-macros', 'keleusma-arena', 'keleusma-wire',
                         'keleusma-wire-derive', 'keleusma-cli', 'keleusma-bench'],
              'detached': ['compiler', 'native_codegen', 'keleusma-lsp', 'keleusma-wasm',
                           'fuzz', 'footprint', 'examples']}


KELEUSMA_LINES = {'kaizen', 'proofs', 'v0.2.3', 'v0.3.0'}


class Unavailable(Exception):
    """The collector cannot produce its figures in this environment."""


def run(args, cwd=ROOT, check=True):
    try:
        result = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=600)
    except (FileNotFoundError, subprocess.TimeoutExpired) as error:
        raise Unavailable(f'{args[0]} could not run: {error}') from None
    if check and result.returncode != 0:
        raise Unavailable(f'{" ".join(args[:3])} failed: {result.stderr.strip()[:300]}')
    return result.stdout


def tracked(*pathspecs):
    out = run(['git', '-c', 'core.quotePath=false', 'ls-files', '--', *pathspecs])
    return [ROOT / line for line in out.splitlines() if line]


def code_lines(path):
    """The lines of a source file that are not `//` comments."""
    return [l for l in path.read_text(errors='replace').splitlines()
            if not l.lstrip().startswith('//')]


# --- collectors --------------------------------------------------------------------------

def lines(_args):
    """Lines by category, as scripts/loc.sh counts them, from cloc's JSON output."""
    if not shutil.which('cloc'):
        raise Unavailable('cloc is not installed')
    result, tmp = {}, ROOT / 'target' / 'metrics' / 'loc-files.txt'
    tmp.parent.mkdir(parents=True, exist_ok=True)
    for category in LOC_CATEGORIES:
        tmp.write_text(run(['sh', 'scripts/loc.sh', '--files', category]))
        if not tmp.read_text().strip():
            result[category] = {'files': 0, 'code': 0, 'comment': 0, 'blank': 0}
            continue
        data = json.loads(run(['cloc', '--json', '--quiet', '--read-lang-def=tools/cloc.lang',
                               "--force-lang=Linker Script,x", f'--list-file={tmp}']))
        total = data['SUM']
        result[category] = {'files': total['nFiles'], 'code': total['code'],
                            'comment': total['comment'], 'blank': total['blank'],
                            'by_language': {k: v['code'] for k, v in data.items()
                                            if k not in ('header', 'SUM')}}
    tmp.unlink()
    shipping = result['runtime']['code'] + result['crates']['code']
    result['derived'] = {
        'shipping_code': shipping,
        'test_to_shipping_ratio': round(result['tests']['code'] / shipping, 3) if shipping else None,
        'runtime_comment_density': round(result['runtime']['comment'] / result['runtime']['code'], 3)
        if result['runtime']['code'] else None}
    return result


def tests(_args):
    """Test functions and ignored tests by area, from the sources."""
    result = {}
    for area, roots in RUST_AREAS.items():
        files = [p for p in tracked(*roots) if p.suffix == '.rs']
        text = [code_lines(p) for p in files]
        result[area] = {
            'test_functions': sum(sum(1 for l in t if l.strip().startswith('#[test]')) for t in text),
            'ignored': sum(sum(1 for l in t if l.strip().startswith('#[ignore')) for t in text)}
    result['integration_test_files'] = len([p for p in tracked('tests') if p.suffix == '.rs'
                                            and p.parent.name == 'tests'])
    result['script_test_files'] = len(tracked(':(glob)scripts/test_*.py'))
    return result


def unsafe_code(_args):
    """`unsafe` blocks, functions and implementations in the shipping runtime."""
    counts = {'blocks': 0, 'functions': 0, 'impls': 0}
    for path in tracked('src'):
        if path.suffix != '.rs':
            continue
        for line in code_lines(path):
            counts['functions'] += len(re.findall(r'\bunsafe\s+fn\b', line))
            counts['impls'] += len(re.findall(r'\bunsafe\s+impl\b', line))
            counts['blocks'] += len(re.findall(r'\bunsafe\s*\{', line))
    return counts


def invariants(_args):
    """The pinned design figures, read from the source the guards also read."""
    bytecode = (ROOT / 'src' / 'bytecode.rs').read_text()
    version = re.search(r'pub const BYTECODE_VERSION: u16 = (\d+);', bytecode)
    slot = re.search(r'size_of::<Value>\(\) == (\d+)', bytecode)
    body = bytecode.split('pub enum Op {', 1)[1].split('\n}', 1)[0] if 'pub enum Op {' in bytecode else ''
    opcodes = sum(1 for l in body.splitlines()
                  if len(l) - len(l.lstrip()) == 4 and l.lstrip()[:1].isupper()
                  and any(c in l for c in '{(,='))
    if not version or not slot or opcodes < 50:
        raise Unavailable('the source no longer matches the patterns tests/claimed_counts.rs uses')
    return {'bytecode_version': int(version.group(1)), 'value_slot_bytes': int(slot.group(1)),
            'opcode_count': opcodes}


def footprint(args):
    """The runtime footprint, from the report scripts/footprint.sh overwrites."""
    path = ROOT / args.footprint
    if not path.exists():
        raise Unavailable(f'{args.footprint} is missing. Run scripts/footprint.sh first')
    report = json.loads(path.read_text())
    head = run(['git', 'rev-parse', 'HEAD']).strip()
    return {'commit': report['commit'], 'matches_head': report['commit'] == head,
            'clean_tree': report['clean_tree'], 'toolchain': report['toolchain'],
            'configurations': {c['name']: {'flash': c['runtime_flash'],
                                           'static_ram': c['runtime_static_ram']}
                               for c in report['configurations'] if c['name'] != 'baseline'}}


def po_counts(path):
    """Translated, untranslated and fuzzy messages of a gettext catalogue."""
    entries, current, fuzzy = [], None, False
    for line in path.read_text(errors='replace').splitlines() + ['']:
        if line.startswith('#,') and 'fuzzy' in line:
            fuzzy = True
        elif line.startswith('msgid '):
            current = {'id': line[6:].strip('"'), 'str': '', 'fuzzy': fuzzy, 'in': 'id'}
        elif line.startswith('msgstr ') and current is not None:
            current['str'], current['in'] = line[7:].strip('"'), 'str'
        elif line.startswith('"') and current is not None:
            current[current['in']] += line.strip('"')
        elif not line.strip() and current is not None:
            entries.append(current)
            current, fuzzy = None, False
    messages = [e for e in entries if e['id']]
    return {'messages': len(messages),
            'untranslated': sum(1 for e in messages if not e['str']),
            'fuzzy': sum(1 for e in messages if e['fuzzy'])}


def documentation(_args):
    """The book's translation state and the changelog fragments awaiting a release."""
    catalogues = [p for p in tracked('book/po') if p.suffix == '.po']
    return {'translations': {p.stem: po_counts(p) for p in catalogues},
            'pending_changelog_fragments': len([p for p in tracked('changelog.d')
                                                if p.name != 'README.md']),
            'markdown_files': len(tracked(':(glob)**/*.md'))}


def process(_args):
    """State-file budgets, the largest handoff, and the interline queue where it exists."""
    handoffs = {}
    for path in tracked('docs/process/handoffs'):
        text = path.read_text(errors='replace')
        budget = re.search(r'within a budget of (\d+)\s*KB', text)
        handoffs[path.name] = {'bytes': len(text.encode()), 'lines': text.count('\n'),
                               'budget_bytes': int(budget.group(1)) * 1000 if budget else None}
    largest = max(handoffs.items(), key=lambda kv: kv[1]['lines'])
    result = {'handoffs': handoffs, 'largest_handoff': {'file': largest[0], **largest[1]}}
    # The mailbox lives under the primary checkout, outside git, so CI has none.
    store = Path(run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir']).strip()
                 ).parent / 'secret' / 'interline'
    if (store / 'interline.py').exists():
        snapshot = ROOT / 'target' / 'metrics' / 'interline-snapshot.json'
        snapshot.parent.mkdir(parents=True, exist_ok=True)
        snapshot.unlink(missing_ok=True)
        run([sys.executable, str(store / 'interline.py'), '--store', str(store),
             'checkpoint', '--output', str(snapshot)])
        messages = json.loads(snapshot.read_text())['messages']
        snapshot.unlink()
        # The store also carries other projects' lines, which are not this project's queue.
        ours = [m for m in messages if m['recipient'] in KELEUSMA_LINES]
        result['interline_unacknowledged'] = sum(1 for m in ours if not m.get('acknowledged'))
    else:
        result['interline_unacknowledged'] = None
    return result


def dependencies(_args):
    """Distinct crates the runtime pulls in, for a no_std embedder and with defaults."""
    def count(*flags):
        out = run(['cargo', 'tree', '-p', 'keleusma', '--locked', '-e', 'normal',
                   '--prefix', 'none', *flags])
        names = {line.split()[0] for line in out.splitlines() if line.strip()}
        return len(names - {'keleusma'})
    return {'no_default_features': count('--no-default-features'), 'default_features': count()}


def proofs(_args):
    """Prose proof results and machine-checked Rocq results, with what is still admitted."""
    prose = {'theorems': 0, 'lemmas': 0, 'corollaries': 0}
    for path in tracked(':(glob)docs/proofs/*_PROOF.md'):
        text = path.read_text()
        for key, word in (('theorems', 'Theorem'), ('lemmas', 'Lemma'), ('corollaries', 'Corollary')):
            prose[key] += len(re.findall(rf'^\*\*{word}', text, re.M))
    rocq = {'files': 0, 'theorems': 0, 'admitted': 0}
    for path in tracked(':(glob)**/*.v'):
        text = re.sub(r'\(\*.*?\*\)', '', path.read_text(), flags=re.S)
        rocq['files'] += 1
        rocq['theorems'] += len(re.findall(r'^\s*(Theorem|Lemma|Corollary)\b', text, re.M))
        rocq['admitted'] += len(re.findall(r'\b(Admitted|admit)\b', text))
    return {'prose': prose, 'rocq': rocq}


def github(args):
    """Delivery figures over a window of days, from the GitHub interface."""
    if not args.github:
        raise Unavailable('not requested. Pass --github OWNER/REPO')
    if not shutil.which('gh'):
        raise Unavailable('the gh client is not installed')
    repo, branch = args.github, args.branch
    since = dt.datetime.now(dt.timezone.utc) - dt.timedelta(days=args.days)

    def api(path):
        return json.loads(run(['gh', 'api', '--paginate', '--slurp', path]))

    def when(text):
        return dt.datetime.fromisoformat(text.replace('Z', '+00:00'))

    pulls = [p for page in api(f'repos/{repo}/pulls?state=closed&base={branch}&per_page=100'
                                f'&sort=updated&direction=desc') for p in page
             if p.get('merged_at') and when(p['merged_at']) >= since]
    lead = [(when(p['merged_at']) - when(p['created_at'])).total_seconds() / 3600 for p in pulls]
    runs = [r for page in api(f'repos/{repo}/actions/workflows/ci.yml/runs?per_page=100'
                               f'&created=>={since.date().isoformat()}') for r in page['workflow_runs']]
    refs = {p['head']['ref'] for p in pulls}
    pr_runs = [r for r in runs if r['event'] == 'pull_request' and r['head_branch'] in refs]
    branch_runs = [r for r in runs if r['event'] == 'push' and r['head_branch'] == branch
                   and r['status'] == 'completed']
    minutes = [(when(r['updated_at']) - when(r['run_started_at'])).total_seconds() / 60
               for r in branch_runs if r.get('run_started_at')]
    fuzz = api(f'repos/{repo}/actions/workflows/fuzz.yml/runs?per_page=1')
    last_fuzz = fuzz[0]['workflow_runs'][0] if fuzz and fuzz[0]['workflow_runs'] else None
    result = {
        'window_days': args.days, 'branch': branch, 'merged_pull_requests': len(pulls),
        'lead_time_hours_median': round(statistics.median(lead), 2) if lead else None,
        'runs_per_merged_pull_request': round(len(pr_runs) / len(pulls), 2) if pulls else None,
        'branch_runs': len(branch_runs),
        'branch_runs_failed': sum(1 for r in branch_runs if r['conclusion'] == 'failure'),
        'branch_run_minutes_median': round(statistics.median(minutes), 1) if minutes else None,
        'fuzz_last_run': {'conclusion': last_fuzz['conclusion'], 'created': last_fuzz['created_at']}
        if last_fuzz else None}
    try:
        alerts = api(f'repos/{repo}/dependabot/alerts?state=open&per_page=100')
        result['open_dependabot_alerts'] = sum(len(page) for page in alerts)
    except Unavailable as error:
        result['open_dependabot_alerts'] = None
        result['open_dependabot_alerts_reason'] = str(error)[:200]
    return result


def tightness(args):
    """Memory-bound tightness over a corpus, from the report its runner writes."""
    path = ROOT / args.tightness
    if not path.exists():
        raise Unavailable(f'{args.tightness} is missing. Run the tightness runner first')
    return json.loads(path.read_text())


def machine(args):
    """Peak footprints the playbook helper recorded, per job label, from its local log."""
    path = ROOT / args.machine_log
    if not path.exists():
        raise Unavailable(f'{args.machine_log} is missing. scripts/machine-guard.sh run writes it')
    jobs = {}
    for line in path.read_text().splitlines():
        fields = line.split('\t')
        if len(fields) != 6:
            continue
        stamp, label, footprint, resident, seconds, status = fields
        entry = jobs.setdefault(label, {'runs': 0, 'peak_footprint': None, 'peak_resident': None})
        entry['runs'] += 1
        entry['last_run'] = stamp
        entry['last_status'] = int(status) if status.lstrip('-').isdigit() else status
        entry['last_seconds'] = int(seconds) if seconds.isdigit() else None
        for key, value in (('peak_footprint', footprint), ('peak_resident', resident)):
            if value.isdigit():
                entry[key] = max(entry[key] or 0, int(value))
    return {'jobs': jobs}


COLLECTORS = {'lines': lines, 'tests': tests, 'unsafe': unsafe_code, 'invariants': invariants,
              'footprint': footprint, 'documentation': documentation, 'process': process,
              'dependencies': dependencies, 'proofs': proofs, 'github': github,
              'tightness': tightness, 'machine': machine}


def collect(args):
    report = {'schema': 1,
              'commit': run(['git', 'rev-parse', 'HEAD']).strip(),
              'clean_tree': run(['git', 'status', '--porcelain']).strip() == '',
              'generated': dt.datetime.now(dt.timezone.utc).isoformat(timespec='seconds'),
              'metrics': {}}
    for name, collector in COLLECTORS.items():
        try:
            report['metrics'][name] = {'status': 'ok', 'values': collector(args)}
        except Unavailable as error:
            report['metrics'][name] = {'status': 'unavailable', 'reason': str(error)}
    return report


def summary(report):
    rows = [f'commit {report["commit"][:8]}, clean tree {report["clean_tree"]}, {report["generated"]}', '']
    for name, entry in report['metrics'].items():
        if entry['status'] != 'ok':
            rows.append(f'{name:14s} unavailable: {entry["reason"]}')
            continue
        values = entry['values']
        flat = {k: v for k, v in values.items() if isinstance(v, (int, float, str)) or v is None}
        rows.append(f'{name:14s} ' + (', '.join(f'{k} {v}' for k, v in flat.items())
                                      or f'{len(values)} groups, see the report'))
    return '\n'.join(rows)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    parser.add_argument('--output', type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument('--footprint', type=Path, default=Path('footprint/target/footprint.json'))
    parser.add_argument('--tightness', type=Path, default=Path('target/metrics/tightness.json'))
    parser.add_argument('--machine-log', type=Path, default=Path('target/machine-guard/peaks.tsv'))
    parser.add_argument('--github', metavar='OWNER/REPO')
    parser.add_argument('--branch', default=os.environ.get('KEL_TRUNK', 'v0.2.3'))
    parser.add_argument('--days', type=int, default=7)
    parser.add_argument('--require', default='',
                        help='comma-separated collectors that must succeed')
    args = parser.parse_args(argv)
    required = {name for name in args.require.split(',') if name}
    unknown = required - set(COLLECTORS)
    if unknown:
        parser.error(f'unknown collectors: {", ".join(sorted(unknown))}')
    report = collect(args)
    output = ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(summary(report))
    print(f'\nWritten to {args.output}')
    missing = sorted(n for n in required if report['metrics'][n]['status'] != 'ok')
    if missing:
        print(f'metrics: required collectors unavailable: {", ".join(missing)}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
