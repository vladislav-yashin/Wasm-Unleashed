#!/usr/bin/env python3
"""Publish original installation files as immutable, independently fetched assets."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile


def publish(source: Path, output: Path, extra_files=()):
    if not all((source / name).is_dir() for name in ('GameData', 'FEData')):
        raise ValueError('source must contain GameData and FEData')
    source, output = source.resolve(), output.resolve()
    if source == output or source in output.parents:
        raise ValueError('output must be outside the installation')
    files = {}
    objects = {}
    def object_entry(data):
        digest = hashlib.sha256(data).hexdigest()
        objects[digest] = data
        return {'url': f'objects/{digest}', 'sha256': digest, 'size': len(data)}

    origins = {}
    extra_files = [Path(p).resolve() for p in extra_files]
    paths = [p for root in ('GameData', 'FEData') for p in (source / root).rglob('*')]
    # Root configuration, movie tables and fallback executable data are consumed by parsers.
    paths += [p for p in source.iterdir() if p.is_file()]
    paths += extra_files
    for path in sorted(set(paths)):
        if not path.is_file():
            continue
        if path.is_symlink() or (source not in path.resolve().parents and path not in extra_files):
            raise ValueError(f'asset escapes installation: {path}')
        relative = path.name if path in extra_files else path.relative_to(source).as_posix()
        origins[relative] = path
        key = relative.casefold()
        if key in files:
            raise ValueError(f'case-insensitive collision: {relative}')
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        parts = relative.split('/')
        group = '/'.join(parts[:2]).casefold()
        if len(parts) > 2 and parts[1].casefold() in ('track', 'carmodel', 'sounds'):
            group += '/' + Path(parts[2]).stem.casefold()
        files[key] = {'path': relative, 'url': f'objects/{digest}',
                      'sha256': digest, 'size': path.stat().st_size, 'group': group}
        if key == 'gamedata/music/zzzymus.viv':
            data = path.read_bytes()
            if data[:4] != b'BIGF' or len(data) < 16:
                raise ValueError('music archive is not BIGF')
            count, header_size = struct.unpack_from('>II', data, 8)
            if count > len(data) // 9 or not 16 <= header_size <= len(data):
                raise ValueError('invalid music directory')
            position, offsets = 16, []
            for _ in range(count):
                offset, size = struct.unpack_from('>II', data, position)
                position = data.index(0, position + 8, header_size) + 1
                if offset < header_size or offset + size > len(data):
                    raise ValueError('music entry outside archive')
                offsets.append(offset)
            boundaries = sorted(set([0, *offsets, len(data)]))
            segments = []
            for begin, end in zip(boundaries, boundaries[1:]):
                segment = object_entry(data[begin:end])
                segment['offset'] = begin
                segment['head'] = object_entry(data[begin:end if begin == 0 else min(end, begin + 1024)])
                segments.append(segment)
            files[key]['segments'] = segments
            del files[key]['url']

    version = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    (output / 'objects').mkdir(parents=True, exist_ok=True)
    for entry in files.values():
        if 'segments' in entry:
            continue
        target = output / entry['url']
        if not target.exists():
            shutil.copyfile(origins[entry['path']], target)
        if hashlib.sha256(target.read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError(f'object integrity failure: {target}')
    for digest, data in objects.items():
        target = output / 'objects' / digest
        if not target.exists():
            target.write_bytes(data)
        if hashlib.sha256(target.read_bytes()).hexdigest() != digest:
            raise ValueError(f'object integrity failure: {target}')
    manifest = {'schema': 1, 'version': version, 'files': files}
    temporary = output / 'manifest.json.tmp'
    temporary.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    temporary.replace(output / 'manifest.json')
    return manifest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--extra-file', type=Path, action='append', default=[], help='Additional root configuration or derived table file')
    parser.add_argument('--movie-exe', type=Path, help='Original game PE image (nfs5.exe or compatible PORSCHE.ICD) used only to extract movie tables')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='nfs-web-tables-') as temporary:
        extras = list(args.extra_file)
        if args.movie_exe:
            tables = Path(temporary) / 'madtables.bin'
            subprocess.run(['cargo', '+stable', 'run', '--locked', '-p', 'nfs-formats', '--example', 'extract_mad_tables', '--', str(args.movie_exe.resolve()), str(tables)], cwd=Path(__file__).resolve().parents[2], check=True)
            extras.append(tables)
        publish(args.source, args.output, extras)
