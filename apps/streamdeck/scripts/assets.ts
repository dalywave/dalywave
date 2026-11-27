// Манифест, переводы и картинки плагина из одного списка действий (scripts/actions.ts) и тех же
// функций рисования, что и в работе. Запуск: bun run assets (сборка делает это сама).
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { setLanguage } from '../src/i18n'
import type { GlyphName } from '../src/render/glyphs.gen'
import { coverKey, iconKey } from '../src/render/keys'
import { C, glyph, svg, waveGlyph } from '../src/render/svg'
import { ACTIONS } from './actions'

const ROOT = join(import.meta.dir, '..', 'rest.daly.wave.sdPlugin')
const pkg = await Bun.file(join(import.meta.dir, '..', 'package.json')).json() as { version: string }

setLanguage('ru')

function write(path: string, body: string): void {
  const full = join(ROOT, path)
  mkdirSync(dirname(full), { recursive: true })
  writeFileSync(full, body)
}

function fromDataUri(uri: string): string {
  return Buffer.from(uri.slice(uri.indexOf(',') + 1), 'base64').toString('utf8')
}

/** Значок списка действий: белый на прозрачном, как требует Stream Deck */
function listIcon(g: GlyphName | 'wave', size = 40): string {
  const body = g === 'wave' ? waveGlyph(size / 2, size / 2, size * 1.1, ['#ffffff']) : glyph(g, size / 2, size / 2, size * 0.9, '#ffffff')
  return svg(size, size, body)
}

const LIST_GLYPH: Record<string, GlyphName | 'wave'> = {
  nowplaying: 'vinyl-record-bold', playback: 'play-pause-fill', multimedia: 'music-notes-bold', volume: 'speaker-high-bold',
  shuffle: 'shuffle-bold', repeat: 'repeat-bold', like: 'heart-bold', dislike: 'thumbs-down-bold',
  notnow: 'arrow-bend-up-right-bold', add: 'list-plus-bold', collection: 'playlist-bold', search: 'magnifying-glass-bold',
  queue: 'queue-bold', wave: 'wave', discover: 'compass-bold', transfer: 'devices-bold', transition: 'intersect-bold',
  sleep: 'moon-stars-bold',
}

/** Картинка кнопки до первого состояния с сервера */
function defaultKey(id: string): string {
  switch (id) {
    case 'nowplaying': return coverKey({ cover: null, progress: 0, playing: true })
    case 'playback': return iconKey({ glyph: 'play-fill' })
    case 'volume': return iconKey({ glyph: 'speaker-high-bold' })
    case 'shuffle': return iconKey({ glyph: 'shuffle-bold', tone: 'off' })
    case 'repeat': return iconKey({ glyph: 'repeat-bold', tone: 'off' })
    case 'like': return iconKey({ glyph: 'heart-bold' })
    case 'dislike': return iconKey({ glyph: 'thumbs-down-bold' })
    case 'notnow': return iconKey({ glyph: 'arrow-bend-up-right-bold' })
    case 'add': return iconKey({ glyph: 'list-plus-bold' })
    case 'collection': return iconKey({ glyph: 'playlist-bold' })
    case 'search': return iconKey({ glyph: 'magnifying-glass-bold' })
    case 'queue': return iconKey({ glyph: 'queue-bold' })
    case 'wave': return iconKey({ glyph: 'wave' })
    case 'discover': return iconKey({ glyph: 'compass-bold' })
    case 'transfer': return iconKey({ glyph: 'devices-bold' })
    case 'transition': return iconKey({ glyph: 'sparkle-bold', label: 'Умный' })
    case 'sleep': return iconKey({ glyph: 'moon-stars-bold' })
    default: return iconKey({ glyph: 'music-notes-bold' })
  }
}

// ---------- картинки ----------

for (const a of ACTIONS) {
  write(`imgs/actions/${a.id}/icon.svg`, listIcon(LIST_GLYPH[a.id] ?? 'music-notes-bold'))
  write(`imgs/actions/${a.id}/key.svg`, fromDataUri(defaultKey(a.id)))
  if (a.controllers.includes('Encoder')) write(`imgs/actions/${a.id}/dial.svg`, listIcon(LIST_GLYPH[a.id] ?? 'music-notes-bold', 72))
}

