"""Offline tests of the selection cohort, including negative paths and restore integrity."""

import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import jev_selection as selection
import jev_shadow as base

ROOT = Path(__file__).resolve().parents[1]
DOCUMENT = ('Preamble line.\n'
            '## Build\nRun cargo build.\n'
            '## Gates\nThe release gate runs twelve steps.\n'
            '## History\nOld notes.\n')
HEADINGS = ['## Build', '## Gates', '## History']


def fake_client(wanted):
    """A response choosing, for each question, the option named in `wanted`."""
    def client(payload, key):
        answers = {}
        for question, asked in payload['questions'].items():
            chosen = next(option for option in asked['criteria'] if option.split('_', 1)[1] == wanted[question])
            answers[question] = {'type': 'choice', 'choice': chosen, 'confidence': 1.0,
                                 'probabilities': {option: float(option == chosen) for option in asked['criteria']}}
        return {'status': 'ok', 'elapsed_seconds': 0.01, 'http_status': 200,
                'response': {'model': payload['model'], 'answers': answers,
                             'usage': {'input_tokens': 900, 'output_tokens': 30}}}
    return client


class SelectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # In the system temporary directory, for the sandbox reason given in test_jev_shadow.py.
        cls.directory = tempfile.TemporaryDirectory(prefix='jev-selection-tests-')
        cls.root = Path(cls.directory.name).resolve()
        cls.repo = cls.root / 'repo'
        cls.repo.mkdir()
        subprocess.run(['git', 'init', '-q', str(cls.repo)], check=True)
        (cls.repo / '.gitignore').write_text('secret/\ntmp/\n')
        (cls.repo / 'guide.md').write_text(DOCUMENT)
        (cls.repo / 'gate.log').write_text('error[E0308]: mismatched types\n')
        base.git(cls.repo, 'add', '.')
        base.git(cls.repo, '-c', 'user.name=Pilot Test', '-c', 'user.email=pilot@example.invalid', 'commit', '-qm', 'fixture')

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def setUp(self):
        self.local = tempfile.TemporaryDirectory(dir=self.root)
        self.store = Path(self.local.name) / 'store' / 'selection'
        self.cfg = selection.config()
        self.live = dict(self.cfg, live_evaluation=True)
        self.db = base.connect(self.store, selection.pinned(self.cfg))
        self.env = mock.patch.dict(os.environ, {'TYPESAFE_API_KEY': 'synthetic-unit-test-key'})
        self.env.start()
        self.packet = {
            'id': 'sel-one', 'task_id': 'sel-task', 'kind': 'sections', 'sample_kind': 'development',
            'input': {'task': 'Change the release gate.', 'document': DOCUMENT}, 'sections': list(HEADINGS),
            'baseline': {'answers': {'section_01': 'bears', 'section_02': 'bears', 'section_03': 'does_not_bear'},
                         'actor': 'author-session'},
            'source_refs': ['guide.md'],
        }

    def tearDown(self):
        self.db.close()
        self.env.stop()
        self.local.cleanup()

    def record(self, packet=None, line='kaizen'):
        return selection.record(self.db, self.repo, line, packet or self.packet, self.cfg)

    def gate_packet(self, case_id):
        return {'id': case_id, 'task_id': 'gate-task', 'kind': 'gate_log', 'sample_kind': 'development',
                'input': {'log': 'error[E0308]: mismatched types'}, 'sections': None,
                'baseline': {'answers': {'judgment': 'compile_error'}, 'actor': 'author-session'},
                'source_refs': ['gate.log']}

    def test_option_order_is_deterministic_and_varies_between_cases(self):
        first = selection.request_for(self.gate_packet('gate-a'), self.cfg)
        self.assertEqual(first, selection.request_for(self.gate_packet('gate-a'), self.cfg))
        orders = set()
        for index in range(20):
            criteria = selection.request_for(self.gate_packet(f'gate-{index}'), self.cfg)['questions']['judgment']['criteria']
            names = [key.split('_', 1)[1] for key in sorted(criteria)]
            self.assertEqual(sorted(names), sorted(self.cfg['kinds']['gate_log']['options']))
            orders.add(tuple(names))
        self.assertGreater(len(orders), 1)

    def test_request_carries_input_and_questions_but_not_baseline(self):
        request = selection.request_for(self.packet, self.cfg)
        self.assertEqual(request['state'], {'input': self.packet['input']})
        self.assertEqual(set(request['questions']), {'section_01', 'section_02', 'section_03'})
        self.assertIn('"## Gates"', request['questions']['section_02']['instructions'])
        self.assertNotIn('author-session', json.dumps(request))

    def test_headings_must_be_whole_unique_lines_in_document_order(self):
        for headings in (['## Build', '## Missing'], ['## Build', '## Gates', '## Gates'],
                         ['## Gates', '## Build'], ['## Bui', '## Gates'], ['## Build']):
            packet = copy.deepcopy(self.packet)
            packet['sections'] = headings
            packet['baseline']['answers'] = {f'section_{i:02d}': 'bears' for i in range(1, len(headings) + 1)}
            with self.subTest(headings=headings), self.assertRaises(ValueError):
                selection.validate_packet(packet, self.cfg)
        packet = copy.deepcopy(self.packet)
        packet['input']['document'] = DOCUMENT + '## Build\n'
        with self.assertRaises(ValueError):
            selection.validate_packet(packet, self.cfg)

    def test_section_bytes_cover_the_document_after_the_preamble(self):
        sizes = selection.section_bytes(self.packet)
        self.assertEqual(sizes, [len('## Build\nRun cargo build.\n'), len('## Gates\nThe release gate runs twelve steps.\n'),
                                 len('## History\nOld notes.\n')])
        self.assertEqual(sum(sizes), len(DOCUMENT.encode()) - len('Preamble line.\n'))

    def test_input_must_occur_verbatim_in_a_source(self):
        packet = copy.deepcopy(self.packet)
        packet['input']['document'] = DOCUMENT.replace('twelve', 'thirteen')
        with self.assertRaisesRegex(ValueError, 'verbatim'):
            self.record(packet)

    def test_baseline_and_label_must_answer_every_question_with_a_known_option(self):
        for answers in ({'section_01': 'bears', 'section_02': 'bears'},
                        {'section_01': 'bears', 'section_02': 'bears', 'section_03': 'maybe'},
                        {'section_01': 'bears', 'section_02': 'bears', 'section_03': 'bears', 'section_04': 'bears'}):
            packet = copy.deepcopy(self.packet)
            packet['baseline']['answers'] = answers
            with self.subTest(answers=answers), self.assertRaises(ValueError):
                selection.validate_packet(packet, self.cfg)
        self.record()
        row = base.sample(self.db, 'sel-one', 'kaizen')
        own = {'answers': self.packet['baseline']['answers'], 'reviewer': 'author-session',
               'independent': True, 'rationale': 'Self review.'}
        with self.assertRaisesRegex(ValueError, 'self-declare'):
            selection.validate_label(own, row, self.cfg)

    def test_live_evaluation_is_refused_until_authorized(self):
        self.record()
        row = base.sample(self.db, 'sel-one', 'kaizen')
        client = mock.Mock()
        with self.assertRaisesRegex(ValueError, 'operator authorization'):
            selection.evaluate(self.db, 'sel-one', 'kaizen', row['request_hash'], self.cfg, client=client)
        client.assert_not_called()
        self.assertIsNone(base.sample(self.db, 'sel-one', 'kaizen')['started'])

    def test_full_flow_reports_question_agreement_misses_and_bytes(self):
        self.record()
        row = base.sample(self.db, 'sel-one', 'kaizen')
        jev = {'section_01': 'bears', 'section_02': 'does_not_bear', 'section_03': 'does_not_bear'}
        truth = {'section_01': 'bears', 'section_02': 'does_not_bear', 'section_03': 'bears'}
        result = selection.evaluate(self.db, 'sel-one', 'kaizen', row['request_hash'], self.live, client=fake_client(jev))
        self.assertEqual(result['status'], 'ok')
        self.assertTrue(result['answers_hidden'])
        selection.label_case(self.db, 'sel-one', 'kaizen', {'answers': truth, 'reviewer': 'reviewer-session',
                             'independent': True, 'rationale': 'History records the gate decision.'}, self.cfg)
        with self.assertRaisesRegex(ValueError, 'reveal'):
            selection.reveal(self.db, 'sel-one', 'kaizen')
        base.finish(self.db, 'kaizen', 'sel-task', {'resolved': True, 'system2_input_tokens': None,
                    'system2_output_tokens': None, 'elapsed_seconds': None, 'notes': 'Fixture outcome.'})
        self.assertEqual(selection.reveal(self.db, 'sel-one', 'kaizen')['jev_answers'], jev)
        group = selection.report(self.db, self.cfg)['groups']['kaizen/sections']
        sizes = selection.section_bytes(self.packet)
        self.assertEqual({key: group[key] for key in ('judged_cases', 'judged_questions', 'baseline_correct', 'jev_correct',
                          'corrected', 'regressed', 'needed', 'baseline_misses', 'jev_misses')},
                         {'judged_cases': 1, 'judged_questions': 3, 'baseline_correct': 1, 'jev_correct': 2,
                          'corrected': 1, 'regressed': 0, 'needed': 2, 'baseline_misses': 1, 'jev_misses': 1})
        self.assertEqual(group['needed_bytes'], sizes[0] + sizes[2])
        self.assertEqual(group['jev_selected_bytes'], sizes[0])
        self.assertEqual(group['jev_missed_bytes'], sizes[2])
        self.assertEqual(group['reported_input_tokens'], 900)

    def test_multi_question_and_single_question_kinds(self):
        diff = 'diff --git a/gate.log b/gate.log'
        (self.repo / 'tmp').mkdir(exist_ok=True)
        (self.repo / 'tmp' / 'diff.txt').write_text(diff + '\n')
        packet = {'id': 'diff-one', 'task_id': 'diff-task', 'kind': 'diff_scope', 'sample_kind': 'development',
                  'input': {'diff': diff}, 'sections': None,
                  'baseline': {'answers': {'tests': 'no', 'ci': 'yes', 'hooks': 'no'}, 'actor': 'author-session'},
                  'source_refs': ['tmp/diff.txt']}
        self.record(packet)
        self.assertEqual(set(base.preview(self.db, 'diff-one', 'kaizen')['outbound_request']['questions']), {'tests', 'ci', 'hooks'})
        self.record(self.gate_packet('gate-one'))
        blind = selection.blind_case(self.db, 'gate-one', 'kaizen', self.cfg)
        self.assertNotIn('baseline', json.dumps(blind))
        self.assertEqual(set(blind['questions']), {'judgment'})

    def test_home_directory_credentials_and_size_are_refused(self):
        packet = self.gate_packet('gate-home')
        packet['input']['log'] = f'error at {Path.home()}/x.rs'
        with self.assertRaisesRegex(ValueError, 'home directory'):
            self.record(packet)
        packet['input']['log'] = 'Authorization: Bearer fake'
        with self.assertRaisesRegex(ValueError, 'credential'):
            selection.validate_packet(packet, self.cfg)
        packet['input']['log'] = 'x' * 60001
        with self.assertRaises(ValueError):
            selection.validate_packet(packet, self.cfg)
        packet['input']['log'] = 'x'
        packet['sections'] = HEADINGS
        with self.assertRaisesRegex(ValueError, 'only a sections case'):
            selection.validate_packet(packet, self.cfg)
        small = dict(self.cfg, max_payload_bytes=500)
        with self.assertRaisesRegex(ValueError, 'too large'):
            selection.validate_packet(self.packet, small)

    def test_restore_round_trips_and_detects_a_tampered_request(self):
        self.record()
        row = base.sample(self.db, 'sel-one', 'kaizen')
        jev = dict(self.packet['baseline']['answers'])
        selection.evaluate(self.db, 'sel-one', 'kaizen', row['request_hash'], self.live, client=fake_client(jev))
        exported = base.export_data(self.db)
        fresh = base.connect(Path(self.local.name) / 'fresh', selection.pinned(self.cfg))
        try:
            self.assertEqual(selection.restore(fresh, copy.deepcopy(exported), self.cfg)['samples'], 1)
            self.assertEqual(base.export_data(fresh), exported)
        finally:
            fresh.close()
        tampered = copy.deepcopy(exported)
        request = json.loads(tampered['samples'][0]['request'])
        criteria = request['questions']['section_01']['criteria']
        request['questions']['section_01']['criteria'] = dict(zip(sorted(criteria, reverse=True), criteria.values()))
        tampered['samples'][0]['request'] = base.packed(request)
        other = base.connect(Path(self.local.name) / 'other', selection.pinned(self.cfg))
        try:
            with self.assertRaisesRegex(ValueError, 'request mismatch'):
                selection.restore(other, tampered, self.cfg)
        finally:
            other.close()

    def test_live_evaluation_flag_is_not_pinned_in_the_store(self):
        self.assertNotIn('live_evaluation', selection.pinned(self.cfg))
        base.connect(self.store, selection.pinned(self.live)).close()
        with self.assertRaisesRegex(ValueError, 'configuration differs'):
            base.connect(self.store, selection.pinned(dict(self.cfg, max_calls=99))).close()

    def test_refused_install_copies_nothing(self):
        for module, cfg, store in ((selection, self.cfg, Path(self.local.name) / 'install' / 'selection'),
                                   (base, base.config(), Path(self.local.name) / 'install-base')):
            with self.subTest(module=module.__name__):
                module.install(store, self.repo, 'v0.2.3', cfg)
                root = store.parent if module is selection else store
                copied = root / 'jev_shadow.py'
                copied.unlink()
                with self.assertRaisesRegex(ValueError, 'another line'):
                    module.install(store, self.repo, 'kaizen', cfg)
                self.assertFalse(copied.exists())
                self.assertEqual(list(root.glob('.install-*')), [])


if __name__ == '__main__':
    unittest.main()
