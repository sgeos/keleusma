"""Tests of the changelog fragment checker and assembler, including its fail-closed paths."""

import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

import changelog_fragments as cf

CHANGELOG = """# Changelog

Intro.

## [Unreleased]

### Fixed

- Old fix.

### Added

- Old feature.

## [0.2.2] - 2026-07-09

### Fixed

- Released fix.
"""


def fragment(date, slug, body):
    return (date, slug, cf.parse_name(f'{date}-{slug}.fixed.md')[2], body, None)


class NameTests(unittest.TestCase):
    def test_a_well_formed_name_parses(self):
        self.assertEqual(cf.parse_name('2026-10-29-verify-targets.fixed.md'),
                         ('2026-10-29', 'verify-targets', 'Fixed'))
        self.assertEqual(cf.parse_name('2026-10-29-x.security.md')[2], 'Security')

    def test_malformed_names_are_refused(self):
        for name in ('fix.md', '2026-10-29-Upper.fixed.md', '2026-10-29-a.fixed.txt',
                     '2026-10-29-a.fix.md', '2026-10-29-a..fixed.md', '2026-10-29--a.fixed.md',
                     '26-10-29-a.fixed.md', '2026-10-29-a.unreleased.md'):
            with self.subTest(name=name), self.assertRaises(cf.FragmentError):
                cf.parse_name(name)

    def test_an_impossible_date_is_refused(self):
        with self.assertRaisesRegex(cf.FragmentError, 'calendar date'):
            cf.parse_name('2026-02-30-a.fixed.md')


class BodyTests(unittest.TestCase):
    def test_surrounding_blank_lines_are_trimmed_and_crlf_normalised(self):
        self.assertEqual(cf.parse_body('f', '\r\n- one\r\n  two\r\n\r\n'), '- one\n  two')

    def test_an_empty_body_is_refused(self):
        with self.assertRaisesRegex(cf.FragmentError, 'empty'):
            cf.parse_body('f', '\n \n')

    def test_a_body_that_is_not_a_list_item_is_refused(self):
        with self.assertRaisesRegex(cf.FragmentError, 'list item'):
            cf.parse_body('f', 'Prose.\n')

    def test_a_heading_anywhere_is_refused(self):
        with self.assertRaisesRegex(cf.FragmentError, 'line 2 is a heading'):
            cf.parse_body('f', '- one\n### Fixed\n')


class AssembleTests(unittest.TestCase):
    def test_fragments_go_above_existing_entries_newest_first_then_by_slug(self):
        loaded = [fragment('2026-10-01', 'b', '- B.'), fragment('2026-10-02', 'z', '- Z.'),
                  fragment('2026-10-01', 'a', '- A.')]
        out = cf.assemble_text(CHANGELOG, loaded)
        self.assertIn('### Fixed\n\n- Z.\n\n- A.\n\n- B.\n\n- Old fix.\n\n### Added', out)
        # The released section is untouched.
        self.assertTrue(out.endswith('## [0.2.2] - 2026-07-09\n\n### Fixed\n\n- Released fix.\n'))

    def test_the_result_does_not_depend_on_listing_order(self):
        loaded = [fragment('2026-10-01', 'b', '- B.'), fragment('2026-10-01', 'a', '- A.')]
        self.assertEqual(cf.assemble_text(CHANGELOG, loaded),
                         cf.assemble_text(CHANGELOG, list(reversed(loaded))))

    def test_a_missing_section_is_created_at_the_end_of_the_unreleased_block(self):
        loaded = [('2026-10-01', 'sec', 'Security', '- S.', None)]
        out = cf.assemble_text(CHANGELOG, loaded)
        self.assertIn('- Old feature.\n\n### Security\n\n- S.\n\n## [0.2.2]', out)

    def test_only_the_first_heading_of_a_repeated_section_receives_entries(self):
        text = CHANGELOG.replace('### Added\n\n- Old feature.\n',
                                 '### Added\n\n- Old feature.\n\n### Fixed\n\n- Second fix.\n')
        out = cf.assemble_text(text, [fragment('2026-10-01', 'n', '- New.')])
        self.assertIn('### Fixed\n\n- New.\n\n- Old fix.', out)
        self.assertIn('### Fixed\n\n- Second fix.\n', out)

    def test_a_heading_with_no_blank_line_or_no_entries_still_gets_one_blank_line(self):
        tight = CHANGELOG.replace('### Fixed\n\n- Old fix.', '### Fixed\n- Old fix.')
        self.assertIn('### Fixed\n\n- New.\n\n- Old fix.',
                      cf.assemble_text(tight, [fragment('2026-10-01', 'n', '- New.')]))
        empty = CHANGELOG.replace('### Fixed\n\n- Old fix.\n\n', '### Fixed\n\n')
        self.assertIn('### Fixed\n\n- New.\n\n### Added',
                      cf.assemble_text(empty, [fragment('2026-10-01', 'n', '- New.')]))

    def test_no_fragments_leave_the_changelog_byte_identical(self):
        self.assertEqual(cf.assemble_text(CHANGELOG, []), CHANGELOG)

    def test_a_changelog_without_exactly_one_unreleased_heading_is_refused(self):
        for text in (CHANGELOG.replace('## [Unreleased]', '## [0.2.3]'),
                     CHANGELOG + '\n## [Unreleased]\n'):
            with self.subTest(text=text[-20:]), self.assertRaisesRegex(cf.FragmentError, 'exactly one'):
                cf.assemble_text(text, [])

    def test_the_preview_is_only_the_unreleased_block(self):
        section = cf.unreleased_section(CHANGELOG)
        self.assertTrue(section.startswith('## [Unreleased]\n'))
        self.assertNotIn('0.2.2', section)


class DirectoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.changelog = self.root / 'CHANGELOG.md'
        self.changelog.write_text(CHANGELOG, encoding='utf-8')
        self.fragments = self.root / 'changelog.d'
        self.fragments.mkdir()
        (self.fragments / 'README.md').write_text('# Not a fragment\n', encoding='utf-8')

    def tearDown(self):
        self.temporary.cleanup()

    def run_main(self, command):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = cf.main([command, '--changelog', str(self.changelog),
                            '--fragments', str(self.fragments)])
        return code, out.getvalue(), err.getvalue()

    def test_check_counts_fragments_and_ignores_the_readme(self):
        (self.fragments / '2026-10-29-a.fixed.md').write_text('- A.\n', encoding='utf-8')
        self.assertEqual(self.run_main('check'), (0, '1 changelog fragments are valid\n', ''))

    def test_an_unrecognised_file_or_directory_fails_closed(self):
        for name, make in (('notes.txt', lambda p: p.write_text('x', encoding='utf-8')),
                           ('sub', lambda p: p.mkdir())):
            with self.subTest(name=name):
                make(self.fragments / name)
                code, _, err = self.run_main('check')
                self.assertEqual(code, 1)
                self.assertIn(name, err)
                path = self.fragments / name
                path.rmdir() if path.is_dir() else path.unlink()

    def test_a_non_utf8_fragment_fails_closed(self):
        (self.fragments / '2026-10-29-a.fixed.md').write_bytes(b'- \xff\n')
        code, _, err = self.run_main('check')
        self.assertEqual(code, 1)
        self.assertIn('not UTF-8', err)

    def test_a_missing_directory_fails_closed(self):
        self.fragments = self.root / 'absent'
        self.assertEqual(self.run_main('check')[0], 1)

    def test_preview_writes_nothing(self):
        (self.fragments / '2026-10-29-a.fixed.md').write_text('- A.\n', encoding='utf-8')
        code, out, _ = self.run_main('preview')
        self.assertEqual(code, 0)
        self.assertIn('### Fixed\n\n- A.\n\n- Old fix.', out)
        self.assertEqual(self.changelog.read_text(encoding='utf-8'), CHANGELOG)
        self.assertTrue((self.fragments / '2026-10-29-a.fixed.md').exists())

    def test_assemble_writes_the_changelog_and_removes_only_the_fragments(self):
        (self.fragments / '2026-10-29-a.fixed.md').write_text('- A.\n', encoding='utf-8')
        code, out, _ = self.run_main('assemble')
        self.assertEqual((code, out), (0, 'assembled 1 changelog fragments into CHANGELOG.md\n'))
        self.assertIn('### Fixed\n\n- A.\n\n- Old fix.', self.changelog.read_text(encoding='utf-8'))
        self.assertEqual(sorted(p.name for p in self.fragments.iterdir()), ['README.md'])
        # A second run has nothing left to add.
        before = self.changelog.read_text(encoding='utf-8')
        self.assertEqual(self.run_main('assemble')[0], 0)
        self.assertEqual(self.changelog.read_text(encoding='utf-8'), before)

    def test_a_malformed_fragment_stops_assembly_before_anything_is_written(self):
        (self.fragments / '2026-10-29-a.fixed.md').write_text('- A.\n', encoding='utf-8')
        (self.fragments / '2026-10-29-b.fixed.md').write_text('# Heading\n', encoding='utf-8')
        self.assertEqual(self.run_main('assemble')[0], 1)
        self.assertEqual(self.changelog.read_text(encoding='utf-8'), CHANGELOG)
        self.assertTrue((self.fragments / '2026-10-29-a.fixed.md').exists())


class RepositoryTests(unittest.TestCase):
    def test_the_repository_fragments_are_valid(self):
        loaded = cf.load(cf.DEFAULT_FRAGMENTS)
        cf.assemble_text(cf.DEFAULT_CHANGELOG.read_text(encoding='utf-8'), loaded)


if __name__ == '__main__':
    unittest.main()
