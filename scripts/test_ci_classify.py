"""Tests of the continuous-integration change classifier, including its fail-closed paths."""

import io
from pathlib import Path
from contextlib import redirect_stdout
from unittest import mock
import unittest

import ci_classify


class ClassifyTests(unittest.TestCase):
    def mode(self, *paths, event='pull_request'):
        return ci_classify.classify(event, list(paths))['mode']

    def test_documentation_book_and_scripts_are_light(self):
        self.assertEqual(self.mode('docs/process/handoffs/kaizen.md', 'book/src/intro.md',
                                   'scripts/jev_shadow.py', 'CLAUDE.md', 'LICENSE'), 'light')

    def test_native_codegen_is_light_while_its_guards_stay_in_the_light_run(self):
        self.assertEqual(self.mode('native_codegen/src/lib.rs', 'docs/a.md'), 'light')
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        guards = workflow.split('\n  guards:\n', 1)[1].split('\n  # ', 1)[0]
        for prefix, binaries in ci_classify.LIGHT_PREFIX_GUARDS.items():
            self.assertIn(prefix, ci_classify.LIGHT_PREFIXES)
            for binary in binaries:
                with self.subTest(prefix=prefix, binary=binary):
                    self.assertIn(f'--test {binary} ', guards + ' ')

    def test_source_workflow_and_crate_readme_are_full(self):
        for path in ('src/lib.rs', 'tests/claimed_counts.rs', '.github/workflows/ci.yml',
                     'keleusma-arena/README.md', 'Cargo.lock', 'examples/scripts/README.md',
                     'editors/vscode/extension.js', 'compiler/README.md'):
            with self.subTest(path=path):
                self.assertEqual(self.mode('docs/a.md', path), 'full')

    def test_guarded_documents_and_stage_sources_are_full(self):
        self.assertEqual(self.mode('docs/roadmap/V0_2_X_ROADMAP.md'), 'full')
        self.assertEqual(self.mode('scripts/check-md-links.kel'), 'full')
        self.assertEqual(self.mode('docs/notes/example.kel'), 'full')

    def test_every_event_but_a_pull_request_is_full(self):
        for event in ('push', 'workflow_dispatch', 'schedule', ''):
            with self.subTest(event=event):
                self.assertEqual(self.mode('docs/a.md', event=event), 'full')

    def test_an_empty_or_blank_list_fails_closed(self):
        closed = {'mode': 'full', 'native': 'true', 'vscode': 'true'}
        self.assertEqual(ci_classify.classify('pull_request', []), closed)
        self.assertEqual(ci_classify.classify('pull_request', ['', '  ']), closed)
        self.assertEqual(ci_classify.classify('push', []), closed)
        self.assertEqual(ci_classify.classify('workflow_dispatch', []), closed)

    def native(self, *paths, event='pull_request'):
        return ci_classify.classify(event, list(paths))['native']

    def test_native_inputs_select_the_native_gate_on_any_event(self):
        for path in ('native_codegen/src/lib.rs', 'src/vm.rs', 'examples/scripts/01_arithmetic.kel',
                     'compiler/kel/prelude.kel', 'docs/decisions/ABSORPTION_130_BRIEF.md',
                     'docs/decisions/NATIVE_REGION_SUPPLEMENT_FORMULA.md',
                     'docs/process/REVERSE_PROMPT.md', 'docs/process/handoffs/v0.3.0.md',
                     'Cargo.toml', 'Cargo.lock', '.cargo/config.toml', 'keleusma-arena/src/lib.rs',
                     'keleusma-macros/src/lib.rs', 'keleusma-wire/src/lib.rs',
                     'keleusma-wire-derive/src/lib.rs', '.github/workflows/ci.yml',
                     'scripts/ci_classify.py'):
            for event in ('pull_request', 'push'):
                with self.subTest(path=path, event=event):
                    self.assertEqual(self.native('docs/a.md', path, event=event), 'true')

    def test_a_light_native_pull_request_still_runs_the_native_gate(self):
        result = ci_classify.classify('pull_request', ['native_codegen/tests/differential.rs'])
        self.assertEqual((result['mode'], result['native']), ('light', 'true'))

    def test_documentation_only_changes_skip_the_native_gate(self):
        for path in ('docs/process/GIT_STRATEGY.md', 'book/src/intro.md', 'CLAUDE.md',
                     'docs/decisions/ABSORPTION_130_PLAN.md', 'docs/process/handoffs/kaizen.md',
                     'scripts/jev_shadow.py', 'tests/claimed_counts.rs', 'keleusma-cli/src/main.rs'):
            with self.subTest(path=path):
                self.assertEqual(self.native(path), 'false')
        result = ci_classify.classify('pull_request', ['docs/process/GIT_STRATEGY.md'])
        self.assertEqual((result['mode'], result['native']), ('light', 'false'))

    def test_vscode_flag_follows_the_extension_directory(self):
        self.assertEqual(ci_classify.classify('push', ['editors/vscode/package.json'])['vscode'], 'true')
        self.assertEqual(ci_classify.classify('push', ['editors/vim/syntax/keleusma.vim'])['vscode'], 'false')

    def test_main_prints_sorted_outputs_and_fails_closed_on_unreadable_input(self):
        out = io.StringIO()
        with mock.patch('sys.stdin', io.StringIO('docs/a.md\n')), redirect_stdout(out):
            ci_classify.main(['--event', 'pull_request'])
        self.assertEqual(out.getvalue(), 'mode=light\nnative=false\nvscode=false\n')
        broken = mock.Mock()
        broken.read.side_effect = UnicodeError('undecodable')
        out = io.StringIO()
        with mock.patch('sys.stdin', broken), redirect_stdout(out):
            ci_classify.main(['--event', 'pull_request'])
        self.assertEqual(out.getvalue(), 'mode=full\nnative=true\nvscode=true\n')


if __name__ == '__main__':
    unittest.main()
