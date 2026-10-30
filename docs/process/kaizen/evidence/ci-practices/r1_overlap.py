"""Tests that run in both the Test job and the self-host shards, with their summed median time."""
import re, sys, csv, statistics as st
ESC = re.compile(r'\x1b\[[0-9;]*m')
PASS = re.compile(r'\s(PASS|FLAKY \d/\d) \[\s*([\d.]+)s\]\s+(?:\(\s*\d+/\d+\)\s+)?(\S+) (\S+)$')
names = {r[1]: re.sub(r' shard \d/4', ' shard', r[2]) for r in csv.reader(open(sys.argv[1]), delimiter='\t')}
per = {}
for job, jname in names.items():
    for raw in open(f'{sys.argv[2]}/{job}.log', encoding='utf-8', errors='replace'):
        m = PASS.search(ESC.sub('', raw.rstrip('\n')))
        if m: per.setdefault(jname, {}).setdefault((m.group(3), m.group(4)), []).append(float(m.group(2)))
a, b = per['Test'], per['Test (self-host feature, shard)']
both = set(a) & set(b)
ta = sum(st.median(a[k]) for k in both); tb = sum(st.median(b[k]) for k in both)
print(f'Test job tests {len(a)}, shard tests {len(b)}, in both {len(both)}')
print(f'time of the shared tests: {ta/60:.1f} test-min in Test, {tb/60:.1f} in the shards')
bybin = {}
for k in both: bybin[k[0]] = bybin.get(k[0], 0) + st.median(a[k])
for bn, s in sorted(bybin.items(), key=lambda kv: -kv[1])[:10]: print(f'  {s/60:5.1f} min in Test  {bn}')
only = {k: v for k, v in a.items() if k not in both}
print(f'Test-only tests {len(only)}, {sum(st.median(v) for v in only.values())/60:.1f} test-min')
ob = {}
for k, v in only.items(): ob[k[0]] = ob.get(k[0], 0) + st.median(v)
for bn, s in sorted(ob.items(), key=lambda kv: -kv[1])[:6]: print(f'  {s/60:5.1f} min Test only  {bn}')
