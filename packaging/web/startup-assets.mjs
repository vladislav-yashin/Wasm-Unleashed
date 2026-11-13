// Keep the original intro and the entry screen's movie. Other media is prepared
// by the front end before its synchronous image/movie readers are called.
const STARTUP_MOVIES = new Set(['earts-av.mad', 'inpc-av.mad', 'ch6.mad']);

export function initialPaths(files) {
  const deferredCars = new Set(carPaths(files));
  for (const path of carPaths(files, ['993'])) deferredCars.delete(path);
  return Object.keys(files).filter(path => {
    if (deferredCars.has(path)) return false;
    if (path.startsWith('fedata/movies/')) return STARTUP_MOVIES.has(path.split('/').at(-1));
    if (path.startsWith('fedata/showcase/')) return false;
    // Chronicle icon packs also contain the timeline's year/index metadata.
    if (path.startsWith('fedata/chron/')) return path.split('/').at(-1).startsWith('icons');
    if (path.startsWith('gamedata/track/')) {
      return path.endsWith('/animdefs.txt') || path.endsWith('/default.bin')
        || path.startsWith('gamedata/track/data/') || path.split('/').at(-1).startsWith('alps');
    }
    return true;
  });
}

export function carPaths(files, models) {
  const prefix = 'gamedata/carmodel/';
  const selected = models ?? Object.keys(files).filter(path => path.startsWith(prefix) && path.endsWith('.crp'))
    .map(path => path.slice(prefix.length, -4));
  return [...new Set(selected.flatMap(model => ['.crp', '.tpg', 'd.fsh', 'l.fsh']
    .map(suffix => `${prefix}${model.toLowerCase()}${suffix}`)))].filter(path => path in files);
}

export function gamePaths(paths) {
  return [...new Set(paths.map(path => {
    const parts = String(path).replaceAll('\\', '/').toLowerCase().split('/').filter(p => p && p !== '.');
    if (parts.shift() !== 'game' || parts.includes('..')) throw new Error(`Invalid game resource: ${path}`);
    return parts.join('/');
  }))];
}
