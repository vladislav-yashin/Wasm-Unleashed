#!/usr/bin/env python3
"""Publish original installation files as immutable, independently fetched assets."""
import argparse
import fnmatch
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

# Files inside GameData/FEData that no crate parses.
UNNEEDED_PATTERNS = ('vssver.scc', '*.xls', '*.h', '*.dtx', '*.eng', '*.pfn')
# Root files the browser build consumes, in addition to the derived movie tables.
ROOT_FILES = ('fe.txt', 'version.txt', 'nfs5.ini')
# Candidate executables for the MAD decoder tables. Some PORSCHE.ICD builds carry a
# different table layout, so --from-installation validates each candidate before using it.
MOVIE_NAMES = ('nfs5.exe', 'porsche.exe', 'porsche.icd')


def _ignore_unneeded(_directory, names):
    return {
        name for name in names
        if any(fnmatch.fnmatchcase(name.casefold(), pattern) for pattern in UNNEEDED_PATTERNS)
    }


def _find_root_file(root: Path, name: str) -> Path | None:
    wanted = name.casefold()
    for path in root.iterdir():
        if path.is_file() and path.name.casefold() == wanted:
            return path
    return None


def _movie_candidates(root: Path) -> list[Path]:
    order = {name: index for index, name in enumerate(MOVIE_NAMES)}
    found = [p for p in root.iterdir() if p.is_file() and p.name.casefold() in order]
    return sorted(found, key=lambda p: order[p.name.casefold()])


def extract_mad_tables(exe: Path, output: Path) -> None:
    """Run the Rust extractor; raise RuntimeError with its diagnostics on failure."""
    command = [
        'cargo', '+stable', 'run', '--quiet', '--locked',
        '-p', 'nfs-formats', '--example', 'extract_mad_tables',
        '--', str(exe.resolve()), str(output.resolve()),
    ]
    proc = subprocess.run(command, cwd=Path(__file__).resolve().parents[2], check=False,
                          capture_output=True, text=True)
    if proc.returncode != 0:
        detail = (proc.stderr or proc.stdout).strip()
        lines = detail.splitlines()
        if len(lines) > 8:
            detail = '\n'.join(lines[-8:])
        raise RuntimeError(detail or f'extract_mad_tables exited with {proc.returncode}')


def stage_installation(source: Path, stage: Path, movie_exe: Path | None = None,
                       extractor=extract_mad_tables) -> Path:
    """Copy only the files the browser build consumes into stage.

    Returns the executable that supplied the movie tables.
    """
    if not all((source / name).is_dir() for name in ('GameData', 'FEData')):
        raise ValueError('source must contain GameData and FEData')
    stage.mkdir(parents=True, exist_ok=True)
    for name in ('GameData', 'FEData'):
        shutil.copytree(source / name, stage / name, ignore=_ignore_unneeded)
    for name in ROOT_FILES:
        path = _find_root_file(source, name)
        if path is not None:
            shutil.copyfile(path, stage / path.name)

    target = stage / 'madtables.bin'
    if movie_exe is not None:
        movie_exe = movie_exe.resolve()
        if not movie_exe.is_file():
            raise ValueError(f'movie executable not found: {movie_exe}')
        extractor(movie_exe, target)
        if not target.is_file():
            raise ValueError(f'extractor produced no movie tables for {movie_exe}')
        print(f'Movie tables: {movie_exe}')
        return movie_exe

    candidates = _movie_candidates(source)
    if not candidates:
        looked = ', '.join(MOVIE_NAMES)
        raise ValueError(f'movie executable not found in {source}; looked for {looked}')
    failures = []
    for candidate in candidates:
        target.unlink(missing_ok=True)
        try:
            extractor(candidate, target)
            if not target.is_file():
                raise RuntimeError('extractor produced no movie tables')
        except Exception as exc:  # noqa: BLE001 - report every candidate
            failures.append(f'{candidate.name}: {exc}')
            continue
        print(f'Movie tables: {candidate}')
        return candidate
    target.unlink(missing_ok=True)
    detail = '\n'.join(f'  {failure}' for failure in failures)
    raise ValueError(f'no compatible movie executable found:\n{detail}')


def prepare_from_installation(source: Path, output: Path, movie_exe: Path | None = None,
                              extra_files=(), extractor=extract_mad_tables):
    """Stage an original installation, pack it, and return the manifest."""
    source, output = source.resolve(), output.resolve()
    if source == output or source in output.parents:
        raise ValueError('output must be outside the installation')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.nfs-web-stage-', dir=output.parent) as temporary:
        stage = Path(temporary)
        stage_installation(source, stage, movie_exe, extractor)
        return publish(stage, output, extra_files)


def compress_object(target: Path) -> None:
    """Keep a deterministic HTTP gzip representation only when it saves bandwidth."""
    data = target.read_bytes()
    compressed = gzip.compress(data, compresslevel=6, mtime=0)
    sidecar = target.with_name(target.name + '.gz')
    if len(compressed) + 64 < len(data):
        temporary = sidecar.with_name(sidecar.name + '.tmp')
        temporary.write_bytes(compressed)
        temporary.replace(sidecar)
    else:
        sidecar.unlink(missing_ok=True)


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
            raise ValueError(f'object integrity failure: {digest}')
    # Include song segments and headers as well as ordinary file objects.
    urls = {entry['url'] for entry in files.values() if 'url' in entry}
    urls.update(f'objects/{digest}' for digest in objects)
    for url in sorted(urls):
        compress_object(output / url)
    manifest = {'schema': 1, 'version': version, 'files': files}
    temporary = output / 'manifest.json.tmp'
    temporary.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    temporary.replace(output / 'manifest.json')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--from-installation', action='store_true',
                        help='source is the original game folder; stage only consumed files')
    parser.add_argument('--movie-exe', type=Path,
                        help='Original game PE image; auto-detected in --from-installation mode')
    parser.add_argument('--extra-file', type=Path, action='append', default=[],
                        help='Additional root configuration or derived table file')
    args = parser.parse_args()
    try:
        if args.from_installation:
            prepare_from_installation(args.source, args.output, args.movie_exe, args.extra_file)
        else:
            with tempfile.TemporaryDirectory(prefix='nfs-web-tables-') as temporary:
                extras = list(args.extra_file)
                if args.movie_exe:
                    tables = Path(temporary) / 'madtables.bin'
                    extract_mad_tables(args.movie_exe.resolve(), tables)
                    extras.append(tables)
                publish(args.source, args.output, extras)
    except (ValueError, RuntimeError, OSError) as exc:
        raise SystemExit(f'error: {exc}')


if __name__ == '__main__':
    main()
