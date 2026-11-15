import gzip
from http.client import HTTPConnection
from pathlib import Path
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import serve


class FakeRelay(socketserver.BaseRequestHandler):
    """One WebSocket upgrade, then echo every byte. Enough to prove the proxy is transparent."""
    def handle(self):
        head = b''
        while b'\r\n\r\n' not in head:
            chunk = self.request.recv(4096)
            if not chunk:
                return
            head += chunk
        self.request.sendall(b'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n')
        while True:
            data = self.request.recv(4096)
            if not data:
                return
            self.request.sendall(data)


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

    def test_relay_proxy_forwards_the_upgrade_and_the_tunnel(self):
        upstream = socketserver.ThreadingTCPServer(('127.0.0.1', 0), FakeRelay)
        upstream.daemon_threads = True
        relay_port = upstream.server_address[1]
        thread = threading.Thread(target=upstream.serve_forever, daemon=True)
        thread.start()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            process = subprocess.Popen([
                sys.executable, str(Path(__file__).with_name('serve.py')),
                '--port', '0', '--data', str(root), '--dist', str(root),
                '--relay-port', str(relay_port),
            ], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
            try:
                url = process.stdout.readline().strip().split()[-1]
                port = int(url.rsplit(':', 1)[1])
                with socket.create_connection(('127.0.0.1', port), timeout=5) as client:
                    client.sendall((
                        'GET /relay?target=127.0.0.1:9442 HTTP/1.1\r\n'
                        f'Host: 127.0.0.1:{port}\r\n'
                        f'Origin: http://127.0.0.1:{port}\r\n'
                        'Upgrade: websocket\r\n'
                        'Connection: Upgrade\r\n'
                        'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n'
                        'Sec-WebSocket-Version: 13\r\n\r\n'
                    ).encode())
                    head = b''
                    while b'\r\n\r\n' not in head:
                        chunk = client.recv(4096)
                        self.assertTrue(chunk, 'the proxy closed the handshake')
                        head += chunk
                    self.assertIn(b' 101 ', head.split(b'\r\n', 1)[0])
                    client.sendall(b'game frame')
                    echoed = b''
                    while len(echoed) < len(b'game frame'):
                        echoed += client.recv(4096)
                    self.assertEqual(echoed, b'game frame')
            finally:
                process.terminate()
                process.wait(timeout=5)
                process.stdout.close()
        upstream.shutdown()
        upstream.server_close()

    def test_relay_origins_follow_the_bind(self):
        self.assertEqual(serve.allowed_origins('127.0.0.1', 8080),
                         'http://127.0.0.1:8080,http://localhost:8080')
        self.assertEqual(serve.allowed_origins('0.0.0.0', 8080), '')
        self.assertIn('http://192.168.1.5:8080', serve.allowed_origins('192.168.1.5', 8080))


if __name__ == '__main__':
    unittest.main()
