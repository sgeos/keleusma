"""Tests of the build-output handling of `scripts/worktree.sh` with a scratch volume.

The operator approved the change on 2026-10-31, in
docs/process/kaizen/2026-10-31-scratch-volume-proposal.md, after a removed worktree left
1.7 GB of build output on the scratch volume that nothing pointed to. Each test builds a
throwaway repository holding copies of `worktree.sh` and `scratch.sh`, with a temporary
directory standing in for the scratch volume, so nothing here touches a real tree or
volume. The failure cases are the point. A new tree must not attach itself to another
tree's output, and a removal must delete its own folder and nothing else.
"""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class WorktreeScratchTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.repo = self.tmp / 'repo'
        self.trees = self.tmp / 'trees'
        self.scratch = self.tmp / 'scratch'
        self.scratch.mkdir()
        (self.repo / 'scripts').mkdir(parents=True)
        for name in ('worktree.sh', 'scratch.sh'):
            shutil.copy2(ROOT / 'scripts' / name, self.repo / 'scripts' / name)
        # The root and one nested package exist. `fuzz` does not, so its target must
        # not be linked, and no `fuzz/` directory may appear.
        (self.repo / 'Cargo.toml').write_text('[package]\n')
        (self.repo / 'compiler').mkdir()
        (self.repo / 'compiler' / 'Cargo.toml').write_text('[package]\n')
        (self.repo / '.gitignore').write_text('**/target\n')
        self.git('init', '-q', '-b', 'main')
        self.git('add', '-A')
        self.git('commit', '-q', '-m', 'initial')

    def env(self, scratch=True):
        env = dict(os.environ, KEL_TRUNK='main', KEL_WORKTREES_DIR=str(self.trees),
                   GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid',
                   GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid')
        env.pop('SCRATCH_BASE', None)
        env.pop('SCRATCH_DIRS', None)
        if scratch:
            env['SCRATCH_BASE'] = str(self.scratch)
        return env

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.repo), *args], check=True,
                              capture_output=True, text=True, env=self.env(False))

    def worktree(self, *args, scratch=True):
        return subprocess.run([str(self.repo / 'scripts' / 'worktree.sh'), *args],
                              cwd=self.repo, capture_output=True, text=True,
                              env=self.env(scratch))

    def home(self, name):
        return self.scratch / 'repo' / name

    def test_new_links_each_existing_package_target(self):
        result = self.worktree('new', 'feat/one')
        self.assertEqual(result.returncode, 0, result.stderr)
        tree = self.trees / 'one'
        for target in ('target', 'compiler/target'):
            with self.subTest(target=target):
                self.assertTrue((tree / target).is_symlink())
                self.assertEqual(Path(os.readlink(tree / target)), self.home('one') / target)
        self.assertFalse((tree / 'fuzz').exists())
        status = subprocess.run(['git', '-C', str(tree), 'status', '--porcelain'],
                                capture_output=True, text=True, check=True).stdout
        self.assertEqual(status, '')

    def test_without_scratch_base_nothing_is_linked(self):
        result = self.worktree('new', 'feat/plain', scratch=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.trees / 'plain' / 'target').exists())
        self.assertEqual(list(self.scratch.iterdir()), [])

    def test_new_refuses_a_name_whose_scratch_folder_exists(self):
        stale = self.home('two') / 'target'
        stale.mkdir(parents=True)
        (stale / 'marker').write_text('another tree')
        result = self.worktree('new', 'feat/two')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('already exists', result.stderr)
        self.assertFalse((self.trees / 'two').exists())
        self.assertEqual((stale / 'marker').read_text(), 'another tree')

    def test_new_refuses_a_name_another_tree_uses(self):
        # A tree created outside this script, under another parent, has the same leaf.
        self.git('worktree', 'add', '-q', '-b', 'other/three', str(self.tmp / 'elsewhere' / 'three'))
        result = self.worktree('new', 'feat/three')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('already uses that scratch folder name', result.stderr)
        self.assertFalse((self.trees / 'three').exists())

    def test_rm_deletes_only_its_own_folder(self):
        for branch in ('feat/four', 'feat/five'):
            self.assertEqual(self.worktree('new', branch).returncode, 0)
        result = self.worktree('rm', 'feat/four')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.home('four').exists())
        self.assertTrue(self.home('five').is_dir())
        self.assertTrue((self.trees / 'five' / 'target').is_symlink())

    def test_rm_of_a_dirty_tree_deletes_nothing(self):
        self.assertEqual(self.worktree('new', 'feat/six').returncode, 0)
        (self.trees / 'six' / 'work.txt').write_text('uncommitted')
        result = self.worktree('rm', 'feat/six')
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(self.home('six').is_dir())
        self.assertTrue((self.trees / 'six' / 'work.txt').exists())

    def test_rm_of_a_vanished_tree_deletes_its_folder(self):
        # The path that left the orphan of 2026-10-30. The tree directory went away
        # without the script, and the build output stayed.
        self.assertEqual(self.worktree('new', 'feat/seven').returncode, 0)
        shutil.rmtree(self.trees / 'seven')
        result = self.worktree('rm', 'feat/seven')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.home('seven').exists())

    def test_rm_keeps_a_folder_a_live_tree_of_the_same_leaf_uses(self):
        # A tree made outside this script, as the lines' trees under .worktrees/ are,
        # owns the scratch folder of its leaf. Removing a missing tree of that leaf from
        # the script's own directory must not delete it.
        live = self.tmp / 'elsewhere' / 'ten'
        self.git('worktree', 'add', '-q', '-b', 'line/ten', str(live))
        owned = self.home('ten') / 'target'
        owned.mkdir(parents=True)
        result = self.worktree('rm', 'feat/ten')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('still uses', result.stdout)
        self.assertTrue(owned.is_dir())

    def test_list_reports_an_orphan_and_deletes_nothing(self):
        orphan = self.home('eight')
        orphan.mkdir(parents=True)
        self.assertEqual(self.worktree('new', 'feat/nine').returncode, 0)
        result = self.worktree('list')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(f'scratch folder used by no tree: {orphan}', result.stdout)
        self.assertNotIn(str(self.home('nine')), result.stdout)
        self.assertTrue(orphan.is_dir())


if __name__ == '__main__':
    unittest.main()
