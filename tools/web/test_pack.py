import tempfile
import unittest
import struct
from pathlib import Path
from pack import publish


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


if __name__ == '__main__':
    unittest.main()
