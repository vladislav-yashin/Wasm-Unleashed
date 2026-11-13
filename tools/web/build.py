#!/usr/bin/env python3
"""Build the browser distribution. Requires Rust WASM target and matching wasm-bindgen CLI."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tomllib

from build_landing import build as build_landing

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--debug', action='store_true')
parser.add_argument('--site-url', default=os.environ.get('SITE_URL'), help='Public base URL for canonical and sitemap')
parser.add_argument('--wasm-bindgen', default='wasm-bindgen')
parser.add_argument('--toolchain', default='stable')
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
lock = tomllib.loads((root / 'Cargo.lock').read_text())
version = next(p['version'] for p in lock['package'] if p['name'] == 'wasm-bindgen')
installed = subprocess.check_output([args.wasm_bindgen, '--version'], text=True).strip().split()[-1]
if installed != version:
    raise SystemExit(f'Install matching CLI: cargo install wasm-bindgen-cli --version {version} --locked')
command = ['cargo', f'+{args.toolchain}', 'build', '--locked', '-p', 'open-unleashed', '--target', 'wasm32-unknown-unknown']
if not args.debug:
    command.append('--release')
subprocess.run(command, cwd=root, check=True)
output = root / 'dist'
output.mkdir(exist_ok=True)
build_landing(output, args.site_url)
for name in ['smoke.html', 'style.css', 'bootstrap.mjs', 'startup-assets.mjs', 'controls.mjs', 'fullscreen.mjs', 'render-scale.mjs', 'exit-game.mjs', 'assets.mjs', 'vfs.mjs', 'i18n.mjs']:
    shutil.copyfile(root / 'packaging/web' / name, output / name)
subprocess.run([args.wasm_bindgen, '--target', 'web', '--out-dir', str(output / 'pkg'),
                '--out-name', 'open_unleashed', str(root / 'target/wasm32-unknown-unknown' / ('debug' if args.debug else 'release') / 'open-unleashed.wasm')], check=True)
print(f'Web distribution: {output}')
