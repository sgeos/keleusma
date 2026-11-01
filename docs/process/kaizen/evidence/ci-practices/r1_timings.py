"""Summarise a cargo `--timings` HTML report into dependency, library and test-target time.

The report embeds `const UNIT_DATA = [...]`, one entry per compilation unit with its
start and duration in seconds. A unit is a workspace unit when its crate name is a
workspace member. Unit seconds overlap in parallel, so their sum is CPU-like time, and
the report's last unit end is the wall time of the build.
"""
import json, re, sys
WORKSPACE = {'keleusma', 'keleusma-macros', 'keleusma-arena', 'keleusma-wire',
             'keleusma-wire-derive', 'keleusma-bench', 'keleusma-cli'}
for path in sys.argv[1:]:
    html = open(path, encoding='utf-8').read()
    units = json.loads(re.search(r'const UNIT_DATA = (\[.*?\]);\n', html, re.S).group(1))
    wall = max(u['start'] + u['duration'] for u in units)
    groups = {}
    for u in units:
        local = u['name'] in WORKSPACE
        target = u.get('target', '')
        if not local: g = 'dependencies'
        elif 'test' in target and u['name'] == 'keleusma' and 'lib' not in target: g = 'keleusma test binaries'
        elif 'lib (test)' in target or (local and 'test' in target): g = f'{u["name"]} unit-test or other test targets'
        elif u['mode'] == 'run-custom-build' or 'build script' in target: g = 'build scripts'
        else: g = f'{u["name"]} library and binaries'
        n, s, last = groups.get(g, (0, 0.0, 0.0))
        groups[g] = (n + 1, s + u['duration'], max(last, u['start'] + u['duration']))
    total = sum(s for _, s, _ in groups.values())
    print(f'\n{path}: {len(units)} units, wall {wall/60:.1f} min, unit-seconds summed {total/60:.1f} min')
    for g, (n, s, last) in sorted(groups.items(), key=lambda kv: -kv[1][1]):
        print(f'  {s/60:6.1f} min {s/total*100:5.1f}%  {n:4d} units, done by {last/60:5.1f} min  {g}')
    tests = sorted((u for u in units if u['name'] == 'keleusma' and 'test' in u.get('target', '')),
                   key=lambda u: -u['duration'])
    if tests:
        d = [u['duration'] for u in tests]
        print(f'  keleusma test targets: {len(d)}, median {sorted(d)[len(d)//2]:.1f}s, slowest',
              ', '.join(f"{u['target'].strip()} {u['duration']:.0f}s" for u in tests[:4]))
