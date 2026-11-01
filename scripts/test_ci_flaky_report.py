"""Tests of the flaky-test report, including its fail-closed paths."""

import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

import ci_flaky_report as fr

# The shape nextest 0.9.138 wrote for a test that failed once and then passed, trimmed.
REPORT = """<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="3" failures="0" errors="0">
    <testsuite name="keleusma" tests="2" disabled="0" errors="0" failures="0">
        <testcase name="tests::steady" classname="keleusma" time="0.014">
        </testcase>
        <testcase name="tests::flaky" classname="keleusma" time="0.012">
            <flakyFailure time="0.010" message="panicked" type="test failure">first</flakyFailure>
        </testcase>
    </testsuite>
    <testsuite name="keleusma::hot_swap" tests="1" disabled="0" errors="0" failures="0">
        <testcase name="twice" classname="keleusma::hot_swap" time="0.5">
            <flakyFailure time="0.1" message="a" type="test failure">a</flakyFailure>
            <flakyFailure time="0.1" message="b" type="test failure">b</flakyFailure>
        </testcase>
    </testsuite>
</testsuites>
"""


class ParseTests(unittest.TestCase):
    def test_flaky_tests_are_counted_by_failed_attempt(self):
        tests, found = fr.flaky_tests(REPORT)
        self.assertEqual(tests, 3)
        self.assertEqual(found, [('keleusma', 'tests::flaky', 1), ('keleusma::hot_swap', 'twice', 2)])

    def test_a_report_without_flakes_lists_none(self):
        clean = REPORT.replace('<flakyFailure', '<!--').replace('</flakyFailure>', '-->')
        self.assertEqual(fr.flaky_tests(clean), (3, []))

    def test_a_failure_is_not_a_flake(self):
        failed = REPORT.replace('flakyFailure', 'failure')
        self.assertEqual(fr.flaky_tests(failed)[1], [])

    def test_malformed_xml_and_a_foreign_root_are_refused(self):
        for text in ('<testsuites>', '<html></html>'):
            with self.subTest(text=text), self.assertRaises(fr.ReportError):
                fr.flaky_tests(text)


class MainTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.report = self.root / 'junit.xml'
        self.summary = self.root / 'summary.md'

    def tearDown(self):
        self.temporary.cleanup()

    def run_main(self, *extra):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = fr.main(['--report', str(self.report), '--label', 'Test', *extra])
        return code, out.getvalue(), err.getvalue()

    def test_flakes_become_warnings_and_summary_lines_without_failing(self):
        self.report.write_text(REPORT, encoding='utf-8')
        code, out, _ = self.run_main('--summary', str(self.summary))
        self.assertEqual(code, 0)
        self.assertIn('::warning title=Flaky test::Test: keleusma tests::flaky passed after 1 failed attempt\n', out)
        self.assertIn('passed after 2 failed attempts', out)
        self.assertIn('Test: 2 of 3 tests needed a retry', out)
        self.assertIn('- `keleusma::hot_swap` `twice`, 2 failed attempts', self.summary.read_text(encoding='utf-8'))

    def test_a_newline_in_a_name_cannot_end_the_workflow_command(self):
        self.report.write_text(REPORT.replace('name="twice"', 'name="a&#10;::error::b"'), encoding='utf-8')
        out = self.run_main()[1]
        self.assertNotIn('\n::error::', out)
        self.assertIn('a%0A::error::b', out)

    def test_a_missing_report_fails_closed(self):
        code, _, err = self.run_main()
        self.assertEqual(code, 1)
        self.assertIn('does not exist', err)

    def test_a_report_with_no_test_cases_fails_closed(self):
        self.report.write_text('<testsuites name="nextest-run"></testsuites>', encoding='utf-8')
        code, _, err = self.run_main()
        self.assertEqual(code, 1)
        self.assertIn('no test cases', err)


if __name__ == '__main__':
    unittest.main()
