/**
 * Translations for the browser shell: loader, toolbar, touch labels and status messages.
 * The game's own text still comes from the original tables (`FEStrings.loc`, the HUD table),
 * so changing this locale never retranslates the front end, HUD or subtitles.
 *
 * English is the default and the fallback. To add a locale, add its catalog here, its option in
 * `index.html` and the locale to the tests; `test_*.mjs` checks that every catalog has the same
 * keys and that every `data-i18n` key used by the page exists.
 */
export const FALLBACK_LOCALE = 'en';
const STORAGE_KEY = 'wasm-unleashed-locale';

export const CATALOG = {
  en: {
    'status.ready': 'Ready to load the game',
    'action.start': 'Start',
    'action.reload': 'Reload',
    'action.retry': 'Retry',
    'loading.assets': 'Loading game resources…',
    'loading.progress': '{done} / {total}: {path}',
    'saves.saving': 'Saving…',
    'saves.saved': 'Saved',
    'saves.failed': 'Not saved: {error}. Export your progress.',
    'saves.temporary': 'Temporary saves: {error}',
    'saves.imported': 'Imported. Reload the game.',
    'toolbar.controls': 'Game controls',
    'toolbar.touch': 'Touch controls',
    'toolbar.fullscreen': 'Toggle fullscreen',
    'toolbar.export': 'Export saves',
    'toolbar.import': 'Import',
    'toolbar.language': 'Language',
    'touch.steerLeft': 'Steer left',
    'touch.steerRight': 'Steer right',
    'touch.pause': 'Pause',
    'touch.camera': 'Camera',
    'touch.zoomIn': 'Zoom in',
    'touch.zoomOut': 'Zoom out',
    'touch.handbrake': 'Handbrake',
    'touch.brake': 'Brake',
    'touch.throttle': 'Throttle',
    'input.profileName': 'Profile name',
    'error.contextLost': 'Graphics context lost. Reload the game to restore the saved profile.',
  },
  ru: {
    'status.ready': 'Готов к загрузке игры',
    'action.start': 'Запустить',
    'action.reload': 'Перезагрузить',
    'action.retry': 'Повторить',
    'loading.assets': 'Загрузка игровых ресурсов…',
    'loading.progress': '{done} / {total}: {path}',
    'saves.saving': 'Сохранение…',
    'saves.saved': 'Сохранено',
    'saves.failed': 'Не сохранено: {error}. Экспортируйте прогресс.',
    'saves.temporary': 'Сохранения временные: {error}',
    'saves.imported': 'Импортировано. Перезагрузите игру.',
    'toolbar.controls': 'Управление игрой',
    'toolbar.touch': 'Сенсорное управление',
    'toolbar.fullscreen': 'Полноэкранный режим',
    'toolbar.export': 'Экспорт сохранений',
    'toolbar.import': 'Импорт',
    'toolbar.language': 'Язык',
    'touch.steerLeft': 'Руль влево',
    'touch.steerRight': 'Руль вправо',
    'touch.pause': 'Пауза',
    'touch.camera': 'Камера',
    'touch.zoomIn': 'Приблизить',
    'touch.zoomOut': 'Отдалить',
    'touch.handbrake': 'Ручник',
    'touch.brake': 'Тормоз',
    'touch.throttle': 'Газ',
    'input.profileName': 'Имя профиля',
    'error.contextLost': 'Графический контекст потерян. Перезагрузите игру, чтобы восстановить сохранённый профиль.',
  },
  de: {
    'status.ready': 'Bereit zum Laden des Spiels',
    'action.start': 'Starten',
    'action.reload': 'Neu laden',
    'action.retry': 'Erneut versuchen',
    'loading.assets': 'Spielressourcen werden geladen…',
    'loading.progress': '{done} / {total}: {path}',
    'saves.saving': 'Speichern…',
    'saves.saved': 'Gespeichert',
    'saves.failed': 'Nicht gespeichert: {error}. Exportieren Sie Ihren Fortschritt.',
    'saves.temporary': 'Temporäre Spielstände: {error}',
    'saves.imported': 'Importiert. Laden Sie das Spiel neu.',
    'toolbar.controls': 'Spielsteuerung',
    'toolbar.touch': 'Touch-Steuerung',
    'toolbar.fullscreen': 'Vollbild umschalten',
    'toolbar.export': 'Spielstände exportieren',
    'toolbar.import': 'Importieren',
    'toolbar.language': 'Sprache',
    'touch.steerLeft': 'Links lenken',
    'touch.steerRight': 'Rechts lenken',
    'touch.pause': 'Pause',
    'touch.camera': 'Kamera',
    'touch.zoomIn': 'Vergrößern',
    'touch.zoomOut': 'Verkleinern',
    'touch.handbrake': 'Handbremse',
    'touch.brake': 'Bremse',
    'touch.throttle': 'Gas',
    'input.profileName': 'Profilname',
    'error.contextLost': 'Grafikkontext verloren. Laden Sie das Spiel neu, um das gespeicherte Profil wiederherzustellen.',
  },
  fr: {
    'status.ready': 'Prêt à charger le jeu',
    'action.start': 'Démarrer',
    'action.reload': 'Recharger',
    'action.retry': 'Réessayer',
    'loading.assets': 'Chargement des ressources du jeu…',
    'loading.progress': '{done} / {total}: {path}',
    'saves.saving': 'Enregistrement…',
    'saves.saved': 'Enregistré',
    'saves.failed': 'Non enregistré : {error}. Exportez votre progression.',
    'saves.temporary': 'Sauvegardes temporaires : {error}',
    'saves.imported': 'Importé. Rechargez le jeu.',
    'toolbar.controls': 'Commandes du jeu',
    'toolbar.touch': 'Commandes tactiles',
    'toolbar.fullscreen': 'Basculer en plein écran',
    'toolbar.export': 'Exporter les sauvegardes',
    'toolbar.import': 'Importer',
    'toolbar.language': 'Langue',
    'touch.steerLeft': 'Tourner à gauche',
    'touch.steerRight': 'Tourner à droite',
    'touch.pause': 'Pause',
    'touch.camera': 'Caméra',
    'touch.zoomIn': 'Zoom avant',
    'touch.zoomOut': 'Zoom arrière',
    'touch.handbrake': 'Frein à main',
    'touch.brake': 'Frein',
    'touch.throttle': 'Accélérateur',
    'input.profileName': 'Nom du profil',
    'error.contextLost': 'Contexte graphique perdu. Rechargez le jeu pour restaurer le profil enregistré.',
  },
  it: {
    'status.ready': 'Pronto per caricare il gioco',
    'action.start': 'Avvia',
    'action.reload': 'Ricarica',
    'action.retry': 'Riprova',
    'loading.assets': 'Caricamento delle risorse di gioco…',
    'loading.progress': '{done} / {total}: {path}',
    'saves.saving': 'Salvataggio…',
    'saves.saved': 'Salvato',
    'saves.failed': 'Non salvato: {error}. Esporta i tuoi progressi.',
    'saves.temporary': 'Salvataggi temporanei: {error}',
    'saves.imported': 'Importato. Ricarica il gioco.',
    'toolbar.controls': 'Comandi di gioco',
    'toolbar.touch': 'Comandi touch',
    'toolbar.fullscreen': 'Attiva/disattiva schermo intero',
    'toolbar.export': 'Esporta salvataggi',
    'toolbar.import': 'Importa',
    'toolbar.language': 'Lingua',
    'touch.steerLeft': 'Sterza a sinistra',
    'touch.steerRight': 'Sterza a destra',
    'touch.pause': 'Pausa',
    'touch.camera': 'Telecamera',
    'touch.zoomIn': 'Ingrandisci',
    'touch.zoomOut': 'Riduci',
    'touch.handbrake': 'Freno a mano',
    'touch.brake': 'Freno',
    'touch.throttle': 'Acceleratore',
    'input.profileName': 'Nome profilo',
    'error.contextLost': 'Contesto grafico perso. Ricarica il gioco per ripristinare il profilo salvato.',
  },
  es: {
    'status.ready': 'Listo para cargar el juego',
    'action.start': 'Iniciar',
    'action.reload': 'Recargar',
    'action.retry': 'Reintentar',
    'loading.assets': 'Cargando recursos del juego…',
    'loading.progress': '{done} / {total}: {path}',
    'saves.saving': 'Guardando…',
    'saves.saved': 'Guardado',
    'saves.failed': 'No guardado: {error}. Exporta tu progreso.',
    'saves.temporary': 'Partidas temporales: {error}',
    'saves.imported': 'Importado. Recarga el juego.',
    'toolbar.controls': 'Controles del juego',
    'toolbar.touch': 'Controles táctiles',
    'toolbar.fullscreen': 'Alternar pantalla completa',
    'toolbar.export': 'Exportar partidas',
    'toolbar.import': 'Importar',
    'toolbar.language': 'Idioma',
    'touch.steerLeft': 'Girar a la izquierda',
    'touch.steerRight': 'Girar a la derecha',
    'touch.pause': 'Pausa',
    'touch.camera': 'Cámara',
    'touch.zoomIn': 'Acercar',
    'touch.zoomOut': 'Alejar',
    'touch.handbrake': 'Freno de mano',
    'touch.brake': 'Freno',
    'touch.throttle': 'Acelerador',
    'input.profileName': 'Nombre del perfil',
    'error.contextLost': 'Se perdió el contexto gráfico. Recarga el juego para restaurar el perfil guardado.',
  },
};

