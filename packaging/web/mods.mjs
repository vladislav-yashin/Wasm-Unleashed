import { AssetSource } from './assets.mjs';

export async function discoverMods(base) {
  const response = await fetch(new URL('mods.json', base), {cache: 'no-cache'});
  // Existing installations do not need a catalog until their first mod is published.
  if (response.status === 404) return [];
  if (!response.ok) throw new Error(`Mod catalog HTTP ${response.status}`);
  const catalog = await response.json();
  if (catalog.schema !== 1 || !Array.isArray(catalog.packs)) throw new Error('Unsupported mod catalog');
  const ids = new Set();
  for (const pack of catalog.packs) {
    if (!/^[a-z0-9][a-z0-9_-]{0,63}$/.test(pack.id) || ids.has(pack.id)
        || typeof pack.name !== 'string' || typeof pack.description !== 'string'
        || typeof pack.enabled_by_default !== 'boolean'
        || !/^[a-f0-9]{64}$/.test(pack.sha256) || !Number.isSafeInteger(pack.size) || pack.size < 0
        || pack.manifest !== `mods/${pack.id}/${pack.sha256}.json`)
      throw new Error('Invalid mod catalog entry');
    ids.add(pack.id);
  }
  return catalog.packs;
}

export async function applyMods(source, packs) {
  const files = {...source.manifest.files};
  const versions = [source.manifest.version];
  for (const pack of packs) {
    // Mod manifests and all replacement objects use the ordinary verified asset cache.
    const bytes = await source.load({...pack, path: pack.name, url: pack.manifest});
    const manifest = JSON.parse(new TextDecoder().decode(bytes));
    if (manifest.schema !== 1 || !manifest.version || !manifest.files || Array.isArray(manifest.files))
      throw new Error(`Unsupported mod manifest: ${pack.name}`);
    for (const [path, entry] of Object.entries(manifest.files)) {
      if (!path || path !== path.toLowerCase() || path.includes('\\')
          || path.split('/').some(part => !part || part === '.' || part === '..'))
        throw new Error(`Invalid mod path: ${path}`);
      files[path] = entry;
    }
    versions.push(manifest.version);
  }
  return new AssetSource(source.base, {...source.manifest, files, version: versions.join(':')});
}
