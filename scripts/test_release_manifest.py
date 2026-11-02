"""Tests of `scripts/release_manifest.py`, the checksum manifest a release signs.

The manifest is what the operator's signature vouches for, so the failure cases are
the point. Each test builds `.crate` files in a temporary directory and answers index
lookups from a dictionary, so nothing here reaches the network or a real release.
"""

from pathlib import Path
import hashlib
import io
import json
import shutil
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_manifest  # noqa: E402


def make_crate(directory, name, version, top=None):
    """Write a minimal `.crate` file and return its path and SHA-256."""
    top = top or f'{name}-{version}'
    path = Path(directory) / f'{name}-{version}.crate'
    manifest = f'[package]\nname = "{name}"\nversion = "{version}"\n'.encode()
    with tarfile.open(path, mode='w:gz') as archive:
        info = tarfile.TarInfo(f'{top}/Cargo.toml')
        info.size = len(manifest)
        archive.addfile(info, io.BytesIO(manifest))
    return path, hashlib.sha256(path.read_bytes()).hexdigest()


def index_line(name, version, cksum, yanked=False):
    return json.dumps({'name': name, 'vers': version, 'cksum': cksum, 'yanked': yanked})


class IndexPathTests(unittest.TestCase):
    def test_each_name_length_uses_the_registry_layout(self):
        cases = {'a': '1/a', 'ab': '2/ab', 'abc': '3/a/abc',
                 'Keleusma': 'ke/le/keleusma', 'keleusma-wire': 'ke/le/keleusma-wire'}
        for name, expected in cases.items():
            with self.subTest(name=name):
                self.assertEqual(release_manifest.index_path(name), expected)

    def test_an_empty_name_is_refused(self):
        with self.assertRaises(release_manifest.ManifestError):
            release_manifest.index_path('')


class ManifestTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)

    def build(self, paths, index):
        return release_manifest.build_manifest(paths, fetch=lambda name: index[name])

    def test_matching_crates_give_a_sorted_manifest(self):
        wire, wire_sum = make_crate(self.tmp, 'keleusma-wire', '0.1.4')
        core, core_sum = make_crate(self.tmp, 'keleusma', '0.2.3')
        index = {'keleusma': index_line('keleusma', '0.2.2', '0' * 64) + '\n'
                 + index_line('keleusma', '0.2.3', core_sum),
                 'keleusma-wire': index_line('keleusma-wire', '0.1.4', wire_sum)}
        text = self.build([wire, core], index)
        self.assertEqual(text, f'{core_sum}  keleusma-0.2.3.crate\n'
                               f'{wire_sum}  keleusma-wire-0.1.4.crate\n')

    def test_a_checksum_that_differs_from_the_index_fails(self):
        core, core_sum = make_crate(self.tmp, 'keleusma', '0.2.3')
        index = {'keleusma': index_line('keleusma', '0.2.3', 'f' * 64)}
        with self.assertRaisesRegex(release_manifest.ManifestError, 'crates.io records'):
            self.build([core], index)

    def test_a_version_the_index_does_not_list_fails(self):
        core, core_sum = make_crate(self.tmp, 'keleusma', '0.2.3')
        index = {'keleusma': index_line('keleusma', '0.2.2', core_sum)}
        with self.assertRaisesRegex(release_manifest.ManifestError, 'does not list'):
            self.build([core], index)

    def test_a_yanked_version_fails(self):
        core, core_sum = make_crate(self.tmp, 'keleusma', '0.2.3')
        index = {'keleusma': index_line('keleusma', '0.2.3', core_sum, yanked=True)}
        with self.assertRaisesRegex(release_manifest.ManifestError, 'yanked'):
            self.build([core], index)

    def test_a_crate_given_twice_fails(self):
        core, core_sum = make_crate(self.tmp, 'keleusma', '0.2.3')
        index = {'keleusma': index_line('keleusma', '0.2.3', core_sum)}
        with self.assertRaisesRegex(release_manifest.ManifestError, 'twice'):
            self.build([core, core], index)

    def test_a_crate_whose_directory_disagrees_with_its_manifest_fails(self):
        odd, _ = make_crate(self.tmp, 'keleusma', '0.2.3', top='keleusma-0.2.2')
        with self.assertRaisesRegex(release_manifest.ManifestError, 'unpacks to'):
            self.build([odd], {})

    def test_a_file_that_is_not_a_crate_fails(self):
        bogus = self.tmp / 'keleusma-0.2.3.crate'
        bogus.write_bytes(b'not a tarball')
        with self.assertRaisesRegex(release_manifest.ManifestError, 'not a readable crate'):
            self.build([bogus], {})

    def test_an_index_that_cannot_be_read_fails(self):
        core, _ = make_crate(self.tmp, 'keleusma', '0.2.3')

        def unreachable(name):
            raise release_manifest.ManifestError(f'the index for {name} could not be read')

        with self.assertRaisesRegex(release_manifest.ManifestError, 'could not be read'):
            release_manifest.build_manifest([core], fetch=unreachable)

    def test_no_crates_fails(self):
        with self.assertRaises(release_manifest.ManifestError):
            self.build([], {})


class MainTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)

    def test_an_existing_manifest_is_never_overwritten(self):
        core, _ = make_crate(self.tmp, 'keleusma', '0.2.3')
        output = self.tmp / 'SHA256SUMS'
        output.write_text('earlier manifest\n')
        self.assertEqual(release_manifest.main([str(core), '--output', str(output)]), 1)
        self.assertEqual(output.read_text(), 'earlier manifest\n')

    def test_a_failure_writes_nothing(self):
        bogus = self.tmp / 'keleusma-0.2.3.crate'
        bogus.write_bytes(b'not a tarball')
        output = self.tmp / 'SHA256SUMS'
        self.assertEqual(release_manifest.main([str(bogus), '--output', str(output)]), 1)
        self.assertFalse(output.exists())


if __name__ == '__main__':
    unittest.main()
