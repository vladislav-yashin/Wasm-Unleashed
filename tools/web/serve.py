#!/usr/bin/env python3
"""Serve the web build and prepared assets locally without copying original data into dist."""
import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlsplit

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--dist', type=Path, default=Path('dist'))
parser.add_argument('--data', type=Path, default=Path('web-data'))
parser.add_argument('--port', type=int, default=8080)
parser.add_argument('--host', default='127.0.0.1')
args = parser.parse_args()
asset_root = args.data.resolve()

class Handler(SimpleHTTPRequestHandler):
    extensions_map = {**SimpleHTTPRequestHandler.extensions_map, '.wasm':'application/wasm', '.mjs':'text/javascript'}
    def translate_path(self, path):
        relative = unquote(urlsplit(path).path)
        if relative.startswith('/data/'):
            target = (asset_root / relative[len('/data/'):]).resolve()
            if not target.is_relative_to(asset_root):
                return str(asset_root / '.missing')
            return str(target)
        return super().translate_path(path)
    def end_headers(self):
        self.send_header('Cache-Control', 'public, max-age=31536000, immutable' if urlsplit(self.path).path.startswith('/data/objects/') else 'no-cache')
        super().end_headers()

print(f'Open http://{args.host}:{args.port}', flush=True)
ThreadingHTTPServer((args.host, args.port), partial(Handler, directory=str(args.dist.resolve()))).serve_forever()
