"""Tests of `scripts/machine-guard.sh`, the playbook helper for heavy Keleusma jobs.

A stand-in `machine-status` on PATH answers from a file of levels, one per call, so each
test fixes the sequence the machine goes through and checks what the helper does: start
at ok, wait at elevated, critical and stale, start where no monitor exists, honour the
operator's off switch, give up after its limit, and record a run with its exit status.
Every wait polls with a zero interval, so no test sleeps.
"""

from pathlib import Path
import os
import shutil
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
GUARD = ROOT / 'scripts' / 'machine-guard.sh'

# Answers with the first line of the levels file and drops it, keeping the last line
# once the file is down to one, so a sequence ends on a steady level.
FAKE_STATUS = """#!/bin/sh
levels=$MACHINE_LEVELS
level=$(head -n 1 "$levels")
if [ "$(wc -l < "$levels")" -gt 1 ]; then
  tail -n +2 "$levels" > "$levels.next" && mv "$levels.next" "$levels"
fi
echo "$level" >> "$levels.calls"
echo "$level"
case $level in ok) exit 0 ;; elevated) exit 1 ;; critical) exit 2 ;; *) exit 3 ;; esac
"""


class MachineGuardTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.bin = self.tmp / 'bin'
        self.bin.mkdir()
        status = self.bin / 'machine-status'
        status.write_text(FAKE_STATUS)
        status.chmod(status.stat().st_mode | stat.S_IXUSR)
        self.levels = self.tmp / 'levels'
        self.log = self.tmp / 'peaks.tsv'

    def guard(self, *args, levels=('ok',), monitor=True, **extra):
        self.levels.write_text('\n'.join(levels) + '\n')
        path = f'{self.bin}{os.pathsep}{os.environ["PATH"]}' if monitor else '/usr/bin:/bin'
        env = dict(os.environ, PATH=path, MACHINE_LEVELS=str(self.levels),
                   KEL_MACHINE_GUARD_POLL='0', KEL_MACHINE_GUARD_LOG=str(self.log), **extra)
        env.pop('KEL_MACHINE_GUARD', None) if 'KEL_MACHINE_GUARD' not in extra else None
        return subprocess.run([shutil.which('sh'), str(GUARD), *args],
                              capture_output=True, text=True, env=env, cwd=self.tmp)

    def calls(self):
        path = Path(f'{self.levels}.calls')
        return path.read_text().split() if path.exists() else []

    def test_level_reports_each_monitor_answer(self):
        for answer in ('ok', 'elevated', 'critical', 'stale'):
            with self.subTest(answer=answer):
                self.assertEqual(self.guard('level', levels=(answer,)).stdout.strip(), answer)

    def test_an_unreadable_answer_is_stale_and_a_missing_monitor_is_unmonitored(self):
        self.assertEqual(self.guard('level', levels=('garbled',)).stdout.strip(), 'stale')
        self.assertEqual(self.guard('level', monitor=False).stdout.strip(), 'unmonitored')

    def test_wait_returns_at_once_when_ok(self):
        result = self.guard('wait', 'job', levels=('ok',))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), ['ok'])
        self.assertEqual(result.stderr, '')

    def test_wait_holds_through_elevated_critical_and_stale_until_ok(self):
        result = self.guard('wait', 'job', levels=('elevated', 'critical', 'stale', 'ok'))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), ['elevated', 'critical', 'stale', 'ok'])
        self.assertIn('level critical, waiting before job', result.stderr)
        self.assertIn('starting job', result.stderr)

    def test_wait_proceeds_where_no_monitor_exists(self):
        result = self.guard('wait', 'job', monitor=False)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_the_operator_can_switch_the_wait_off(self):
        result = self.guard('wait', 'job', levels=('critical',), KEL_MACHINE_GUARD='off')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), [])

    def test_wait_gives_up_after_its_limit_and_says_so(self):
        result = self.guard('wait', 'job', levels=('elevated',), KEL_MACHINE_GUARD_MAX='0')
        self.assertEqual(result.returncode, 75)
        self.assertIn('level still elevated after 0s, not starting job', result.stderr)

    def test_run_records_the_command_and_returns_its_status(self):
        ok = self.guard('run', 'passes', 'sh', '-c', 'exit 0')
        failed = self.guard('run', 'fails', 'sh', '-c', 'exit 3')
        self.assertEqual(ok.returncode, 0, ok.stderr)
        self.assertEqual(failed.returncode, 3)
        rows = [line.split('\t') for line in self.log.read_text().splitlines()]
        self.assertEqual([(r[1], r[5]) for r in rows], [('passes', '0'), ('fails', '3')])
        for row in rows:
            self.assertEqual(len(row), 6)
            for peak in row[2:4]:
                self.assertTrue(peak == 'unknown' or peak.isdigit(), row)

    def test_run_does_not_start_while_the_machine_is_critical_past_the_limit(self):
        marker = self.tmp / 'ran'
        result = self.guard('run', 'job', 'touch', str(marker), levels=('critical',),
                            KEL_MACHINE_GUARD_MAX='0')
        self.assertEqual(result.returncode, 75)
        self.assertFalse(marker.exists())

    def test_sourcing_defines_the_functions_without_running_anything(self):
        script = f'. "{GUARD}"; machine_guard_wait sourced && echo done'
        self.levels.write_text('ok\n')
        env = dict(os.environ, PATH=f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
                   MACHINE_LEVELS=str(self.levels), KEL_MACHINE_GUARD_POLL='0')
        result = subprocess.run([shutil.which('sh'), '-c', script], capture_output=True,
                                text=True, env=env)
        self.assertEqual(result.stdout.strip(), 'done', result.stderr)

    @unittest.skipUnless(shutil.which('dash'), 'dash is not installed')
    def test_it_runs_under_dash(self):
        self.levels.write_text('elevated\nok\n')
        env = dict(os.environ, PATH=f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
                   MACHINE_LEVELS=str(self.levels), KEL_MACHINE_GUARD_POLL='0')
        result = subprocess.run([shutil.which('dash'), str(GUARD), 'wait', 'job'],
                                capture_output=True, text=True, env=env)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
