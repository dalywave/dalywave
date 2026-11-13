// Превью картинок плагина без устройства: сетка кнопок во всех состояниях, полосы Stream Deck + по
// раскладкам из layouts/*.json и макет готового профиля. Рисует Chrome (на устройстве — Qt, шрифт
// текста полосы — системный). Обложки — с сервера PREVIEW_API (по умолчанию локальный :8475).
//   bun scripts/preview.ts <папка для PNG>
import { mkdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { chromium } from 'playwright-core'
import { Api } from '../src/api'
import { MOOD_COLORS, waveColors } from '../src/catalog'
import { setLanguage, t } from '../src/i18n'
import { artKey, coverKey, iconKey, statusKey } from '../src/render/keys'
import { type Fields, listFields, trackFields, volumeFields } from '../src/render/strip'
import { stripGlyph } from '../src/render/keys'
import { C } from '../src/render/svg'

const OUT = process.argv[2] ?? join(import.meta.dir, '..', 'dist', 'preview')
mkdirSync(OUT, { recursive: true })
setLanguage('ru')

const api = new Api()
api.configure(process.env.PREVIEW_API ?? 'http://localhost:8475', null)
const COVERS = [
  '/api/images/albums/01a1cc24-5647-7597-9e2c-eddd24639f3d/250.jpg',
  '/api/images/albums/01a1cb12-c9bb-706b-8612-4eafa1504f22/250.jpg',
  '/api/images/albums/01a1cc23-a268-725a-9cef-2fee2f4a1dbc/250.jpg',
  '/api/images/albums/01a1c8e5-3f8e-7424-b454-1ff81ef9e3b9/250.jpg',
]
const covers = await Promise.all(COVERS.map(c => api.cover(c)))
const cover = (i: number) => covers[i % covers.length] ?? null

// ---------- раскладки полосы ----------

interface LayoutItem { key: string, type: 'pixmap' | 'text' | 'bar', rect: [number, number, number, number], value?: string | number, color?: string, font?: { size?: number, weight?: number }, alignment?: string, opacity?: number, enabled?: boolean, bar_bg_c?: string, bar_fill_c?: string, range?: { min: number, max: number }, zOrder?: number }

const PLUGIN = join(import.meta.dir, '..', 'rest.daly.wave.sdPlugin')
function layout(name: string): LayoutItem[] {
  return (JSON.parse(readFileSync(join(PLUGIN, 'layouts', `${name}.json`), 'utf8')) as { items: LayoutItem[] }).items
}

function strip(name: string, fields: Fields, w = 200, h = 100): string {
  const items = layout(name).map((it) => {
    const f = fields[it.key]
    const merged: LayoutItem = { ...it, ...(typeof f === 'object' ? f : f !== undefined ? { value: f } : {}) } as LayoutItem
    if (merged.enabled === false) return ''
    const [x, y, iw, ih] = it.rect
    const pos = `position:absolute;left:${x}px;top:${y}px;width:${iw}px;height:${ih}px;opacity:${merged.opacity ?? 1};z-index:${it.zOrder ?? 0}`
    if (it.type === 'pixmap') {
      const src = String(merged.value ?? '')
      if (!src) return ''
      return `<img style="${pos}" src="${src.startsWith('data:') ? src : `data:image/svg+xml;base64,${readFileSync(join(PLUGIN, src)).toString('base64')}`}">`
    }
    if (it.type === 'text') {
      return `<div style="${pos};color:${merged.color};font:${merged.font?.weight ?? 400} ${merged.font?.size ?? 14}px -apple-system,'Segoe UI',sans-serif;line-height:${ih}px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;text-align:${merged.alignment ?? 'center'}">${String(merged.value ?? '')}</div>`
    }
    const max = merged.range?.max ?? 100
    const pct = Math.max(0, Math.min(1, Number(merged.value ?? 0) / max)) * 100
    return `<div style="${pos};background:${merged.bar_bg_c}"><div style="width:${pct}%;height:100%;background:${merged.bar_fill_c}"></div></div>`
  }).join('')
  return `<div class="strip" style="width:${w}px;height:${h}px">${items}</div>`
}

const now = { cover: cover(0), title: 'Кто тебе сказал?', subtitle: 'Lowborn, vista', progress: 0.42, playing: true }
const STRIPS: [string, string][] = [
  ['Мультимедиа: трек', strip('track', trackFields(now))],
  ['На паузе', strip('track', trackFields({ ...now, playing: false }))],
  ['Громкость', strip('volume', volumeFields({ level: 0.64, muted: false, label: '64 %', track: 'Кто тебе сказал? — Lowborn' }))],
  ['Без звука', strip('volume', volumeFields({ level: 0.64, muted: true, label: t().muted, track: 'Кто тебе сказал? — Lowborn' }))],
  ['Коллекция', strip('list', listFields({ cover: cover(1), fallback: 'playlist-bold', title: 'Не так как ты хочешь', subtitle: 'Mzlff', counter: '3 из 24' }))],
  ['Поиск', strip('list', listFields({ cover: cover(2), fallback: 'music-notes-bold', title: 'Синий Lamborghini', subtitle: '5opka', counter: '1 из 20' }))],
  ['Очередь', strip('list', listFields({ cover: cover(3), fallback: 'music-notes-bold', title: 'MONTAGEM RANHURA', subtitle: 'Parker the Bandit', counter: '2 из 18' }))],
  ['Моя волна: настроение', strip('list', listFields({ cover: null, fallback: 'waves-bold', colors: waveColors('calm'), title: 'Спокойное', subtitle: '', counter: '4 из 7' }))],
  ['Моя волна: «Работа»', strip('list', listFields({ cover: null, fallback: 'sparkle-bold', colors: waveColors('energetic'), title: 'Работа', subtitle: '', counter: '1 из 7' }))],
  ['Открытия', strip('list', listFields({ cover: cover(1), fallback: 'music-notes-bold', title: 'Где твоя любовь?', subtitle: 'Rushex', counter: '5 из 10' }))],
  ['Передать', strip('list', listFields({ cover: null, fallback: 'laptop-bold', title: 'MacBook Pro', subtitle: t().playing, counter: '1 из 3' }))],
  ['Не подключено', strip('list', { cover: stripGlyph('plugs-bold', 64, C.text3, 0.6), title: t().connect, subtitle: '', counter: '' })],
  ['Нет связи', strip('list', { cover: stripGlyph('cloud-slash-bold', 64, C.text3, 0.6), title: t().offline, subtitle: '', counter: '' })],
]

// ---------- кнопки ----------

const KEYS: [string, string][] = [
  ['Сейчас играет', coverKey({ cover: cover(0), progress: 0.42, playing: true })],
  ['…на паузе', coverKey({ cover: cover(0), progress: 0.42, playing: false })],
  ['Играть', iconKey({ glyph: 'play-fill' })],
  ['Пауза', iconKey({ glyph: 'pause-fill' })],
  ['Следующий', iconKey({ glyph: 'skip-forward-fill' })],
  ['Предыдущий', iconKey({ glyph: 'skip-back-fill' })],
  ['Перемешать: выкл', iconKey({ glyph: 'shuffle-bold', tone: 'off' })],
  ['Перемешать: вкл', iconKey({ glyph: 'shuffle-bold', tone: 'on' })],
  ['Повтор: выкл', iconKey({ glyph: 'repeat-bold', tone: 'off' })],
  ['Повтор: все', iconKey({ glyph: 'repeat-bold', tone: 'on' })],
  ['Повтор: один', iconKey({ glyph: 'repeat-once-bold', tone: 'on' })],
  ['Громче', iconKey({ glyph: 'speaker-high-bold', label: '64 %' })],
  ['Тише', iconKey({ glyph: 'speaker-low-bold', label: '64 %' })],
  ['Без звука', iconKey({ glyph: 'speaker-x-bold', label: t().muted, tone: 'off' })],
  ['Нравится: нет', iconKey({ glyph: 'heart-bold' })],
  ['Нравится: да', iconKey({ glyph: 'heart-fill', tone: 'accent' })],
  ['Не нравится', iconKey({ glyph: 'thumbs-down-bold' })],
  ['Не сейчас', iconKey({ glyph: 'arrow-bend-up-right-bold' })],
  ['В плейлист', iconKey({ glyph: 'list-plus-bold', label: 'В дорогу' })],
  ['…уже там', iconKey({ glyph: 'check-bold', label: 'В дорогу' })],
  ['Коллекция', artKey(cover(1), 'playlist-bold', 'Не так как ты хочешь')],
  ['Любимые', iconKey({ glyph: 'heart-fill', tone: 'accent', label: t().liked })],
  ['Поиск', iconKey({ glyph: 'magnifying-glass-bold', label: 'Lowborn' })],
  ['Очередь', artKey(cover(3), 'music-notes-bold', 'MONTAGEM RANHURA')],
  ['Моя волна', iconKey({ glyph: 'wave', label: t().wave })],
  ['…спокойное', iconKey({ glyph: 'wave', waveColors: waveColors('calm'), label: 'Спокойное' })],
  ['…«Работа»', iconKey({ glyph: 'laptop-bold', color: MOOD_COLORS.energetic[1], label: 'Работа · Бодрое' })],
  ['Открытия', iconKey({ glyph: 'compass-bold', label: t().discover })],
  ['Передать', iconKey({ glyph: 'laptop-bold', label: 'MacBook Pro', tone: 'on' })],
  ['Переходы', iconKey({ glyph: 'sparkle-bold', label: t().transition.smart })],
  ['…Диджей', iconKey({ glyph: 'vinyl-record-bold', label: t().transition.dj })],
  ['Таймер сна', iconKey({ glyph: 'moon-stars-bold', label: `30 ${t().min}` })],
  ['…идёт', iconKey({ glyph: 'moon-stars-bold', big: '23:41', ring: 0.79 })],
  ['Не подключено', statusKey('connect', t().connect)],
  ['Нет связи', statusKey('offline', t().offline)],
  ['Нет устройства', statusKey('noDevice', t().noDevice)],
]

const CSS = `body{margin:0;padding:24px;background:#1c1d21;color:#d6d8de;font:13px -apple-system,'Segoe UI',sans-serif}
h1{font-size:15px;font-weight:600;margin:0 0 16px}
.grid{display:grid;gap:18px 14px}
.cell{display:flex;flex-direction:column;align-items:center;gap:6px;font-size:11px;color:#9a9da6;text-align:center}
.key{width:96px;height:96px;border-radius:14px;overflow:hidden;background:#000;box-shadow:0 0 0 1px #2c2e34}
.key img{width:96px;height:96px;display:block}
.strip{position:relative;background:#000;overflow:hidden;border-radius:4px}
.deck{display:inline-flex;flex-direction:column;gap:22px;padding:28px 30px;background:#0b0c0e;border-radius:28px;box-shadow:0 0 0 1px #2a2c31}
.keys{display:grid;grid-template-columns:repeat(4,96px);gap:18px 26px;justify-content:center}
.touch{display:flex;background:#000;border-radius:6px;overflow:hidden;width:800px}
.touch .strip{border-radius:0}
.dials{display:grid;grid-template-columns:repeat(4,200px);justify-items:center}
.dial{width:74px;height:74px;border-radius:50%;background:radial-gradient(circle at 35% 30%,#3a3c42,#16171a 70%);box-shadow:0 0 0 1px #2c2e34}
.names{display:grid;grid-template-columns:repeat(4,200px);font-size:11px;color:#8c8f98;text-align:center}`

const img = (src: string) => `<img src="${src}">`

async function shoot(name: string, html: string, width: number): Promise<void> {
  const browser = await chromium.launch({ channel: 'chrome', headless: true })
  const page = await browser.newPage({ viewport: { width, height: 400 }, deviceScaleFactor: 2 })
  await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>${CSS}</style></head><body>${html}</body></html>`)
  await page.waitForTimeout(150)
  await page.screenshot({ path: join(OUT, name), fullPage: true })
  await browser.close()
}

await shoot('keys.png', `<h1>Кнопки 144×144 (здесь — 96)</h1><div class="grid" style="grid-template-columns:repeat(8,110px)">${KEYS.map(([n, s]) => `<div class="cell"><div class="key">${img(s)}</div>${n}</div>`).join('')}</div>`, 1000)
await shoot('strips.png', `<h1>Сенсорная полоса Stream Deck + — 200×100 на крутилку (здесь ×1,5)</h1><div class="grid" style="grid-template-columns:repeat(3,300px)">${STRIPS.map(([n, s]) => `<div class="cell"><div style="transform:scale(1.5);transform-origin:top left;width:300px;height:150px">${s}</div>${n}</div>`).join('')}</div>`, 1000)

// Макет готового профиля Stream Deck +
const profileKeys = [
  coverKey({ cover: cover(0), progress: 0.42, playing: true }),
  iconKey({ glyph: 'skip-back-fill' }),
  iconKey({ glyph: 'pause-fill' }),
  iconKey({ glyph: 'skip-forward-fill' }),
  iconKey({ glyph: 'heart-fill', tone: 'accent' }),
  iconKey({ glyph: 'wave', label: t().wave }),
  iconKey({ glyph: 'compass-bold', label: t().discover }),
  iconKey({ glyph: 'moon-stars-bold', label: `30 ${t().min}` }),
]
const profileStrips = [
  strip('volume', volumeFields({ level: 0.64, muted: false, label: '64 %', track: 'Кто тебе сказал? — Lowborn' })),
  strip('track', trackFields(now)),
  strip('list', listFields({ cover: cover(3), fallback: 'music-notes-bold', title: 'MONTAGEM RANHURA', subtitle: 'Parker the Bandit', counter: '1 из 18' })),
  strip('list', listFields({ cover: null, fallback: 'waves-bold', colors: waveColors('any'), title: t().wave, subtitle: '', counter: '1 из 7' })),
]
await shoot('profile.png', `<h1>Готовый профиль «DalyWave» для Stream Deck +</h1><div class="deck"><div class="keys">${profileKeys.map(s => `<div class="key">${img(s)}</div>`).join('')}</div><div class="touch">${profileStrips.join('')}</div><div class="dials">${profileStrips.map(() => '<div class="dial"></div>').join('')}</div><div class="names"><div>Громкость</div><div>Мультимедиа</div><div>Очередь</div><div>Моя волна</div></div></div>`, 960)
console.log(`превью: ${OUT}`)
