"""Tests of the continuous-integration change classifier, including its fail-closed paths."""

import io
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
        self.assertEqual(ci_classify.classify('pull_request', []), {'mode': 'full', 'vscode': 'true'})
        self.assertEqual(ci_classify.classify('pull_request', ['', '  ']), {'mode': 'full', 'vscode': 'true'})

    def test_vscode_flag_follows_the_extension_directory(self):
        self.assertEqual(ci_classify.classify('push', ['editors/vscode/package.json'])['vscode'], 'true')
        self.assertEqual(ci_classify.classify('push', ['editors/vim/syntax/keleusma.vim'])['vscode'], 'false')

    def test_main_prints_sorted_outputs_and_fails_closed_on_unreadable_input(self):
        out = io.StringIO()
        with mock.patch('sys.stdin', io.StringIO('docs/a.md\n')), redirect_stdout(out):
            ci_classify.main(['--event', 'pull_request'])
        self.assertEqual(out.getvalue(), 'mode=light\nvscode=false\n')
        broken = mock.Mock()
        broken.read.side_effect = UnicodeError('undecodable')
        out = io.StringIO()
        with mock.patch('sys.stdin', broken), redirect_stdout(out):
            ci_classify.main(['--event', 'pull_request'])
        self.assertEqual(out.getvalue(), 'mode=full\nvscode=true\n')


if __name__ == '__main__':
    unittest.main()
