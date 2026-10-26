"""Guards on the local tooling that the documentation tells agents to run directly.

Both defects these tests pin were reported by the V0.2.X line on 2026-10-25. The
`cargo tns` alias quoted its filterset, which cargo passes through, so nextest refused
it. `scripts/gate_summary.py` carried a shebang without the executable bit, so the
documented `scripts/gate_summary.py -- ...` form exited 126. Neither is visible to CI,
which never runs an alias and always calls Python scripts through `python3`.
"""

from pathlib import Path
import subprocess
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CargoAliasTests(unittest.TestCase):
    def aliases(self):
        return tomllib.loads((ROOT / '.cargo/config.toml').read_text())['alias']

    def test_string_aliases_carry_no_quotes(self):
        # Cargo splits a string alias on whitespace and does not interpret quotes, so
        # any quote reaches the subcommand verbatim.
        aliases = self.aliases()
        self.assertIn('tns', aliases)
        for name, value in aliases.items():
            with self.subTest(alias=name):
                self.assertIsInstance(value, str, 'a list alias cannot merge with the string '
                                      'alias of a parent checkout, see .cargo/config.toml')
                self.assertNotIn("'", value)
                self.assertNotIn('"', value)

    def test_the_self_host_tier_selects_the_selfhost_binaries(self):
        self.assertEqual(self.aliases()['tns'].split(),
                         ['nextest', 'run', '--workspace', '-E', 'binary(~selfhost)'])


class ScriptModeTests(unittest.TestCase):
    def test_every_tracked_script_with_a_shebang_is_executable(self):
        listing = subprocess.run(['git', '-C', str(ROOT), 'ls-files', '-s', '--', 'scripts'],
                                 check=True, capture_output=True, text=True).stdout.splitlines()
        checked = 0
        for line in listing:
            mode, _, _, path = line.split(maxsplit=3)
            if mode == '120000':
                continue
            with (ROOT / path).open('rb') as stream:
                if stream.read(2) != b'#!':
                    continue
            checked += 1
            with self.subTest(path=path):
                self.assertEqual(mode, '100755')
        # A collapsed count would mean the listing or the shebang test stopped matching.
        self.assertGreaterEqual(checked, 20)


if __name__ == '__main__':
    unittest.main()
