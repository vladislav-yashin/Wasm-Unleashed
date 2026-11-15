#!/usr/bin/env python3
"""Serve the web build and prepared assets locally without copying original data into dist.

With `--relay` the development server also starts `nfs-relay` and proxies `/relay` to it, so the
browser's same-origin WebSocket URL works exactly as it does behind nginx in production.
"""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import select
import signal
import socket
import subprocess
import sys
import time
from urllib.parse import unquote, urlsplit
from urllib.request import urlopen

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--dist', type=Path, default=Path('dist'))
parser.add_argument('--data', type=Path, default=Path('web-data'))
parser.add_argument('--port', type=int, default=8080)
parser.add_argument('--host', default='127.0.0.1')
parser.add_argument('--relay', action='store_true', help='start nfs-relay and proxy /relay to it')
parser.add_argument('--relay-binary', type=Path, help='path to nfs-relay (default target/debug/nfs-relay)')
parser.add_argument('--relay-port', type=int, default=0, help='relay TCP port; 0 picks a free one, and a non-zero port without --relay proxies to an existing relay')
parser.add_argument('--relay-allowed-origins', default=None, help='override the relay Origin allowlist')


class Handler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, '.wasm': 'application/wasm', '.mjs': 'text/javascript'}
    asset_root = Path('.')
    relay_port = None

    def translate_path(self, path):
        relative = unquote(urlsplit(path).path)
        if relative.startswith('/data/'):
            target = (type(self).asset_root / relative[len('/data/'):]).resolve()
            if not target.is_relative_to(type(self).asset_root):
                return str(type(self).asset_root / '.missing')
            return str(target)
        return super().translate_path(path)

    def do_GET(self):
        if urlsplit(self.path).path == '/relay':
            self.proxy_relay()
            return
        super().do_GET()

    def proxy_relay(self):
        """Tunnel one WebSocket handshake and the frames after it to the relay.

        `BaseHTTPRequestHandler` has already parsed the request line and headers, and a WebSocket
        client sends nothing else until the 101 response, so the raw connection can be forwarded
        from here on. This is the development equivalent of nginx's `proxy_pass` with an Upgrade.
        """
        port = type(self).relay_port
        if not port:
            self.send_error(503, 'the relay is not running; start serve.py with --relay')
            return
        try:
            upstream = socket.create_connection(('127.0.0.1', port), timeout=5)
        except OSError as error:
            self.send_error(502, f'relay unavailable: {error}')
            return
        upstream.settimeout(5)
        self.close_connection = True
        try:
            client_ip = self.client_address[0]
            headers = []
            for name, value in self.headers.items():
                if name.lower() in ('host', 'x-real-ip', 'x-forwarded-for'):
                    continue
                headers.append(f'{name}: {value}')
            headers.append(f'Host: 127.0.0.1:{port}')
            headers.append(f'X-Real-IP: {client_ip}')
            headers.append(f'X-Forwarded-For: {client_ip}')
            request = f'{self.command} {self.path} {self.request_version}\r\n' + '\r\n'.join(headers) + '\r\n\r\n'
            upstream.sendall(request.encode('latin-1', 'replace'))

            # The relay's answer and the first frames may arrive in the same packet; keep the tail.
            buf = b''
            while b'\r\n\r\n' not in buf:
                chunk = upstream.recv(4096)
                if not chunk:
                    break
                buf += chunk
            if b'\r\n\r\n' not in buf:
                self.send_error(502, 'relay closed the WebSocket handshake')
                return
            head, rest = buf.split(b'\r\n\r\n', 1)
            self.connection.sendall(head + b'\r\n\r\n')
            if rest:
                self.connection.sendall(rest)
            if b' 101 ' not in head.split(b'\r\n', 1)[0]:
                # A refused upgrade (origin, target policy) is forwarded as-is and the socket closes.
                return
            upstream.settimeout(None)
            self.log_message('relay proxy %s', self.path)
            sockets = (self.connection, upstream)
            while True:
                readable, _, _ = select.select(sockets, (), (), 3600)
                if not readable:
                    break
                for sock in readable:
                    data = sock.recv(65536)
                    if not data:
                        return
                    (upstream if sock is self.connection else self.connection).sendall(data)
        except (OSError, ValueError):
            pass
        finally:
            upstream.close()

    def send_head(self):
        if urlsplit(self.path).path.startswith('/data/objects/'):
            encodings = self.headers.get('Accept-Encoding', '').lower().split(',')
            accepts_gzip = any(part.strip().split(';')[0] == 'gzip'
                               and not any(param.strip() in ('q=0', 'q=0.0', 'q=0.00', 'q=0.000')
                                           for param in part.split(';')[1:])
                               for part in encodings)
            target = Path(self.translate_path(self.path))
            sidecar = target.with_name(target.name + '.gz')
            if accepts_gzip and target.is_file() and sidecar.is_file():
                stream = sidecar.open('rb')
                self.send_response(200)
                self.send_header('Content-Type', 'application/octet-stream')
                self.send_header('Content-Encoding', 'gzip')
                self.send_header('Content-Length', str(sidecar.stat().st_size))
                self.end_headers()
                return stream
        return super().send_head()

    def end_headers(self):
        if urlsplit(self.path).path.startswith('/data/objects/'):
            self.send_header('Vary', 'Accept-Encoding')
        self.send_header('Cache-Control', 'public, max-age=31536000, immutable' if urlsplit(self.path).path.startswith('/data/objects/') else 'no-cache')
        super().end_headers()


