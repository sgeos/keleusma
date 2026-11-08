"""Tests of `scripts/metrics.py`, the tracked-metrics report.

Each test builds a throwaway repository holding copies of the script, `scripts/loc.sh` and
`tools/cloc.lang`, with small sources whose counts are known, and a stand-in `cloc` on PATH
that answers in cloc's JSON shape. The checks are about what the collectors count, that an
unavailable collector is reported rather than dropped, and that `--require` turns an
unavailable collector into a failed run. The GitHub collector is only checked for refusing
to run unless asked, since its figures come from a live service.
"""

from pathlib import Path
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

FAKE_CLOC = """#!/bin/sh
for arg in "$@"; do
  case $arg in --list-file=*) list=${arg#--list-file=} ;; esac
done
n=$(wc -l < "$list" | tr -d ' ')
printf '{"header": {}, "Rust": {"nFiles": %s, "code": %s, "comment": 1, "blank": 1}, ' "$n" "$((n * 10))"
printf '"SUM": {"nFiles": %s, "code": %s, "comment": %s, "blank": %s}}\\n' "$n" "$((n * 10))" "$n" "$n"
"""

OP_ENUM = 'pub enum Op {\n' + ''.join(f'    Op{i},\n' for i in range(60)) + '}\n'

FILES = {
    'src/bytecode.rs': ('pub const BYTECODE_VERSION: u16 = 2;\n'
                        'const _: () = assert!(core::mem::size_of::<Value>() == 32);\n' + OP_ENUM),
    'src/vm.rs': ('fn a() { unsafe { x() } }\nunsafe fn b() {}\n// unsafe { not counted }\n'
                  'unsafe impl Sync for T {}\n#[test]\nfn t() {}\n'),
    'tests/a.rs': '#[test]\nfn one() {}\n#[test]\n#[ignore]\nfn two() {}\n// #[test] not counted\n',
    'keleusma-cli/src/main.rs': 'fn main() {}\n',
    'book/po/ja.po': ('msgid ""\nmsgstr "header"\n\nmsgid "Hello"\nmsgstr "Konnichiwa"\n\n'
                      '#, fuzzy\nmsgid "World"\nmsgstr "Sekai"\n\nmsgid "Left"\nmsgstr ""\n'),
    'changelog.d/README.md': 'rules\n',
    'changelog.d/2026-11-07-x.added.md': 'a fragment\n',
    'docs/process/handoffs/kaizen.md': 'within a budget of 12 KB.\n' + 'x\n' * 4,
    'docs/process/handoffs/big.md': 'x\n' * 40,
    'docs/proofs/X_PROOF.md': '**Theorem A.**\n**Lemma 1.**\n**Lemma 2.**\n',
    'proofs/Core.v': ('Theorem sound : True.\nProof. exact I. Qed.\n'
                      'Lemma open : False.\nAdmitted.\n(*\nLemma in_comment : True.\nAdmitted.\n*)\n'),
}


class MetricsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.repo = self.tmp / 'repo'
        for name, text in FILES.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        for name in ('scripts/metrics.py', 'scripts/loc.sh', 'tools/cloc.lang'):
            (self.repo / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, self.repo / name)
        env = dict(os.environ, GIT_AUTHOR_NAME='t', GIT_AUTHOR_EMAIL='t@example.invalid',
                   GIT_COMMITTER_NAME='t', GIT_COMMITTER_EMAIL='t@example.invalid')
        for args in (['init', '-q', '-b', 'main'], ['add', '-A'], ['commit', '-q', '-m', 'initial']):
            subprocess.run(['git', '-C', str(self.repo), *args], check=True, capture_output=True, env=env)
        self.head = subprocess.run(['git', '-C', str(self.repo), 'rev-parse', 'HEAD'],
                                   capture_output=True, text=True, check=True).stdout.strip()
        self.bin = self.tmp / 'bin'
        self.bin.mkdir()
        cloc = self.bin / 'cloc'
        cloc.write_text(FAKE_CLOC)
        cloc.chmod(cloc.stat().st_mode | stat.S_IXUSR)

    def write_footprint(self, commit):
        report = {'commit': commit, 'clean_tree': True, 'toolchain': 'rustc 1.92.0',
                  'configurations': [{'name': 'baseline', 'runtime_flash': 0, 'runtime_static_ram': 0},
                                     {'name': 'vm-verify', 'runtime_flash': 246436,
                                      'runtime_static_ram': 0}]}
        path = self.repo / 'footprint' / 'target' / 'footprint.json'
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report))

    def run_metrics(self, *args):
        env = dict(os.environ, PATH=f'{self.bin}{os.pathsep}{os.environ["PATH"]}')
        return subprocess.run([sys.executable, '-B', str(self.repo / 'scripts' / 'metrics.py'), *args],
                              capture_output=True, text=True, env=env, cwd=self.repo)

    def report(self):
        return json.loads((self.repo / 'target' / 'metrics' / 'metrics.json').read_text())['metrics']

    def values(self, name):
        entry = self.report()[name]
        self.assertEqual(entry['status'], 'ok', entry)
        return entry['values']

    def test_the_report_names_its_commit_and_every_collector(self):
        result = self.run_metrics()
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads((self.repo / 'target' / 'metrics' / 'metrics.json').read_text())
        self.assertEqual(report['commit'], self.head)
        self.assertEqual(set(report['metrics']), {'lines', 'tests', 'unsafe', 'invariants', 'footprint',
                                                  'documentation', 'process', 'dependencies',
                                                  'proofs', 'github', 'tightness', 'machine'})

    def test_test_functions_and_ignored_tests_are_counted_outside_comments(self):
        self.run_metrics()
        tests = self.values('tests')
        self.assertEqual(tests['tests'], {'test_functions': 2, 'ignored': 1})
        self.assertEqual(tests['runtime']['test_functions'], 1)
        self.assertEqual(tests['integration_test_files'], 1)

    def test_unsafe_is_counted_by_kind_outside_comments(self):
        self.run_metrics()
        self.assertEqual(self.values('unsafe'), {'blocks': 1, 'functions': 1, 'impls': 1})

    def test_the_invariants_are_read_from_the_source(self):
        self.run_metrics()
        self.assertEqual(self.values('invariants'),
                         {'bytecode_version': 2, 'value_slot_bytes': 32, 'opcode_count': 60})

    def test_a_source_that_stops_matching_is_unavailable_not_zero(self):
        (self.repo / 'src' / 'bytecode.rs').write_text('pub const BYTECODE_VERSION: u16 = 2;\n')
        self.run_metrics()
        self.assertEqual(self.report()['invariants']['status'], 'unavailable')

    def test_translation_counts_untranslated_and_fuzzy_messages(self):
        self.run_metrics()
        documentation = self.values('documentation')
        self.assertEqual(documentation['translations']['ja'],
                         {'messages': 3, 'untranslated': 1, 'fuzzy': 1})
        self.assertEqual(documentation['pending_changelog_fragments'], 1)

    def test_state_file_budgets_and_the_largest_handoff(self):
        self.run_metrics()
        process = self.values('process')
        self.assertEqual(process['handoffs']['kaizen.md']['budget_bytes'], 12000)
        self.assertIsNone(process['handoffs']['big.md']['budget_bytes'])
        self.assertEqual(process['largest_handoff']['file'], 'big.md')

    def test_rocq_results_and_admitted_steps_are_counted_outside_comments(self):
        self.run_metrics()
        proofs = self.values('proofs')
        self.assertEqual(proofs['rocq'], {'files': 1, 'theorems': 2, 'admitted': 1})
        self.assertEqual(proofs['prose'], {'theorems': 1, 'lemmas': 2, 'corollaries': 0})

    def test_lines_come_from_the_loc_categories(self):
        self.run_metrics()
        lines = self.values('lines')
        self.assertEqual(lines['runtime']['files'], 2)
        self.assertEqual(lines['derived']['shipping_code'], 30)

    def test_footprint_states_whether_it_describes_this_commit(self):
        self.write_footprint(self.head)
        self.run_metrics()
        self.assertIs(self.values('footprint')['matches_head'], True)
        self.write_footprint('0' * 40)
        self.run_metrics()
        footprint = self.values('footprint')
        self.assertIs(footprint['matches_head'], False)
        self.assertEqual(footprint['configurations']['vm-verify']['flash'], 246436)

    def test_machine_peaks_keep_the_largest_recorded_figure_per_job(self):
        log = self.repo / 'target' / 'machine-guard' / 'peaks.tsv'
        log.parent.mkdir(parents=True)
        log.write_text('2026-11-08T10:00:00Z\tgate\t900\t800\t60\t0\n'
                       '2026-11-08T11:00:00Z\tgate\tunknown\t1200\t70\t3\n'
                       'a malformed line\n')
        self.run_metrics()
        gate = self.values('machine')['jobs']['gate']
        self.assertEqual(gate['runs'], 2)
        self.assertEqual(gate['peak_footprint'], 900)
        self.assertEqual(gate['peak_resident'], 1200)
        self.assertEqual(gate['last_status'], 3)

    def test_text_after_the_cloc_json_is_kept_as_a_note(self):
        cloc = self.bin / 'cloc'
        cloc.write_text(FAKE_CLOC + 'echo "1 file ignored."\n')
        self.run_metrics()
        lines = self.values('lines')
        self.assertEqual(lines['runtime']['files'], 2)
        self.assertEqual(lines['cloc_notes']['runtime'], '1 file ignored.')

    def test_cloc_runs_without_its_timeout_guard(self):
        # cloc 2.06 skipped slow files on a CI runner and warned after its JSON. The
        # stand-in does the same unless asked for --timeout 0.
        cloc = self.bin / 'cloc'
        cloc.write_text(FAKE_CLOC.replace('#!/bin/sh\n', '#!/bin/sh\ncase " $* " in *" --timeout 0 "*) ;; '
                                          '*) trap \'echo "One or more files took longer"\' EXIT ;; esac\n', 1))
        self.run_metrics()
        self.assertNotIn('cloc_notes', self.values('lines'))

    def test_a_collector_defect_is_recorded_and_the_rest_still_run(self):
        (self.repo / 'docs' / 'process' / 'handoffs' / 'kaizen.md').write_bytes(b'\xff\xfe not utf-8')
        cloc = self.bin / 'cloc'
        cloc.write_text('#!/bin/sh\necho "{ not json"\n')
        result = self.run_metrics('--require', 'lines')
        self.assertEqual(result.returncode, 1)
        report = self.report()
        self.assertEqual(report['lines']['status'], 'unavailable')
        self.assertIn('not JSON', report['lines']['reason'])
        self.assertEqual(report['tests']['status'], 'ok')

    def test_an_unexpected_exception_is_an_error_entry_not_a_crash(self):
        (self.repo / 'src' / 'bytecode.rs').unlink()
        result = self.run_metrics('--require', 'invariants')
        self.assertEqual(result.returncode, 1)
        entry = self.report()['invariants']
        self.assertEqual(entry['status'], 'error')
        self.assertIn('FileNotFoundError', entry['reason'])
        self.assertEqual(self.report()['documentation']['status'], 'ok')

    def test_unavailable_collectors_carry_a_reason(self):
        self.run_metrics()
        report = self.report()
        self.assertEqual(report['footprint']['status'], 'unavailable')
        self.assertIn('footprint.sh', report['footprint']['reason'])
        self.assertEqual(report['github']['status'], 'unavailable')
        self.assertIn('--github', report['github']['reason'])

    def test_require_fails_the_run_when_a_named_collector_is_unavailable(self):
        result = self.run_metrics('--require', 'tests,footprint')
        self.assertEqual(result.returncode, 1)
        self.assertIn('required collectors unavailable: footprint', result.stderr)
        self.write_footprint(self.head)
        self.assertEqual(self.run_metrics('--require', 'tests,footprint').returncode, 0)

    def test_an_unknown_required_collector_is_refused(self):
        result = self.run_metrics('--require', 'nonesuch')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unknown collectors: nonesuch', result.stderr)

    def test_each_run_overwrites_the_report(self):
        self.run_metrics()
        self.assertEqual(self.report()['footprint']['status'], 'unavailable')
        self.write_footprint(self.head)
        self.run_metrics()
        self.assertEqual(self.report()['footprint']['status'], 'ok')


if __name__ == '__main__':
    unittest.main()
