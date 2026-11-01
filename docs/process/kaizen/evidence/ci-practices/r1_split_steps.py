"""Split each cargo step of a CI job log into compile and test time.

Every log line starts with an ISO timestamp. A step starts at a `##[group]Run` line.
Within a step, `Compiling <crate> v<ver>` lines are dependencies unless they carry a
local path in parentheses, which marks a workspace or path crate. Cargo's
`Finished ... in <dur>` gives the compile time it measured. nextest's
`Summary [<secs>s]` gives the test-run time it measured.
"""
import re, sys, csv, statistics as st
from datetime import datetime
TS = re.compile(r'^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d+)Z (.*)$')
ESC = re.compile(r'\x1b\[[0-9;]*m')
DUR = re.compile(r'Finished `[^`]+` profile .* in (?:(\d+)m )?([\d.]+)s')
SUM = re.compile(r'Summary \[\s*([\d.]+)s\] (\d+) tests? run')
COMP = re.compile(r'^\s*Compiling (\S+) v\S+( \(/.*\))?')
def ts(s): return datetime.fromisoformat(s[:26])
def steps(path):
    cur = None
    for raw in open(path, encoding='utf-8', errors='replace'):
        m = TS.match(raw.rstrip('\n'))
        if not m: continue
        t, text = ts(m.group(1)), ESC.sub('', m.group(2))
        if text.startswith('##[group]Run '):
            if cur: yield cur
            cur = dict(name=text[13:][:70], start=t, end=t, compile=0.0, test=0.0, tests=0,
                       first_dep=None, last_dep=None, first_local=None, deps=0, locals=0)
            continue
        if cur is None: continue
        cur['end'] = t
        c = COMP.match(text)
        if c:
            if c.group(2):
                cur['locals'] += 1; cur['first_local'] = cur['first_local'] or t
            else:
                cur['deps'] += 1; cur['first_dep'] = cur['first_dep'] or t; cur['last_dep'] = t
        d = DUR.search(text)
        if d: cur['compile'] += int(d.group(1) or 0) * 60 + float(d.group(2))
        s = SUM.search(text)
        if s: cur['test'] += float(s.group(1)); cur['tests'] += int(s.group(2))
    if cur: yield cur
names = {r[1]: r[2] for r in csv.reader(open(sys.argv[1]), delimiter='\t')}
out = csv.writer(sys.stdout, delimiter='\t', lineterminator='\n')
out.writerow(['job_id', 'job', 'step', 'wall_s', 'compile_s', 'test_s', 'tests', 'deps', 'locals', 'last_dep_s'])
for job, jname in names.items():
    for s in steps(f'{sys.argv[2]}/{job}.log'):
        if not (s['compile'] or s['test'] or s['deps']): continue
        last_dep = (s['last_dep'] - s['start']).total_seconds() if s['last_dep'] else 0
        out.writerow([job, re.sub(r' shard \d/4', ' shard', jname), s['name'],
                      round((s['end'] - s['start']).total_seconds()), round(s['compile']),
                      round(s['test']), s['tests'], s['deps'], s['locals'], round(last_dep)])
