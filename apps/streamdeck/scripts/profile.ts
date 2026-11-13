// Готовый профиль Stream Deck + в составе плагина (profiles/DalyWave.streamDeckProfile): Stream Deck
// ставит его вместе с плагином и показывает в списке профилей (сам на него не переключается).
// Раскладка: кнопки — Сейчас играет, Пред., Играть/пауза, След. / Нравится, Моя волна, Открытия,
// Таймер сна; крутилки — Громкость, Мультимедиа, Очередь, Моя волна.
// Формат — как у экспорта из Stream Deck (ProfilesV3): zip с папкой <id>.sdProfile.
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { ACTIONS } from './actions'

const ROOT = join(import.meta.dir, '..')
const OUT = join(ROOT, 'rest.daly.wave.sdPlugin', 'profiles')
const pkg = await Bun.file(join(ROOT, 'package.json')).json() as { version: string }
const version = process.env.VERSION ?? pkg.version

// Постоянные id: профиль, страница и действия не меняются от сборки к сборке
const PROFILE_ID = '7A1C2E58-3B1D-4C61-9E0B-5D0A1F6B7C11'
const PAGE_ID = '2F6E9B14-8D3A-4E27-A5C9-0B7D41E3F902'

export const KEYS: [string, Record<string, string>][] = [
  ['nowplaying', {}],
  ['playback', { mode: 'previous' }],
  ['playback', { mode: 'toggle' }],
  ['playback', { mode: 'next' }],
  ['like', {}],
  ['wave', {}],
  ['discover', {}],
  ['sleep', { minutes: '30' }],
]

export const DIALS: [string, Record<string, string>][] = [
  ['volume', {}],
  ['multimedia', { mode: 'tracks' }],
  ['queue', {}],
  ['wave', {}],
]

function uuid(i: number): string {
  const h = i.toString(16)
  return `5d1e${h.padStart(4, '0')}-da1e-4a7e-8000-${h.padStart(12, '0')}`
}

function entry(id: string, settings: Record<string, string>, n: number, encoder: boolean) {
  const def = ACTIONS.find(a => a.id === id)
  if (!def) throw new Error(`нет действия ${id}`)
  return {
    ActionID: uuid(n),
    LinkedTitle: true,
    Name: def.ru.name,
    Plugin: { Name: 'DalyWave', UUID: 'rest.daly.wave', Version: version },
    Resources: null,
    Settings: settings,
    State: 0,
    States: [encoder ? {} : { Title: '', ShowTitle: true, TitleAlignment: 'bottom' }],
    UUID: `rest.daly.wave.${id}`,
    ...(encoder ? { Encoder: { IconVisible: true } } : {}),
  }
}

const keypad: Record<string, unknown> = {}
KEYS.forEach(([id, s], i) => {
  keypad[`${i % 4},${Math.floor(i / 4)}`] = entry(id, s, i + 1, false)
})
const encoders: Record<string, unknown> = {}
DIALS.forEach(([id, s], i) => {
  encoders[`${i},0`] = entry(id, s, 20 + i, true)
})

const tmp = join(ROOT, 'dist', 'profile-tmp')
rmSync(tmp, { recursive: true, force: true })
const base = join(tmp, `${PROFILE_ID}.sdProfile`)
const page = join(base, 'Profiles', PAGE_ID)
mkdirSync(join(page, 'Images'), { recursive: true })
mkdirSync(join(base, 'Images'), { recursive: true })
writeFileSync(join(base, 'manifest.json'), JSON.stringify({
  Device: { Model: '20GBD9901', UUID: '' },
  Name: 'DalyWave',
  Pages: { Current: PAGE_ID.toLowerCase(), Default: PAGE_ID.toLowerCase(), Pages: [PAGE_ID.toLowerCase()] },
  Version: '3.0',
}))
writeFileSync(join(page, 'manifest.json'), JSON.stringify({
  Controllers: [
    { Actions: keypad, Type: 'Keypad' },
    { Actions: encoders, Type: 'Encoder' },
  ],
  Icon: '',
  Name: '',
}))

mkdirSync(OUT, { recursive: true })
const file = join(OUT, 'DalyWave.streamDeckProfile')
rmSync(file, { force: true })
const zip = Bun.spawnSync(['zip', '-qrX', file, `${PROFILE_ID}.sdProfile`], { cwd: tmp })
if (zip.exitCode !== 0) throw new Error(`zip: ${zip.stderr}`)
rmSync(tmp, { recursive: true, force: true })
console.log('профиль Stream Deck +: 8 кнопок, 4 крутилки')
