"""Tests of `scripts/loc.sh`, the per-category line count.

Each test builds a throwaway repository holding a copy of the script and of
`tools/cloc.lang`, with a stand-in `cloc` on PATH that records the files it was asked to
count. The checks are about selection, which is what the script decides: which tracked
files land in which category, that excluded data and untracked build output never do,
that no file is counted twice, and that a failure is reported rather than hidden. When
the real cloc is installed, one test also runs it end to end.
"""

from pathlib import Path
import os
import shutil
import stat
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

# A stand-in for cloc. It appends the file list it was given to a log, under a marker,
# and prints the banner the real tool prints so the script's filter is exercised.
FAKE_CLOC = """#!/bin/sh
for arg in "$@"; do
  case $arg in --list-file=*) list=${arg#--list-file=} ;; esac
done
echo 'github.com/AlDanial/cloc v 0.0'
echo '--- block' >> "$CLOC_LOG"
cat "$list" >> "$CLOC_LOG"
echo "counted $(wc -l < "$list" | tr -d ' ') files"
exit "${CLOC_EXIT:-0}"
"""

TRACKED = [
    'src/lib.rs', 'src/selfhost/kel/parse.kel',
    'keleusma-cli/src/main.rs', 'keleusma-cli/README.md', 'keleusma-cli/Cargo.toml',
    'tests/a.rs',
    'compiler/src/lib.rs', 'compiler/README.md', 'fuzz/fuzz_targets/x.rs',
    'fuzz/corpus/x/seed-1', 'fuzz/Cargo.lock',
    'scripts/loc.sh', 'scripts/README.md', 'scripts/gate_summary_fixtures/f.log',
    'tools/cloc.lang', 'Cargo.toml', 'Cargo.lock',
    'examples/demo.rs', 'examples/rtos/README.md', 'examples/scripts/a.kel',
    'README.md', 'docs/spec/WIRE.md', 'book/src/intro.md', 'changelog.d/x.added.md',
    'docs/process/TASKLOG.md', 'docs/process/kaizen/evidence/data.tsv',
    'docs/process/interline/checkpoint.json', 'book/po/ja.po',
]


class LocTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.repo = self.tmp / 'repo'
        for name in TRACKED:
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            # Distinct contents, because cloc skips a file identical to one already counted.
            path.write_text(f'// {name}\n')
        shutil.copy2(ROOT / 'scripts' / 'loc.sh', self.repo / 'scripts' / 'loc.sh')
        shutil.copy2(ROOT / 'tools' / 'cloc.lang', self.repo / 'tools' / 'cloc.lang')
        self.git('init', '-q', '-b', 'main')
        self.git('add', '-A')
        self.git('commit', '-q', '-m', 'initial')
        # Untracked build output, which no category may count.
        (self.repo / 'target' / 'debug').mkdir(parents=True)
        (self.repo / 'target' / 'debug' / 'out.rs').write_text('x\n')
        (self.repo / 'src' / 'scratch.rs').write_text('untracked\n')
        self.bin = self.tmp / 'bin'
        self.bin.mkdir()
        fake = self.bin / 'cloc'
        fake.write_text(FAKE_CLOC)
        fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
        self.log = self.tmp / 'cloc.log'

    def git(self, *args):
        env = dict(os.environ, GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid',
                   GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid')
        subprocess.run(['git', '-C', str(self.repo), *args], check=True,
                       capture_output=True, env=env)

    def run_loc(self, *args, path=None, shell='sh', **extra):
        env = dict(os.environ, PATH=path or f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
                   CLOC_LOG=str(self.log), **extra)
        return subprocess.run([shutil.which(shell), str(self.repo / 'scripts' / 'loc.sh'), *args],
                              capture_output=True, text=True, env=env)

    def blocks(self):
        text = self.log.read_text() if self.log.exists() else ''
        return [set(b.split()) for b in text.split('--- block')[1:]]

    def test_each_category_counts_its_own_files(self):
        expected = {
            'runtime': {'src/lib.rs', 'src/selfhost/kel/parse.kel'},
            'crates': {'keleusma-cli/src/main.rs', 'keleusma-cli/Cargo.toml'},
            'tests': {'tests/a.rs'},
            'detached': {'compiler/src/lib.rs', 'fuzz/fuzz_targets/x.rs'},
            'tooling': {'scripts/loc.sh', 'tools/cloc.lang', 'Cargo.toml'},
            'examples': {'examples/demo.rs', 'examples/scripts/a.kel'},
            'example-docs': {'examples/rtos/README.md'},
            'docs': {'README.md', 'docs/spec/WIRE.md', 'book/src/intro.md',
                     'changelog.d/x.added.md'},
            'package-docs': {'keleusma-cli/README.md', 'compiler/README.md', 'scripts/README.md'},
            'process': {'docs/process/TASKLOG.md'},
            'translations': {'book/po/ja.po'},
        }
        for category, files in expected.items():
            with self.subTest(category=category):
                self.log.unlink(missing_ok=True)
                result = self.run_loc(category)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.blocks(), [files])

    def test_the_total_is_the_union_with_nothing_counted_twice(self):
        result = self.run_loc()
        self.assertEqual(result.returncode, 0, result.stderr)
        *categories, total = self.blocks()
        union = set().union(*categories)
        self.assertEqual(total, union)
        self.assertEqual(sum(len(c) for c in categories), len(union))

    def test_excluded_data_and_untracked_output_are_never_counted(self):
        self.assertEqual(self.run_loc().returncode, 0)
        counted = set().union(*self.blocks())
        for path in ('fuzz/corpus/x/seed-1', 'fuzz/Cargo.lock', 'Cargo.lock',
                     'scripts/gate_summary_fixtures/f.log',
                     'docs/process/kaizen/evidence/data.tsv',
                     'docs/process/interline/checkpoint.json',
                     'target/debug/out.rs', 'src/scratch.rs'):
            with self.subTest(path=path):
                self.assertNotIn(path, counted)

    def test_output_has_a_heading_per_block_and_no_cloc_banner(self):
        result = self.run_loc('runtime', 'docs')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('== Runtime library, src/ ==', result.stdout)
        self.assertIn('== Documentation:', result.stdout)
        self.assertNotIn('github.com/AlDanial/cloc', result.stdout)

    def test_a_cloc_failure_fails_the_script(self):
        result = self.run_loc('runtime', CLOC_EXIT='3')
        self.assertNotEqual(result.returncode, 0)

    def test_an_unknown_category_is_refused_before_counting(self):
        result = self.run_loc('runtime', 'nonesuch')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unknown category 'nonesuch'", result.stderr)
        self.assertFalse(self.log.exists())

    def test_a_missing_cloc_is_reported(self):
        empty = self.tmp / 'empty-bin'
        empty.mkdir()
        for tool in ('git', 'dirname', 'sed', 'mktemp', 'rm', 'sort', 'cat'):
            found = shutil.which(tool)
            if found:
                (empty / tool).symlink_to(found)
        result = self.run_loc(path=str(empty))
        self.assertEqual(result.returncode, 127)
        self.assertIn('cloc is not installed', result.stderr)

    def test_the_list_option_names_every_category(self):
        result = self.run_loc('-l')
        self.assertEqual(result.returncode, 0, result.stderr)
        names = [line.split()[0] for line in result.stdout.splitlines()]
        self.assertEqual(names, ['runtime', 'crates', 'tests', 'detached', 'tooling',
                                 'examples', 'example-docs', 'docs', 'package-docs',
                                 'process', 'translations', 'total'])

    @unittest.skipUnless(shutil.which('dash'), 'dash is not installed')
    def test_it_runs_under_dash(self):
        result = self.run_loc('runtime', shell='dash')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.blocks(), [{'src/lib.rs', 'src/selfhost/kel/parse.kel'}])

    @unittest.skipUnless(shutil.which('cloc'), 'cloc is not installed')
    def test_the_real_cloc_counts_keleusma_source(self):
        real = os.path.dirname(shutil.which('cloc'))
        result = self.run_loc('runtime', path=f'{real}{os.pathsep}{os.environ["PATH"]}')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Keleusma', result.stdout)
        self.assertIn('Rust', result.stdout)


if __name__ == '__main__':
    unittest.main()
