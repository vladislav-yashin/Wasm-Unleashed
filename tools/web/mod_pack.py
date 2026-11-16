#!/usr/bin/env python3
"""Build a mod overlay and register it in a prepared data directory."""
import argparse
import hashlib
import json
from pathlib import Path
import re

from pack import publish


def register(catalog, entry):
    """Keep list order on updates; new packs have the highest overlay priority."""
    if catalog.get('schema') != 1 or not isinstance(catalog.get('packs'), list):
        raise ValueError('unsupported mod catalog')
    packs = catalog['packs']
    for index, old in enumerate(packs):
        if old['id'] == entry['id']:
            packs[index] = entry
            break
    else:
        packs.append(entry)
    return catalog


def build(source, output):
    source, output = source.resolve(), output.resolve()
    config = json.loads((source / 'mod.json').read_text(encoding='utf-8'))
    if not isinstance(config, dict):
        raise ValueError('mod.json must be an object')
    if not isinstance(config.get('id'), str) or not re.fullmatch(r'[a-z0-9][a-z0-9_-]{0,63}', config['id']):
        raise ValueError('id must contain 1–64 lowercase letters, digits, underscores or hyphens')
    if not isinstance(config.get('name'), str) or not config['name'].strip():
        raise ValueError('name must be a nonempty string')
    if not isinstance(config.get('description'), str):
        raise ValueError('description must be a string')
    if not isinstance(config.get('enabled_by_default'), bool):
        raise ValueError('enabled_by_default must be a boolean')
    manifest = publish(source, output, mod=True)
    if not manifest['files']:
        raise ValueError('mod pack has no game files')
    payload = json.dumps(manifest, ensure_ascii=False, sort_keys=True).encode('utf-8')
    digest = hashlib.sha256(payload).hexdigest()
    relative = f"mods/{config['id']}/{digest}.json"
    target = output / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(payload)
    entry = {key: config[key] for key in ('id', 'name', 'description', 'enabled_by_default')}
    entry.update(manifest=relative, sha256=digest, size=len(payload))
    catalog_path = output / 'mods.json'
    catalog = json.loads(catalog_path.read_text(encoding='utf-8')) if catalog_path.exists() else {'schema': 1, 'packs': []}
    register(catalog, entry)
    temporary = output / 'mods.json.tmp'
    temporary.write_text(json.dumps(catalog, ensure_ascii=False, indent=2), encoding='utf-8')
    temporary.replace(catalog_path)
    return entry


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    try:
        entry = build(args.source, args.output)
    except (ValueError, OSError) as exc:
        parser.exit(1, f'error: {exc}\n')
    print(f"Published mod: {entry['id']}")


if __name__ == '__main__':
    main()
