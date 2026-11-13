// Инспекторы свойств (ui/<действие>.html): блок аккаунта (ui/pi.js) и поля настроек на
// sdpi-components. Подписи — ключи переводов из pi.js (data-t-*), язык — как у Stream Deck.
import { writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { ACTIONS } from './actions'

const UI = join(import.meta.dir, '..', 'rest.daly.wave.sdPlugin', 'ui')

type Opt = [value: string, key: string]

const select = (setting: string, label: string, options: Opt[], def: string) =>
  `<sdpi-item data-t-label="${label}"><sdpi-select setting="${setting}" default="${def}">${options.map(([v, k]) => `<option value="${v}" data-t="${k}"></option>`).join('')}</sdpi-select></sdpi-item>`

const source = (setting: string, label: string, ds: string, extra = '') =>
  `<sdpi-item data-t-label="${label}" id="row-${setting}"><sdpi-select setting="${setting}" datasource="${ds}" hot-reload ${extra}></sdpi-select></sdpi-item>`

const text = (setting: string, label: string, placeholder: string) =>
  `<sdpi-item data-t-label="${label}"><sdpi-textfield setting="${setting}" data-t-placeholder="${placeholder}"></sdpi-textfield></sdpi-item>`

const range = (setting: string, label: string, min: number, max: number, step: number, def: number) =>
  `<sdpi-item data-t-label="${label}"><sdpi-range setting="${setting}" min="${min}" max="${max}" step="${step}" default="${def}" showlabels></sdpi-range></sdpi-item>`

const checkbox = (setting: string, label: string, caption: string) =>
  `<sdpi-item data-t-label="${label}"><sdpi-checkbox setting="${setting}" default="true" data-t-label="${caption}"></sdpi-checkbox></sdpi-item>`

const help = (key: string) => `<p class="dw-help" data-t="${key}"></p>`

const MOODS: Opt[] = [['any', 'any'], ['energetic', 'energetic'], ['happy', 'happy'], ['calm', 'calm'], ['sad', 'sad']]
const ACTIVITIES: Opt[] = [['any', 'any'], ['wake', 'wake'], ['road', 'road'], ['work', 'work'], ['sport', 'sport'], ['rest', 'rest'], ['sleep', 'sleep'], ['party', 'party']]
const CHARACTERS: Opt[] = [['mix', 'mix'], ['fav', 'fav'], ['new', 'new'], ['pop', 'pop'], ['rare', 'rare']]
const LANGUAGES: Opt[] = [['any', 'any'], ['ru', 'langRu'], ['foreign', 'langForeign'], ['none', 'langNone']]

/** Поле выбора альбома или артиста поиском: найденное сохраняется в itemId */
const FIND = `
<div id="find" class="dw-hidden">
  <sdpi-item data-t-label="find"><div class="dw-row"><input id="find-q" class="dw-input" data-t-placeholder="findPlaceholder"><button id="find-go" class="dw-btn" type="button" data-t="find"></button></div></sdpi-item>
  <sdpi-item data-t-label="found"><select id="find-list" class="dw-input" style="width:100%"></select></sdpi-item>
</div>`

const COLLECTION_SCRIPT = `
<script>
  // Поля под источник: плейлист и чарт — списком, альбом и артист — поиском
  const [, saveItem] = SDPIComponents.useSettings('itemId', () => {})
  let sourceValue = 'liked'
  function show() {
    document.getElementById('row-itemId').classList.toggle('dw-hidden', sourceValue !== 'playlist')
    document.getElementById('row-chartId').classList.toggle('dw-hidden', sourceValue !== 'chart')
    document.getElementById('find').classList.toggle('dw-hidden', sourceValue !== 'album' && sourceValue !== 'artist')
  }
  SDPIComponents.useSettings('source', (v) => { sourceValue = v || 'liked'; show() })
  SDPIComponents.useSettings('chartId', (v) => { if (sourceValue === 'chart' && v) saveItem(v) })
  document.addEventListener('DOMContentLoaded', () => {
    const list = document.getElementById('find-list')
    const go = () => DW.toPlugin({ event: 'find', kind: sourceValue === 'artist' ? 'artist' : 'album', q: document.getElementById('find-q').value })
    document.getElementById('find-go').addEventListener('click', go)
    document.getElementById('find-q').addEventListener('keydown', e => { if (e.key === 'Enter') go() })
    list.addEventListener('change', () => saveItem(list.value))
    DW.onMessage((p) => {
      if (p.event !== 'found') return
      list.replaceChildren(...p.items.map((it) => { const o = document.createElement('option'); o.value = it.value; o.textContent = it.label; return o }))
      if (p.items[0]) saveItem(p.items[0].value)
    })
    show()
  })
</script>`

const WAVE_SCRIPT = `
<script>
  // Своя настройка — поля настроения, занятия, характера и языка; сохранённая — только её выбор
  SDPIComponents.useSettings('presetId', (v) => {
    document.querySelectorAll('.dw-own').forEach(e => e.classList.toggle('dw-hidden', !!v))
  })
</script>`

const ADD_SCRIPT = `
<script>
  SDPIComponents.useSettings('target', (v) => {
    document.getElementById('row-playlistId').classList.toggle('dw-hidden', (v || 'library') !== 'playlist')
  })
</script>`

const FIELDS: Record<string, string> = {
  nowplaying: [help('helpNow'), checkbox('pulse', 'pulse', 'pulseLabel')].join(''),
  playback: select('mode', 'mode', [['toggle', 'toggle'], ['play', 'play'], ['pause', 'pause'], ['next', 'next'], ['previous', 'previous']], 'toggle'),
  multimedia: [
    select('mode', 'dialMode', [['tracks', 'modeTracks'], ['seek', 'modeSeek'], ['volume', 'modeVolume']], 'tracks'),
    checkbox('pulse', 'pulse', 'pulseLabel'),
    help('helpMultimedia'),
  ].join(''),
  volume: [
    select('mode', 'mode', [['up', 'volumeUp'], ['down', 'volumeDown'], ['mute', 'mute']], 'up'),
    range('step', 'step', 1, 25, 1, 10),
    help('helpVolume'),
  ].join(''),
  shuffle: select('mode', 'mode', [['toggle', 'shuffleToggle'], ['on', 'on'], ['off', 'off']], 'toggle'),
  repeat: help('helpRepeat'),
  like: help('helpLike'),
  dislike: help('helpDislike'),
  notnow: help('helpNotNow'),
  add: [
    select('target', 'target', [['library', 'liked'], ['playlist', 'playlist']], 'library'),
    source('playlistId', 'playlist', 'writablePlaylists'),
    ADD_SCRIPT,
  ].join(''),
  collection: [
    select('source', 'source', [
      ['liked', 'liked'], ['playlist', 'playlist'], ['album', 'album'], ['artist', 'artist'], ['chart', 'chart'], ['history', 'history'],
      ['playlists', 'playlists'], ['mixes', 'mixes'], ['albums', 'albums'], ['artists', 'artists'], ['charts', 'charts'],
    ], 'liked'),
    source('itemId', 'playlist', 'playlists'),
    source('chartId', 'chart', 'charts'),
    FIND,
    `<sdpi-item data-t-label="shuffle"><sdpi-checkbox setting="shuffle"></sdpi-checkbox></sdpi-item>`,
    COLLECTION_SCRIPT,
  ].join(''),
  search: [
    text('query', 'query', 'queryPlaceholder'),
    select('kind', 'kind', [['track', 'tracks'], ['album', 'album'], ['artist', 'artist']], 'track'),
    select('scope', 'where', [['catalog', 'catalog'], ['library', 'library']], 'catalog'),
  ].join(''),
  queue: help('helpQueue'),
  wave: [
    `<sdpi-item data-t-label="preset"><sdpi-select setting="presetId" datasource="presets" hot-reload><option value="" data-t="ownSettings"></option></sdpi-select></sdpi-item>`,
    `<div class="dw-own">`,
    select('mood', 'mood', MOODS, 'any'),
    select('activity', 'activity', ACTIVITIES, 'any'),
    select('character', 'character', CHARACTERS, 'mix'),
    select('language', 'language', LANGUAGES, 'any'),
    `</div>`,
    WAVE_SCRIPT,
  ].join(''),
  discover: help('helpDiscover'),
  transfer: source('deviceId', 'device', 'devices'),
  transition: help('helpTransition'),
  sleep: [
    select('minutes', 'minutes', [['5', '5'], ['10', '10'], ['15', '15'], ['30', '30'], ['45', '45'], ['60', '60'], ['90', '90'], ['120', '120']], '30'),
    select('mode', 'sleepMode', [['toggle', 'sleepToggle'], ['restart', 'sleepRestart'], ['extend', 'sleepExtend']], 'toggle'),
  ].join(''),
}

for (const a of ACTIONS) {
  const body = FIELDS[a.id] ?? ''
  const html = `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <link rel="stylesheet" href="pi.css">
  <script src="sdpi-components.js"></script>
  <script src="pi.js"></script>
</head>
<body>
  <div class="dw-fields">${body}</div>
</body>
</html>
`
  writeFileSync(join(UI, `${a.id}.html`), html)
}
console.log(`инспекторов: ${ACTIONS.length}`)
