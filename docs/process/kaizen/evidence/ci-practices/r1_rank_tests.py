"""Total and rank nextest test durations per job class across downloaded logs."""
import re, sys, csv, statistics as st
ESC = re.compile(r'\x1b\[[0-9;]*m')
PASS = re.compile(r'\s(PASS|FLAKY \d/\d|TRY \d PASS|SLOW|FAIL|TRY \d FAIL) \[\s*([\d.]+)s\]\s+(?:\(\s*\d+/\d+\)\s+)?(\S+) (\S+)$')
names = {r[1]: re.sub(r' shard \d/4', ' shard', r[2]) for r in csv.reader(open(sys.argv[1]), delimiter='\t')}
per = {}
retries = []
for job, jname in names.items():
    for raw in open(f'{sys.argv[2]}/{job}.log', encoding='utf-8', errors='replace'):
        line = ESC.sub('', raw.rstrip('\n'))
        m = PASS.search(line)
        if not m: continue
        kind, secs, binary, test = m.groups()
        if kind.startswith(('FLAKY', 'TRY')): retries.append((jname, kind, binary, test))
        if kind in ('SLOW',): continue
        per.setdefault(jname, {}).setdefault((binary, test), []).append(float(secs))
for jname, tests in sorted(per.items()):
    runs = max(len(v) for v in tests.values())
    total = sum(st.median(v) for v in tests.values())
    bybin = {}
    for (b, t), v in tests.items(): bybin[b] = bybin.get(b, 0) + st.median(v)
    print(f'\n== {jname}: {len(tests)} tests, median test-seconds summed {total/60:.1f} min, up to {runs} samples each')
    for b, s in sorted(bybin.items(), key=lambda kv: -kv[1])[:8]:
        print(f'   bin {s/60:6.1f} min  {s/total*100:4.1f}%  {b}')
    for (b, t), v in sorted(tests.items(), key=lambda kv: -st.median(kv[1]))[:6]:
        print(f'   test {st.median(v):7.1f}s  {b} {t}')
print('\nretries:', len(retries))
for r in retries[:20]: print('  ', *r)