export const LOCALES = Object.keys(CATALOG);
const listeners = new Set();
let current = FALLBACK_LOCALE;

function defaultStorage() {
  try { return globalThis.localStorage ?? null; } catch { return null; }
}

function storedLocale() {
  try { return defaultStorage()?.getItem(STORAGE_KEY) ?? null; } catch { return null; }
}

/** `de-AT` and `de_DE` select `de`; a locale without a catalog selects `null`. */
export function normalizeLocale(tag) {
  const base = String(tag ?? '').trim().toLowerCase().split(/[-_]/)[0];
  return Object.hasOwn(CATALOG, base) ? base : null;
}

/**
 * Locale precedence: `?lang=`, then the saved choice, then the browser's languages, then English.
 * A query parameter applies to that page load only and is not saved.
 */
export function detectLocale({ search = globalThis.location?.search ?? '', languages = globalThis.navigator?.languages ?? [], stored = storedLocale() } = {}) {
  const query = new URLSearchParams(search).get('lang');
  return normalizeLocale(query) ?? normalizeLocale(stored)
    ?? languages.map(normalizeLocale).find(Boolean) ?? FALLBACK_LOCALE;
}

export function getLocale() { return current; }

/** Look up `key` with `{name}` placeholders; unknown keys fall back to English, then to the key. */
export function t(key, params = {}) {
  const text = CATALOG[current][key] ?? CATALOG[FALLBACK_LOCALE][key] ?? key;
  return text.replace(/\{(\w+)\}/g, (match, name) => Object.hasOwn(params, name) ? String(params[name]) : match);
}

/** Switch the active locale, remember an explicit choice and update `<html lang>`. */
export function setLocale(tag, { persist = true, storage = defaultStorage(), documentElement = globalThis.document?.documentElement } = {}) {
  current = normalizeLocale(tag) ?? FALLBACK_LOCALE;
  if (persist) { try { storage?.setItem(STORAGE_KEY, current); } catch { /* storage may be unavailable */ } }
  documentElement?.setAttribute('lang', current);
  for (const listener of listeners) listener(current);
  return current;
}

export function onLocaleChange(listener) { listeners.add(listener); return () => listeners.delete(listener); }

const ATTRIBUTES = [
  ['data-i18n', null],
  ['data-i18n-aria-label', 'aria-label'],
  ['data-i18n-placeholder', 'placeholder'],
  ['data-i18n-title', 'title'],
];

/** Replace text and attributes marked with `data-i18n*`; markup carries English as the no-JS fallback. */
export function apply(root = globalThis.document) {
  if (!root) return;
  for (const [selector, attribute] of ATTRIBUTES) {
    for (const node of root.querySelectorAll(`[${selector}]`)) {
      const value = t(node.getAttribute(selector));
      if (attribute) node.setAttribute(attribute, value); else node.textContent = value;
    }
  }
}
