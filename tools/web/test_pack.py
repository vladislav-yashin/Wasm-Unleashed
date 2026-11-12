import tempfile
import unittest
import struct
from pathlib import Path
from pack import prepare_from_installation, publish, stage_installation


class PublishTests(unittest.TestCase):
    def test_stable_manifest_and_content(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData').mkdir(parents=True)
            (source / 'FEData').mkdir()
            (source / 'FEData' / 'MENU.bin').write_bytes(b'menu')
            first = publish(source, base / 'out')
            self.assertEqual(first, publish(source, base / 'out'))
            entry = first['files']['fedata/menu.bin']
            self.assertEqual((base / 'out' / entry['url']).read_bytes(), b'menu')
            (source / 'FEData' / 'menu.BIN').write_bytes(b'other')
            with self.assertRaisesRegex(ValueError, 'collision'):
                publish(source, base / 'out')

    def test_root_configuration_and_object_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData').mkdir(parents=True)
            (source / 'FEData').mkdir()
            (source / 'fe.txt').write_bytes(b'language=english')
            manifest = publish(source, base / 'out')
            entry = manifest['files']['fe.txt']
            (base / 'out' / entry['url']).write_bytes(b'bad')
            with self.assertRaisesRegex(ValueError, 'integrity'):
                publish(source, base / 'out')

    def test_music_archive_has_independent_song_objects(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData/Music').mkdir(parents=True)
            (source / 'FEData').mkdir()
            songs = [('menu', b'a'*1500), ('game', b'b'*1800)]
            header_size = 16 + sum(8+len(name)+1 for name, _ in songs)
            offset, table = header_size, bytearray()
            for name, data in songs:
                table.extend(struct.pack('>II', offset, len(data)) + name.encode() + b'\0')
                offset += len(data)
            archive = b'BIGF' + struct.pack('>III', offset, len(songs), header_size) + table + b''.join(data for _, data in songs)
            (source / 'GameData/Music/zzzymus.viv').write_bytes(archive)
            manifest = publish(source, base / 'out')
            entry = manifest['files']['gamedata/music/zzzymus.viv']
            self.assertNotIn('url', entry)
            rebuilt = bytearray(entry['size'])
            for segment in entry['segments']:
                data = (base / 'out' / segment['url']).read_bytes()
                rebuilt[segment['offset']:segment['offset']+len(data)] = data
                self.assertLessEqual(segment['head']['size'], max(1024, header_size))
            self.assertEqual(rebuilt, archive)

    def test_extra_files_are_published_without_modifying_installation(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData').mkdir(parents=True)
            (source / 'FEData').mkdir()
            tables = base / 'madtables.bin'
            tables.write_bytes(b'tables')
            manifest = publish(source, base / 'out', [tables])
            entry = manifest['files']['madtables.bin']
            self.assertEqual((base / 'out' / entry['url']).read_bytes(), b'tables')
            self.assertFalse((source / 'madtables.bin').exists())

    def test_reject_nested_output(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / 'GameData').mkdir()
            (source / 'FEData').mkdir()
            with self.assertRaises(ValueError):
                publish(source, source / 'output')

    def test_from_installation_stages_only_consumed_files(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData/Track').mkdir(parents=True)
            (source / 'GameData/Track/alps.bin').write_bytes(b'track')
            (source / 'GameData/junk.xls').write_bytes(b'xls')
            (source / 'GameData/readme.H').write_bytes(b'header')
            (source / 'FEData/Data').mkdir(parents=True)
            (source / 'FEData/Data/nfs5.car').write_bytes(b'car')
            (source / 'FEData/Movies').mkdir()
            (source / 'FEData/Movies/intro.mad').write_bytes(b'mad')
            (source / 'Backup').mkdir()
            (source / 'Backup/old.bin').write_bytes(b'old')
            for name, data in (('fe.txt', b'fe'), ('version.txt', b'3.5'), ('nfs5.ini', b'[NFS5]')):
                (source / name).write_bytes(data)
            (source / 'junk.dll').write_bytes(b'dll')
            for name in ('nfs5.exe', 'Porsche.exe', 'PORSCHE.ICD'):
                (source / name).write_bytes(name.encode())

            calls = []
            def extractor(exe, out):
                calls.append(exe.name.casefold())
                if exe.name.casefold() == 'nfs5.exe':
                    raise RuntimeError('incompatible tables')
                out.write_bytes(b'tables')

            stage = base / 'stage'
            chosen = stage_installation(source, stage, extractor=extractor)

            self.assertEqual(calls, ['nfs5.exe', 'porsche.exe'])
            self.assertEqual(chosen.name.casefold(), 'porsche.exe')
            self.assertTrue((stage / 'GameData/Track/alps.bin').is_file())
            self.assertFalse((stage / 'GameData/junk.xls').exists())
            self.assertFalse((stage / 'GameData/readme.H').exists())
            self.assertTrue((stage / 'FEData/Data/nfs5.car').is_file())
            self.assertTrue((stage / 'FEData/Movies/intro.mad').is_file())
            self.assertEqual((stage / 'madtables.bin').read_bytes(), b'tables')
            for name in ('fe.txt', 'version.txt', 'nfs5.ini'):
                self.assertTrue((stage / name).is_file())
            for name in ('junk.dll', 'nfs5.exe', 'Porsche.exe', 'PORSCHE.ICD'):
                self.assertFalse((stage / name).exists())
            self.assertFalse((stage / 'Backup').exists())

    def test_explicit_movie_executable_skips_auto_detection(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData').mkdir(parents=True)
            (source / 'FEData').mkdir()
            (source / 'nfs5.exe').write_bytes(b'bad')
            (source / 'PORSCHE.ICD').write_bytes(b'chosen')

            calls = []
            def extractor(exe, out):
                calls.append(exe.name.casefold())
                out.write_bytes(b'tables')

            chosen = stage_installation(
                source, base / 'stage', movie_exe=source / 'PORSCHE.ICD', extractor=extractor)
            self.assertEqual(calls, ['porsche.icd'])
            self.assertEqual(chosen.name.casefold(), 'porsche.icd')
            self.assertEqual((base / 'stage/madtables.bin').read_bytes(), b'tables')

    def test_from_installation_requires_a_movie_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData').mkdir(parents=True)
            (source / 'FEData').mkdir()
            with self.assertRaisesRegex(ValueError, 'movie executable not found'):
                stage_installation(source, base / 'stage')

    def test_prepare_from_installation_drops_unconsumed_root_files(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            source = base / 'game'
            (source / 'GameData').mkdir(parents=True)
            (source / 'GameData/menu.bin').write_bytes(b'menu')
            (source / 'FEData').mkdir()
            (source / 'FEData/menu.bin').write_bytes(b'menu')
            (source / 'Porsche.exe').write_bytes(b'exe')
            (source / 'junk.dll').write_bytes(b'dll')
            (source / 'fe.txt').write_bytes(b'fe')
            (source / 'version.txt').write_bytes(b'3.5')
            (source / 'nfs5.ini').write_bytes(b'[NFS5]')
            (source / 'Backup').mkdir()
            (source / 'Backup/old.bin').write_bytes(b'old')

            def extractor(_exe, out):
                out.write_bytes(b'tables')

            manifest = prepare_from_installation(source, base / 'out', extractor=extractor)
            files = set(manifest['files'])
            for name in ('gamedata/menu.bin', 'fedata/menu.bin', 'madtables.bin',
                         'fe.txt', 'version.txt', 'nfs5.ini'):
                self.assertIn(name, files)
            for name in ('porsche.exe', 'junk.dll', 'backup/old.bin'):
                self.assertNotIn(name, files)
            self.assertTrue((base / 'out/manifest.json').is_file())
            self.assertFalse(any(base.glob('.nfs-web-stage-*')))


if __name__ == '__main__':
    unittest.main()
