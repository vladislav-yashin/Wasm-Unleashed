#!/usr/bin/env python3
"""Build one pack, upload its immutable objects, then atomically update the server catalog.

Requires SSH, rsync, Python 3 and write access to the server's mounted data directory.
"""
import argparse
import json
from pathlib import Path
import shlex
import subprocess
import tempfile

from mod_pack import build, register
import inspect


def deploy(source, host, port, remote):
    # Isolate this pack so deployment never uploads the local original data set.
    with tempfile.TemporaryDirectory(prefix='nfs-mod-') as directory:
        output = Path(directory)
        entry = build(source, output)
        ssh = ['ssh', '-p', str(port), host]
        subprocess.run([*ssh, 'mkdir -p -- ' + shlex.quote(remote)], check=True)
        subprocess.run(['rsync', '-rt', '--protect-args', '-e', f'ssh -p {port}',
                        str(output / 'objects'), str(output / 'mods'), f'{host}:{remote}/'], check=True)
        # Serialize catalog updates from concurrent deploys. Install the catalog last so
        # readers always see complete objects; existing sessions retain their old hashes.
        script = '''import fcntl, json, os, pathlib, sys, tempfile
''' + inspect.getsource(register) + '''
root = pathlib.Path(sys.argv[1])
with (root / '.mods.lock').open('a') as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    path = root / 'mods.json'
    catalog = json.loads(path.read_text(encoding='utf-8')) if path.exists() else {'schema': 1, 'packs': []}
    register(catalog, json.load(sys.stdin))
    fd, name = tempfile.mkstemp(prefix='.mods-', dir=root)
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as stream:
            json.dump(catalog, stream, ensure_ascii=False, indent=2)
        os.chmod(name, 0o644)
        os.replace(name, path)
    finally:
        if os.path.exists(name): os.unlink(name)
'''
        command = 'python3 -c ' + shlex.quote(script) + ' ' + shlex.quote(remote)
        subprocess.run([*ssh, command], input=json.dumps(entry), text=True, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--host', required=True)
    parser.add_argument('--port', type=int, default=22)
    parser.add_argument('--remote-data', default='/srv/open-unleashed/web-data')
    args = parser.parse_args()
    if not 1 <= args.port <= 65535 or args.host.startswith('-') or not args.remote_data.startswith('/'):
        parser.error('expected an SSH host, a valid port and an absolute remote data path')
    deploy(args.source, args.host, args.port, args.remote_data)


if __name__ == '__main__':
    main()