// Значок плагина — как у приложения: волна в цветах бренда на тёмном
const appIcon = (size: number) => svg(size, size, [
  `<defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2a2a2e"/><stop offset="1" stop-color="#0e0e10"/></linearGradient></defs>`,
  `<rect width="${size}" height="${size}" fill="url(#bg)"/>`,
  waveGlyph(size / 2, size / 2, size * 0.68, C.brand, 'brand'),
].join(''))
// PNG значка (его требует Stream Deck) рисует scripts/icons.ts — Chrome нужен один раз, файлы в репозитории
write('imgs/plugin/icon-source.svg', appIcon(512))
write('imgs/plugin/category.svg', listIcon('wave', 28))

// ---------- манифест ----------

const raw = process.env.VERSION ?? pkg.version
const version = /^\d+\.\d+\.\d+$/.test(raw) ? `${raw}.0` : raw

const manifest = {
  $schema: 'https://schemas.elgato.com/streamdeck/plugins/manifest.json',
  UUID: 'rest.daly.wave',
  Name: 'DalyWave',
  Version: version,
  Author: 'DalyWave',
  Description: 'Пульт DalyWave: кнопки, крутилки и сенсорная полоса Stream Deck управляют музыкой там, где она играет.',
  URL: 'https://wave.daly.rest',
  Icon: 'imgs/plugin/marketplace',
  Category: 'DalyWave',
  CategoryIcon: 'imgs/plugin/category',
  CodePath: 'bin/plugin.js',
  SDKVersion: 3,
  // SDK 3 (Node.js 24) — с 7.1, экран Neo — с 7.6
  Software: { MinimumVersion: '7.6' },
  OS: [
    { Platform: 'mac', MinimumVersion: '13' },
    { Platform: 'windows', MinimumVersion: '10' },
  ],
  Nodejs: { Version: '24' },
  Profiles: [
    { Name: 'profiles/DalyWave', DeviceType: 7, AutoInstall: true, DontAutoSwitchWhenInstalled: true, Readonly: false },
  ],
  Actions: ACTIONS.map(a => ({
    UUID: `rest.daly.wave.${a.id}`,
    Name: a.ru.name,
    Tooltip: a.ru.tooltip,
    Icon: `imgs/actions/${a.id}/icon`,
    PropertyInspectorPath: `ui/${a.id}.html`,
    Controllers: a.controllers,
    SupportedInMultiActions: a.controllers.includes('Keypad') && !['nowplaying', 'queue'].includes(a.id),
    States: [{ Image: `imgs/actions/${a.id}/key`, ShowTitle: true, TitleAlignment: 'bottom' }],
    ...(a.controllers.includes('Encoder') && a.layout
      ? {
          Encoder: {
            layout: a.layout,
            Icon: `imgs/actions/${a.id}/dial`,
            TriggerDescription: { Rotate: a.ru.rotate, Push: a.ru.push, Touch: a.ru.touch, LongTouch: a.ru.longTouch },
          },
        }
      : {}),
  })),
}
write('manifest.json', `${JSON.stringify(manifest, null, 2)}\n`)

// ---------- переводы названий ----------

const DESCRIPTION = {
  ru: 'Пульт DalyWave: кнопки, крутилки и сенсорная полоса Stream Deck управляют музыкой там, где она играет.',
  en: 'DalyWave remote: Stream Deck keys, dials and touch strip control your music wherever it plays.',
  uk: 'Пульт DalyWave: кнопки, регулятори й сенсорна смуга Stream Deck керують музикою там, де вона грає.',
}

for (const lang of ['ru', 'en', 'uk'] as const) {
  const out: Record<string, unknown> = {
    Name: 'DalyWave',
    Description: DESCRIPTION[lang],
    Category: 'DalyWave',
    Localization: {},
  }
  for (const a of ACTIONS) {
    const s = a[lang]
    out[`rest.daly.wave.${a.id}`] = {
      Name: s.name,
      Tooltip: s.tooltip,
      ...(s.rotate ? { Encoder: { TriggerDescription: { Rotate: s.rotate, Push: s.push, Touch: s.touch, LongTouch: s.longTouch } } } : {}),
    }
  }
  write(`${lang}.json`, `${JSON.stringify(out, null, 2)}\n`)
}

console.log(`манифест ${version}, действий: ${ACTIONS.length}`)
