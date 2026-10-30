#!/usr/bin/env python3
"""List the tests that nextest retried in a CI run, from its JUnit report.

The `ci` profile in .config/nextest.toml retries a failing test twice, so a test that
fails and then passes does not fail the run. That keeps one flake from failing a merge,
and it also hides the flake. This script reads the JUnit report nextest writes and
names every test that needed a retry, as a warning annotation and in the job summary,
so the count that decides whether to fix or quarantine a test is visible. It never
changes a run's result for a flake. See docs/process/kaizen/2026-10-30-ci-practices-spike.md,
recommendation R5, adopted by the operator on 2026-10-30.

A test is flaky when its `testcase` holds a `flakyFailure` element, one per failed
attempt before the passing one. A test that failed every attempt is a failure, which
nextest already reports. The script fails closed when the report is missing or is not
a nextest JUnit report, because a step that silently found nothing would look the same
as a run with no flakes.

The report is produced by nextest on the same runner, so it is trusted to be
well-formed XML from that tool. The parser is the standard library's, which does not
resolve external entities.
"""

import argparse
import sys
import xml.etree.ElementTree as ElementTree
from pathlib import Path

DEFAULT_REPORT = Path('target/nextest/ci/junit.xml')


class ReportError(Exception):
    """The report is missing or is not the shape nextest writes."""


def flaky_tests(xml_text):
    """Return (tests, [(classname, name, failed_attempts)]) from a JUnit report."""
    try:
        root = ElementTree.fromstring(xml_text)
    except ElementTree.ParseError as error:
        raise ReportError(f'the report is not well-formed XML: {error}') from None
    if root.tag != 'testsuites':
        raise ReportError(f'expected a <testsuites> root, found <{root.tag}>')
    cases = root.findall('./testsuite/testcase')
    found = []
    for case in cases:
        attempts = len(case.findall('flakyFailure'))
        if attempts:
            found.append((case.get('classname', '?'), case.get('name', '?'), attempts))
    return len(cases), sorted(found)


def escape(text):
    """Make text safe inside a workflow command, which ends at a newline."""
    return text.replace('%', '%25').replace('\r', '%0D').replace('\n', '%0A')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    parser.add_argument('--report', type=Path, default=DEFAULT_REPORT)
    parser.add_argument('--label', required=True, help='the job or step, named in the output')
    parser.add_argument('--summary', type=Path, help='a file to append Markdown to')
    args = parser.parse_args(argv)
    try:
        if not args.report.is_file():
            raise ReportError(f'{args.report} does not exist, so nextest wrote no report')
        tests, found = flaky_tests(args.report.read_text(encoding='utf-8'))
    except (ReportError, OSError, UnicodeDecodeError) as error:
        print(f'flaky report: {error}', file=sys.stderr)
        return 1
    if tests == 0:
        print(f'flaky report: {args.report} lists no test cases', file=sys.stderr)
        return 1
    described = [(classname, name, f'{attempts} failed attempt{"" if attempts == 1 else "s"}')
                 for classname, name, attempts in found]
    for classname, name, attempts in described:
        message = escape(f'{args.label}: {classname} {name} passed after {attempts}')
        print(f'::warning title=Flaky test::{message}')
    line = f'{args.label}: {len(found)} of {tests} tests needed a retry'
    print(line)
    if args.summary:
        with args.summary.open('a', encoding='utf-8') as out:
            out.write(f'**{line}.**\n\n')
            for classname, name, attempts in described:
                out.write(f'- `{classname}` `{name}`, {attempts}\n')
            if found:
                out.write('\n')
    return 0


if __name__ == '__main__':
    sys.exit(main())
