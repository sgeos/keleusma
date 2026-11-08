"""Tests of `scripts/footprint.sh`, the runtime footprint measurement.

The real measurement builds a bare-metal image per configuration, which needs the pinned
toolchain and its target, so these tests replace cargo, rustc and llvm-size with
stand-ins on PATH. The stand-in llvm-size reports sizes chosen per feature set, which
lets each test fix the figures and check what the script makes of them: the differences
from the baseline, the JSON it overwrites, and the ceilings that --check enforces. The
CI footprint job runs the real build. The machine guard is switched off, so that no test
depends on the live machine's level.
"""

from pathlib import Path
import json
import os
import shutil
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

FAKE_CARGO = """#!/bin/sh
# Record the requested features where the stand-in llvm-size will find them.
features=''
while [ "$#" -gt 0 ]; do
  [ "$1" = --features ] && features=$2
  shift
done
[ -n "${FAIL_FEATURES:-}" ] && [ "$features" = "$FAIL_FEATURES" ] && exit 101
mkdir -p target/thumbv8m.main-none-eabihf/release
printf '%s\\n' "$features" > target/thumbv8m.main-none-eabihf/release/keleusma-footprint
"""

FAKE_RUSTC = """#!/bin/sh
case $1 in
  -vV) printf 'rustc 1.92.0\\nhost: test-host\\n' ;;
  -V) echo 'rustc 1.92.0 (test)' ;;
  --print) echo "$FAKE_SYSROOT" ;;
esac
"""

# Text, data and bss per feature set. The baseline is the empty feature set.
SIZES = {'': (4000, 0, 65536), 'probe': (104000, 8, 65536), 'verify': (154000, 8, 65600),
         'verify,floats': (164000, 8, 65600), 'verify,floats,text': (165000, 8, 65600),
         'verify,text': (160000, 8, 65600)}

FAKE_SIZE = """#!/bin/sh
features=$(cat "$2")
echo '   text    data     bss     dec     hex filename'
case $features in
%s
esac
"""


# Ceilings just above the stand-in figures, so a default run passes.
CEILINGS = {'vm-trusted': (100008, 100), 'vm-verify': (150008, 100),
            'vm-verify-floats': (160008, 100), 'vm-verify-floats-text': (161008, 100)}


def size_cases():
    return '\n'.join(f"  '{k}') echo '{t} {d} {b} 0 0 image' ;;" for k, (t, d, b) in SIZES.items())


class FootprintTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.repo = self.tmp / 'repo'
        (self.repo / 'scripts').mkdir(parents=True)
        (self.repo / 'footprint').mkdir()
        for name in ('footprint.sh', 'machine-guard.sh'):
            shutil.copy2(ROOT / 'scripts' / name, self.repo / 'scripts' / name)
        self.write_budget(CEILINGS)
        for args in (['init', '-q', '-b', 'main'], ['add', '-A'], ['commit', '-q', '-m', 'initial']):
            subprocess.run(['git', '-C', str(self.repo), *args], check=True, capture_output=True,
                           env=dict(os.environ, GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid',
                                    GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid'))
        self.bin = self.tmp / 'bin'
        self.sysroot = self.tmp / 'sysroot'
        tool_dir = self.sysroot / 'lib' / 'rustlib' / 'test-host' / 'bin'
        for path, text in ((self.bin / 'cargo', FAKE_CARGO), (self.bin / 'rustc', FAKE_RUSTC),
                           (tool_dir / 'llvm-size', FAKE_SIZE % size_cases())):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
            path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def write_budget(self, ceilings):
        lines = ['# configuration flash ram'] + [f'{k} {f} {r}' for k, (f, r) in ceilings.items()]
        (self.repo / 'footprint' / 'budget.tsv').write_text('\n'.join(lines) + '\n')

    def run_footprint(self, *args, **extra):
        env = dict(os.environ, PATH=f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
                   FAKE_SYSROOT=str(self.sysroot), KEL_MACHINE_GUARD='off', **extra)
        return subprocess.run([shutil.which('sh'), str(self.repo / 'scripts' / 'footprint.sh'), *args],
                              capture_output=True, text=True, env=env)

    def report(self):
        return json.loads((self.repo / 'footprint' / 'target' / 'footprint.json').read_text())

    def test_figures_are_differences_from_the_baseline(self):
        result = self.run_footprint()
        self.assertEqual(result.returncode, 0, result.stderr)
        configs = {c['name']: c for c in self.report()['configurations']}
        self.assertEqual(configs['baseline']['runtime_flash'], 0)
        self.assertEqual(configs['vm-trusted']['runtime_flash'], 100008)
        self.assertEqual(configs['vm-trusted']['runtime_static_ram'], 8)
        self.assertEqual(configs['vm-verify']['runtime_flash'], 150008)
        self.assertEqual(configs['vm-verify']['runtime_static_ram'], 72)
        self.assertEqual(configs['vm-verify-floats-text']['features'], 'verify,floats,text')

    def test_the_report_names_its_build(self):
        self.assertEqual(self.run_footprint().returncode, 0)
        report = self.report()
        head = subprocess.run(['git', '-C', str(self.repo), 'rev-parse', 'HEAD'],
                              capture_output=True, text=True, check=True).stdout.strip()
        self.assertEqual(report['commit'], head)
        self.assertEqual(report['toolchain'], 'rustc 1.92.0 (test)')
        self.assertIs(report['clean_tree'], True)

    def test_each_run_overwrites_the_report(self):
        self.assertEqual(self.run_footprint().returncode, 0)
        (self.repo / 'scripts' / 'extra.txt').write_text('dirty\n')
        self.assertEqual(self.run_footprint().returncode, 0)
        self.assertIs(self.report()['clean_tree'], False)

    def test_check_passes_within_every_ceiling(self):
        result = self.run_footprint('--check')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('within its ceiling', result.stdout)

    def test_check_fails_over_a_flash_ceiling_and_names_it(self):
        self.write_budget(dict(CEILINGS, **{'vm-trusted': (100007, 100)}))
        result = self.run_footprint('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('vm-trusted flash 100008 exceeds its ceiling 100007', result.stderr)

    def test_check_fails_over_a_static_ram_ceiling(self):
        self.write_budget(dict(CEILINGS, **{'vm-verify': (150008, 71)}))
        result = self.run_footprint('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('vm-verify static RAM 72 exceeds its ceiling 71', result.stderr)

    def test_check_fails_when_a_configuration_has_no_ceiling(self):
        self.write_budget({'vm-trusted': CEILINGS['vm-trusted']})
        result = self.run_footprint('--check')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('vm-verify has no ceiling', result.stderr)

    def test_a_custom_configuration_is_measured_but_not_checked(self):
        result = self.run_footprint('--check', '--features', 'verify,text')
        self.assertEqual(result.returncode, 0, result.stderr)
        configs = {c['name']: c for c in self.report()['configurations']}
        self.assertEqual(configs['custom']['runtime_flash'], 156008)

    def test_custom_features_with_shell_characters_are_refused(self):
        result = self.run_footprint('--features', 'verify;rm')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('features may hold only', result.stderr)

    def test_a_failed_build_fails_the_run(self):
        result = self.run_footprint(FAIL_FEATURES='verify')
        self.assertNotEqual(result.returncode, 0)

    @unittest.skipUnless(shutil.which('dash'), 'dash is not installed')
    def test_it_runs_under_dash(self):
        env = dict(os.environ, PATH=f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
                   FAKE_SYSROOT=str(self.sysroot), KEL_MACHINE_GUARD='off')
        result = subprocess.run([shutil.which('dash'), str(self.repo / 'scripts' / 'footprint.sh'), '--check'],
                                capture_output=True, text=True, env=env)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
