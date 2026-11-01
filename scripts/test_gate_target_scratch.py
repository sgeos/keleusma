"""Tests of where `scripts/gate-in-worktree.sh` puts its build output.

The operator decided on 2026-10-31 that the gate's default target directory moves to
the scratch volume when `SCRATCH_BASE` is set, after four gate directories of 19.4 GB
were found unused on the system disk. Each test runs the script's `--setup-only` path
in a throwaway repository, which prepares the gate worktree and prints the target
without building anything.
"""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class GateTargetTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.repo = self.tmp / 'repo'
        self.trees = self.tmp / 'trees'
        (self.repo / 'scripts').mkdir(parents=True)
        shutil.copy2(ROOT / 'scripts' / 'gate-in-worktree.sh', self.repo / 'scripts')
        env = self.env({})
        for args in (['init', '-q', '-b', 'main'], ['add', '-A'], ['commit', '-q', '-m', 'initial']):
            subprocess.run(['git', '-C', str(self.repo), *args], check=True,
                           capture_output=True, env=env)

    def env(self, extra):
        env = dict(os.environ, KEL_WORKTREES_DIR=str(self.trees),
                   GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid',
                   GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid')
        for name in ('SCRATCH_BASE', 'KEL_GATE_TARGET', 'KEL_GATE_NAME', 'KEL_GATE_LOG'):
            env.pop(name, None)
        env.update(extra)
        return env

    def target(self, **extra):
        result = subprocess.run([str(self.repo / 'scripts' / 'gate-in-worktree.sh'), '--setup-only'],
                                cwd=self.repo, capture_output=True, text=True, env=self.env(extra))
        lines = [l for l in result.stdout.splitlines() if l.startswith('gate-worktree: target')]
        return result, (lines[0].split(None, 2)[2] if lines else None)

    def test_without_scratch_base_the_default_stays_beside_the_trees(self):
        result, target = self.target()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(target), self.trees.resolve() / '.gate-target')

    def test_scratch_base_moves_the_default_to_the_repository_folder(self):
        scratch = self.tmp / 'scratch'
        scratch.mkdir()
        result, target = self.target(SCRATCH_BASE=str(scratch))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(target), scratch / 'repo' / '.gate-target')

    def test_an_explicit_target_wins_over_scratch_base(self):
        scratch = self.tmp / 'scratch'
        scratch.mkdir()
        chosen = self.tmp / 'chosen'
        result, target = self.target(SCRATCH_BASE=str(scratch), KEL_GATE_TARGET=str(chosen))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(target), chosen)

    def test_an_unusable_scratch_base_refuses_rather_than_falling_back(self):
        result, target = self.target(SCRATCH_BASE=str(self.tmp / 'missing'))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('not a writable directory', result.stderr)
        self.assertIsNone(target)


if __name__ == '__main__':
    unittest.main()