def free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(('127.0.0.1', 0))
        return sock.getsockname()[1]


def relay_binary(path):
    if path:
        return path
    name = 'nfs-relay.exe' if os.name == 'nt' else 'nfs-relay'
    return Path('target/debug') / name


def allowed_origins(host, port):
    """The origins the relay should accept for this development server.

    A wildcard bind means the page may be opened through a LAN address, which cannot be enumerated
    reliably; that case allows any origin and prints a warning. The default 127.0.0.1 bind names the
    two usual loopback spellings.
    """
    if host in ('0.0.0.0', '::', ''):
        return ''
    names = ['127.0.0.1', 'localhost']
    if host not in names and host != '::1':
        names.append(host)
    origins = [f'http://{name}:{port}' for name in names]
    if host == '::1':
        origins.append(f'http://[::1]:{port}')
    return ','.join(origins)


def start_relay(binary, port, origins):
    binary = Path(binary)
    if not binary.is_file():
        raise RuntimeError(f'{binary} is missing; run `make relay-build` or `make serve`')
    command = [str(binary), '--listen', f'127.0.0.1:{port}', '--allow-loopback', '--allow-private', '--allowed-origins', origins]
    process = subprocess.Popen(command)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f'relay exited with status {process.returncode}')
        try:
            with urlopen(f'http://127.0.0.1:{port}/healthz', timeout=0.5) as response:
                if response.status == 200:
                    return process
        except OSError:
            pass
        time.sleep(0.1)
    process.terminate()
    raise RuntimeError(f'relay did not answer /healthz on 127.0.0.1:{port}')


def main(argv=None):
    args = parser.parse_args(argv)
    Handler.asset_root = args.data.resolve()
    server = ThreadingHTTPServer((args.host, args.port), partial(Handler, directory=str(args.dist.resolve())))
    port = server.server_port
    relay = None

    if args.relay or args.relay_port:
        relay_port = args.relay_port or (free_port() if args.relay else 0)
        origins = args.relay_allowed_origins
        if origins is None:
            origins = allowed_origins(args.host, port)
        if origins == '':
            print('warning: wildcard host: the relay accepts any WebSocket origin', flush=True)
        Handler.relay_port = relay_port
        if args.relay:
            try:
                relay = start_relay(relay_binary(args.relay_binary), relay_port, origins)
            except Exception as error:
                server.server_close()
                print(f'error: {error}', file=sys.stderr, flush=True)
                return 1
        elif not relay_port:
            server.server_close()
            print('error: --relay-port is required when --relay is not set', file=sys.stderr, flush=True)
            return 1

    def stop(*_):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, stop)
    print(f'Open http://{args.host}:{port}', flush=True)
    if Handler.relay_port:
        print(f'Relay: ws://127.0.0.1:{Handler.relay_port}/relay (proxied at /relay)', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        if relay:
            relay.terminate()
            try:
                relay.wait(timeout=5)
            except subprocess.TimeoutExpired:
                relay.kill()
                relay.wait()
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
