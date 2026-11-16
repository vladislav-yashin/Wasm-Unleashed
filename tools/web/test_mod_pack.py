import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from mod_pack import build
from deploy_mod import deploy


class ModPackTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'mod'
        (self.source / 'gamedata').mkdir(parents=True)
        (self.source / 'gamedata/car.bin').write_bytes(b'mod car')
        self.config = dict(id='cars', name='Cars', description='Replacement cars', enabled_by_default=True)
        self.write_config()
        self.output = self.root / 'data'

    def write_config(self):
        (self.source / 'mod.json').write_text(json.dumps(self.config), encoding='utf-8')

    def test_overlay_preserves_base_and_updates_catalog_in_place(self):
        self.output.mkdir()
        (self.output / 'manifest.json').write_text('original manifest')
        first = build(self.source, self.output)
        self.assertEqual(first, build(self.source, self.output))
        manifest = json.loads((self.output / first['manifest']).read_text())
        self.assertEqual(list(manifest['files']), ['gamedata/car.bin'])
        resource = manifest['files']['gamedata/car.bin']
        self.assertEqual((self.output / resource['url']).read_bytes(), b'mod car')
        self.assertEqual((self.output / 'manifest.json').read_text(), 'original manifest')
        self.config['id'] = 'second'
        self.write_config()
        build(self.source, self.output)
        self.config['id'] = 'cars'
        self.config['name'] = 'New name'
        self.write_config()
        (self.source / 'gamedata/car.bin').write_bytes(b'new car')
        updated = build(self.source, self.output)
        catalog = json.loads((self.output / 'mods.json').read_text())
        self.assertEqual([pack['id'] for pack in catalog['packs']], ['cars', 'second'])
        self.assertEqual(catalog['packs'][0]['name'], 'New name')
        self.assertNotEqual(first['manifest'], updated['manifest'])
        self.assertTrue((self.output / first['manifest']).exists())

    def test_invalid_config_collisions_and_escaping_symlinks(self):
        for key, value in [('id', '../bad'), ('name', ''), ('description', []), ('enabled_by_default', 'false')]:
            with self.subTest(key=key):
                config = self.config.copy()
                config[key] = value
                (self.source / 'mod.json').write_text(json.dumps(config))
                with self.assertRaises(ValueError):
                    build(self.source, self.output)
        self.write_config()
        collision = self.source / 'gamedata/CAR.bin'
        collision.write_bytes(b'collision')
        with self.assertRaisesRegex(ValueError, 'collision'):
            build(self.source, self.output)
        collision.unlink()
        outside = self.root / 'private'
        outside.write_bytes(b'private')
        (self.source / 'gamedata/link').symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'escapes'):
            build(self.source, self.output)

    def test_deployment_publishes_catalog_last_and_preserves_other_packs(self):
        remote = self.root / 'remote'
        remote.mkdir()
        other = {'id': 'other'}
        (remote / 'mods.json').write_text(json.dumps({'schema': 1, 'packs': [other]}))
        import subprocess
        import shlex
        calls = []
        run = subprocess.run

        def execute(command, **kwargs):
            calls.append(command)
            if command[0] == 'rsync':
                # The upload is isolated from the original data and catalog.
                self.assertEqual([Path(p).name for p in command[-3:-1]], ['objects', 'mods'])
                self.assertFalse((Path(command[-3]).parent / 'manifest.json').exists())
                return
            if command[-1].startswith('python3 -c '):
                return run(shlex.split(command[-1]), **kwargs)

        with patch('deploy_mod.subprocess.run', side_effect=execute):
            deploy(self.source, 'example.test', 2222, str(remote))
        self.assertEqual(calls[1][0], 'rsync')
        catalog = json.loads((remote / 'mods.json').read_text())
        self.assertEqual([p['id'] for p in catalog['packs']], ['other', 'cars'])
        self.assertEqual((remote / 'mods.json').stat().st_mode & 0o777, 0o644)
