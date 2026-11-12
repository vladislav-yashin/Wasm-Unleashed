import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { apply, CATALOG, detectLocale, getLocale, LOCALES, normalizeLocale, onLocaleChange, setLocale, t } from './i18n.mjs';

const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const bootstrap = readFileSync(new URL('./bootstrap.mjs', import.meta.url), 'utf8');

test('every catalog carries the English keys and no empty text', () => {
  const keys = Object.keys(CATALOG.en).sort();
  for (const locale of LOCALES) {
    assert.deepEqual(Object.keys(CATALOG[locale]).sort(), keys, locale);
    for (const [key, value] of Object.entries(CATALOG[locale])) {
      assert.equal(typeof value, 'string', `${locale}:${key}`);
      assert.notEqual(value.trim(), '', `${locale}:${key}`);
    }
  }
  assert.equal(LOCALES[0], 'en');
});

test('Cyrillic stays in the Russian catalog', () => {
  for (const locale of LOCALES.filter(name => name !== 'ru')) {
    for (const [key, value] of Object.entries(CATALOG[locale])) assert.doesNotMatch(value, /[\u0400-\u04FF]/, `${locale}:${key}`);
  }
});

test('the UI starts in English and translates with placeholders', () => {
  setLocale('en', { persist: false });
  assert.equal(getLocale(), 'en');
  assert.equal(t('loading.progress', { done: 3, total: 7, path: 'a/b' }), '3 / 7: a/b');
  assert.equal(t('missing.key'), 'missing.key');
  setLocale('ru', { persist: false });
  assert.equal(t('saves.saved'), 'Сохранено');
  assert.equal(t('saves.failed', { error: 'quota' }), 'Не сохранено: quota. Экспортируйте прогресс.');
  assert.equal(t('loading.progress', { done: 1, total: 2, path: 'x' }), '1 / 2: x');
});

test('locale tags normalize to a shipped catalog', () => {
  assert.equal(normalizeLocale('de-AT'), 'de');
  assert.equal(normalizeLocale('FR_fr'), 'fr');
  assert.equal(normalizeLocale('xx'), null);
  assert.equal(normalizeLocale(undefined), null);
});

test('locale precedence is query, saved choice, browser languages, English', () => {
  assert.equal(detectLocale({ search: '?data=x&lang=de-AT', languages: ['fr-FR'], stored: 'it' }), 'de');
  assert.equal(detectLocale({ search: '?lang=xx', languages: ['fr-FR'], stored: 'it' }), 'it');
  assert.equal(detectLocale({ search: '', languages: ['es-MX', 'en-US'], stored: 'xx' }), 'es');
  assert.equal(detectLocale({ search: '', languages: ['pt-BR'], stored: null }), 'en');
  assert.equal(detectLocale({ search: '', languages: [], stored: null }), 'en');
});

test('an explicit choice is saved and updates the document language', () => {
  const element = { attributes: {}, setAttribute(name, value) { this.attributes[name] = value; } };
  const written = [];
  setLocale('fr-FR', { storage: { setItem: (key, value) => written.push([key, value]) }, documentElement: element });
  assert.equal(getLocale(), 'fr');
  assert.deepEqual(written, [['wasm-unleashed-locale', 'fr']]);
  assert.equal(element.attributes.lang, 'fr');
  setLocale('es', { persist: false, documentElement: element });
  assert.deepEqual(written, [['wasm-unleashed-locale', 'fr']]);
  assert.equal(element.attributes.lang, 'es');
});

test('locale listeners fire after the switch', () => {
  let seen = null;
  const off = onLocaleChange(locale => { seen = locale; });
  setLocale('it', { persist: false });
  assert.equal(seen, 'it');
  off();
  setLocale('en', { persist: false });
  assert.equal(seen, 'it');
});

const node = attributes => ({
  attributes: { ...attributes }, textContent: '',
  getAttribute(name) { return this.attributes[name] ?? null; },
  setAttribute(name, value) { this.attributes[name] = value; },
});
const root = nodes => ({
  querySelectorAll(selector) {
    const name = selector.slice(1, -1);
    return nodes.filter(item => name in item.attributes);
  },
});

test('apply translates text and accessibility attributes', () => {
  setLocale('ru', { persist: false });
  const text = node({ 'data-i18n': 'toolbar.export' });
  const label = node({ 'data-i18n-aria-label': 'touch.brake', 'aria-label': 'Brake' });
  const placeholder = node({ 'data-i18n-placeholder': 'input.profileName', placeholder: 'Profile name' });
  const title = node({ 'data-i18n-title': 'toolbar.fullscreen', title: 'Toggle fullscreen' });
  apply(root([text, label, placeholder, title]));
  assert.equal(text.textContent, 'Экспорт сохранений');
  assert.equal(label.getAttribute('aria-label'), 'Тормоз');
  assert.equal(placeholder.getAttribute('placeholder'), 'Имя профиля');
  assert.equal(title.getAttribute('title'), 'Полноэкранный режим');
});

test('index.html uses known keys and offers every catalog', () => {
  const keys = [...html.matchAll(/data-i18n(?:-[a-z-]+)?="([^"]+)"/g)].map(match => match[1]);
  assert.ok(keys.length > 0);
  for (const key of keys) assert.ok(CATALOG.en[key], `index.html: ${key}`);
  const options = [...html.matchAll(/<option value="([a-z]+)">/g)].map(match => match[1]);
  assert.deepEqual(options, LOCALES);
  assert.match(html, /<html lang="en">/);
});

test('bootstrap.mjs uses known keys', () => {
  const keys = [...bootstrap.matchAll(/\bt\('([^']+)'/g)].map(match => match[1]);
  assert.ok(keys.length > 0);
  for (const key of keys) assert.ok(CATALOG.en[key], `bootstrap.mjs: ${key}`);
});
