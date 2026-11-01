"""Tests of scripts/ci-certified.sh against stub `gh` and `rustc` commands.

The stubs replace the network and the toolchain, so these tests check the script's
control flow and its fail-closed paths. The `--jq` filters themselves were checked
against real run and artifact records of this repository on 2026-10-25, because a stub
cannot evaluate them.
"""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parent / 'ci-certified.sh'
REPO = 'example/repo'

GH = r'''#!/usr/bin/env python3
import os, sys
args = sys.argv[1:]
if os.environ.get('FAKE_GH_FAIL'):
    sys.exit(1)
if args[:1] == ['api'] and 'actions/artifacts?' in args[1]:
    print('\n'.join(os.environ.get('FAKE_RUNS', '').split()))
elif args[:1] == ['api'] and '/actions/runs/' in args[1]:
    run = args[1].rsplit('/', 1)[1]
    if run in os.environ.get('FAKE_ACCEPT', '').split():
        print(run)
elif args[:2] == ['run', 'download']:
    run, target = args[2], args[args.index('--dir') + 1]
    body = os.environ.get('FAKE_CERT_' + run)
    if body is None:
        sys.exit(1)
    with open(os.path.join(target, 'certificate.txt'), 'w') as stream:
        stream.write(body)
else:
    sys.exit(2)
'''

RUSTC = '#!/bin/sh\necho "rustc 1.99.0 (stub)"\n'


class CertifiedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory(prefix='ci-certified-tests-')
        root = Path(cls.directory.name).resolve()
        cls.repo = root / 'repo'
        cls.repo.mkdir()
        subprocess.run(['git', 'init', '-q', str(cls.repo)], check=True)
        (cls.repo / 'file.txt').write_text('content\n')
        subprocess.run(['git', '-C', str(cls.repo), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(cls.repo), '-c', 'user.name=T', '-c', 'user.email=t@example.invalid',
                        'commit', '-qm', 'fixture'], check=True)
        cls.tree = subprocess.run(['git', '-C', str(cls.repo), 'rev-parse', 'HEAD^{tree}'], check=True,
                                  capture_output=True, text=True).stdout.strip()
        cls.bin = root / 'bin'
        cls.bin.mkdir()
        # The script's own scratch space, inside the fixture, so no sandbox rule on the
        # system temporary directory applies.
        cls.scratch = root / 'scratch'
        cls.scratch.mkdir()
        for name, body in (('gh', GH), ('rustc', RUSTC)):
            path = cls.bin / name
            path.write_text(body)
            path.chmod(0o755)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def certificate(self, tree=None, toolchain='rustc 1.99.0 (stub)'):
        return f'tree={tree or self.tree}\ntoolchain={toolchain}\ncommit=abc\nrun=1\n'

    def run_script(self, **fake):
        env = {'PATH': f'{self.bin}{os.pathsep}{os.environ["PATH"]}', 'GITHUB_REPOSITORY': REPO,
               'HOME': os.environ.get('HOME', '/'), 'TMPDIR': str(self.scratch)}
        env.update(fake)
        done = subprocess.run(['bash', str(SCRIPT)], cwd=self.repo, env=env, capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        return done.stdout

    def test_a_matching_certificate_certifies(self):
        out = self.run_script(FAKE_RUNS='7', FAKE_ACCEPT='7', FAKE_CERT_7=self.certificate())
        self.assertEqual(out, 'mode=certified\ncertified_by=7\n')

    def test_the_first_matching_run_is_reported(self):
        out = self.run_script(FAKE_RUNS='5 6', FAKE_ACCEPT='5 6',
                              FAKE_CERT_5=self.certificate(toolchain='rustc 1.98.0'),
                              FAKE_CERT_6=self.certificate())
        self.assertEqual(out, 'mode=certified\ncertified_by=6\n')

    def test_every_mismatch_or_failure_leaves_the_run_full(self):
        cases = {
            'no artifact': {'FAKE_RUNS': ''},
            'run not accepted': {'FAKE_RUNS': '7', 'FAKE_ACCEPT': '', 'FAKE_CERT_7': self.certificate()},
            'other toolchain': {'FAKE_RUNS': '7', 'FAKE_ACCEPT': '7',
                                'FAKE_CERT_7': self.certificate(toolchain='rustc 1.98.0 (old)')},
            'other tree': {'FAKE_RUNS': '7', 'FAKE_ACCEPT': '7', 'FAKE_CERT_7': self.certificate(tree='0' * 40)},
            'download fails': {'FAKE_RUNS': '7', 'FAKE_ACCEPT': '7'},
            'gh fails': {'FAKE_GH_FAIL': '1', 'FAKE_RUNS': '7', 'FAKE_ACCEPT': '7', 'FAKE_CERT_7': self.certificate()},
            'no repository': {'GITHUB_REPOSITORY': '', 'FAKE_RUNS': '7', 'FAKE_ACCEPT': '7',
                              'FAKE_CERT_7': self.certificate()},
        }
        for label, fake in cases.items():
            with self.subTest(label):
                self.assertEqual(self.run_script(**fake), '')


if __name__ == '__main__':
    unittest.main()
