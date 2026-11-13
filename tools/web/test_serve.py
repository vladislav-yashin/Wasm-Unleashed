import gzip
from http.client import HTTPConnection
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class ServeTests(unittest.TestCase):
    def test_object_encoding_negotiation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'objects').mkdir()
            data = b'game asset' * 1000
            (root / 'objects/hash').write_bytes(data)
            (root / 'objects/hash.gz').write_bytes(gzip.compress(data, mtime=0))
            process = subprocess.Popen([
                sys.executable, str(Path(__file__).with_name('serve.py')),
                '--port', '0', '--data', str(root), '--dist', str(root),
            ], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
            try:
                url = process.stdout.readline().strip().split()[-1]
                port = int(url.rsplit(':', 1)[1])
                for encoding, compressed in [('gzip', True), ('br, gzip;q=1', True),
                                              ('gzip;q=0', False), ('identity', False)]:
                    connection = HTTPConnection('127.0.0.1', port, timeout=5)
                    try:
                        connection.request('GET', '/data/objects/hash', headers={'Accept-Encoding': encoding})
                        response = connection.getresponse()
                        self.assertEqual(response.status, 200)
                        self.assertEqual(response.getheader('Vary'), 'Accept-Encoding')
                        self.assertEqual(response.getheader('Content-Encoding'), 'gzip' if compressed else None)
                        body = response.read()
                        self.assertEqual(gzip.decompress(body) if compressed else body, data)
                    finally:
                        connection.close()
            finally:
                process.terminate()
                process.wait(timeout=5)
                process.stdout.close()
