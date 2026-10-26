#!/usr/bin/env python3
"""Gate summary contract tests over real captured failures and live commands."""

from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

from gate_summary import summarize

SCRIPT = Path(__file__).with_name("gate_summary.py")
FIXTURES = Path(__file__).with_name("gate_summary_fixtures")
# An oracle written independently of the implementation's markers. Every line it
# matches in a failing fixture must reach the summary.
ORACLE = re.compile(r"FAILED|panicked|^error|TIMEOUT|SIGABRT|Diff in|didn't exit")
ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\([A-Za-z]")


def fixture(name):
    lines = (FIXTURES / (name + ".txt")).read_text(encoding="utf-8").splitlines()
    status = int(lines[0].removeprefix("exit="))
    return status, lines[1:]


class FixtureTests(unittest.TestCase):
    EXPECTED = {
        "fail_cargo_test": ("tests::assertion_fails ... FAILED", "tests::unwrap_panics ... FAILED",
                            "two is not three", "called `Option::unwrap()` on a `None` value",
                            "test result: FAILED. 1 passed; 2 failed"),
        "fail_nextest": ("FAIL [", "tests::unwrap_panics", "tests::assertion_fails", "two is not three",
                         "3 tests run: 1 passed, 2 failed"),
        "fail_nextest_timeout": ("TIMEOUT [", "tests::hangs", "1 timed out"),
        "fail_compile": ("error[E0308]: mismatched types", "--> src/lib.rs:1:23", "could not compile"),
        "fail_doctest": ("src/lib.rs - two (line 1) ... FAILED", "doctest failed"),
        "fail_clippy": ("returning the result of a `let` binding", "using `clone` on type `u32`"),
        "fail_fmt": ("Diff in /WORK/c/src/lib.rs:1:",),
        "fail_abort": ("SIGABRT", "test failed, to rerun pass `--lib`"),
    }

    def test_fixtures_are_present_and_real(self):
        # Non-vacuity. A missing or empty fixture would make every assertion below pass.
        for name in list(self.EXPECTED) + ["pass_cargo_test", "pass_nextest"]:
            status, lines = fixture(name)
            self.assertGreater(len(lines), 3, name)
            self.assertEqual(status == 0, name.startswith("pass_"), name)

    def test_each_failure_is_surfaced(self):
        for name, needles in self.EXPECTED.items():
            status, lines = fixture(name)
            summary = "\n".join(summarize(lines, status))
            for needle in needles:
                self.assertIn(needle, summary, name)

    def test_no_oracle_line_is_hidden(self):
        for name in self.EXPECTED:
            status, lines = fixture(name)
            summary = summarize(lines, status)
            for line in lines:
                plain = ANSI.sub("", line).rstrip()
                if ORACLE.search(plain):
                    self.assertIn(plain, summary, name + ": " + plain)

    def test_summary_is_shorter_than_the_failing_output(self):
        status, lines = fixture("fail_nextest")
        self.assertLess(len(summarize(lines, status)), len(lines))

    def test_passing_runs_are_brief_and_carry_their_result_lines(self):
        for name, needle in (("pass_cargo_test", "test result: ok."), ("pass_nextest", "tests run: 2 passed")):
            status, lines = fixture(name)
            summary = summarize(lines, status)
            self.assertTrue(any(needle in line for line in summary), name)
            self.assertLessEqual(len(summary), 6, name)
            self.assertFalse(any("FAILED" in line for line in summary), name)

    def test_unrecognised_failure_shows_the_tail(self):
        lines = ["step one", "something odd happened", "the end"]
        summary = summarize(lines, 2, tail=2)
        self.assertIn("No failure marker was recognised", summary[0])
        self.assertEqual(summary[-2:], ["something odd happened", "the end"])

    def test_zero_exit_with_failure_markers_is_flagged(self):
        summary = summarize(["test a ... FAILED", "test result: FAILED. 0 passed; 1 failed"], 0)
        self.assertIn("exited 0", summary[0])

    def test_truncation_is_counted_and_followed_by_the_tail(self):
        lines = ["test t" + str(i) + " ... FAILED" for i in range(500)] + ["final line"]
        summary = summarize(lines, 101, tail=1, limit=10)
        self.assertTrue(any("490 further marker" in line for line in summary))
        self.assertEqual(summary[-1], "final line")

    def test_failures_name_their_step(self):
        lines = ["\x1b[1m=== Clippy (workspace) ===\x1b[0m", "    Checking x", "error: unused variable"]
        summary = summarize(lines, 1)
        self.assertEqual(summary[:2], ["[=== Clippy (workspace) ===]", "error: unused variable"])


class CommandTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.log = Path(self.temp.name) / "gate.log"

    def wrap(self, *command):
        return subprocess.run([sys.executable, str(SCRIPT), "--log", str(self.log), "--", *command],
                              capture_output=True, text=True, timeout=30)

    def test_status_and_full_log_are_preserved(self):
        result = self.wrap("sh", "-c", "i=0; while [ $i -lt 300 ]; do echo noise $i; i=$((i+1)); done; "
                                       "echo 'error: boom' >&2; exit 3")
        self.assertEqual(result.returncode, 3)
        self.assertIn("gate FAILED, exit 3", result.stdout)
        self.assertIn("error: boom", result.stdout)
        self.assertNotIn("noise 5\n", result.stdout)
        self.assertEqual(len(self.log.read_text().splitlines()), 301)

    def test_success_passes_through(self):
        result = self.wrap("sh", "-c", "echo 'test result: ok. 1 passed; 0 failed'")
        self.assertEqual(result.returncode, 0)
        self.assertIn("gate passed, exit 0", result.stdout)

    def test_signal_is_reported_as_a_shell_would(self):
        result = self.wrap("sh", "-c", "kill -9 $$")
        self.assertEqual(result.returncode, 137)
        self.assertIn("killed by signal 9", result.stdout)

    def test_unstartable_command_fails_loudly(self):
        result = self.wrap(str(Path(self.temp.name) / "no-such-command"))
        self.assertEqual(result.returncode, 127)
        self.assertIn("could not start the command", result.stdout)


if __name__ == "__main__":
    unittest.main()
