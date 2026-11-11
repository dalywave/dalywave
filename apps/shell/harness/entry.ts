// Стенд проверки оболочки: вызывает команды моста и пишет результаты в /report.
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createShell, isShellError, trackSnapArea, type ShellEvents } from '../bridge/shell'

const shell = createShell({ invoke, listen })
const log = document.getElementById('log')!
const params = new URLSearchParams(location.search)
// Отчёты и команды — на сервер стенда: страница бывает и в собранном приложении (tauri://)
const BASE = 'http://localhost:3499'

function report(kind: string, data: unknown) {
  const line = `${new Date().toISOString().slice(11, 23)} ${kind} ${JSON.stringify(data)}`
  const row = document.createElement('div')
  row.textContent = line
  log.prepend(row)
  void fetch(`${BASE}/report`, { method: 'POST', body: JSON.stringify({ kind, data }) }).catch(() => {})
}

async function step<T>(name: string, run: () => Promise<T>): Promise<T | undefined> {
  try {
    const result = await run()
    report(`ok ${name}`, result ?? null)
    return result
  } catch (e) {
    report(`err ${name}`, isShellError(e) ? e : String(e))
    return undefined
  }
}

const events: (keyof ShellEvents)[] = [
  'window:maximized', 'window:fullscreen', 'window:focus', 'window:caption', 'auth:changed',
  'media:command', 'server:changed', 'update:available', 'update:progress', 'deep-link:open',
]
for (const name of events) {
  void shell.on(name, payload => report(`event ${name}`, payload))
}

// Заголовок: высота и отступ от «светофора» из сведений оболочки
const info = shell.info
const root = document.documentElement
if (params.has('vibrancy')) root.classList.add('vibrancy')
if (info) {
  root.dataset.controls = info.windowControls
  root.style.setProperty('--titlebar-h', `${info.titleBar.height}px`)
  if (info.titleBar.controls) {
    const c = info.titleBar.controls
    root.style.setProperty('--controls-right', `${c.x + c.width + 12}px`)
    root.style.setProperty('--controls-cy', `${c.y + c.height / 2}px`)
  }
}
document.getElementById('info')!.textContent = JSON.stringify(info, null, 1)

const maximize = document.getElementById('btn-max')!
trackSnapArea(shell, maximize)

let audio: HTMLAudioElement | undefined
/** Тихий тон 220 Гц: WAV 8 кГц, 8 бит, моно — data: URL без сети */
function tone(seconds: number): string {
  const rate = 8000
  const n = rate * seconds
  const bytes = new Uint8Array(44 + n)
  const view = new DataView(bytes.buffer)
  const text = (at: number, s: string) => [...s].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)))
  text(0, 'RIFF'); view.setUint32(4, 36 + n, true); text(8, 'WAVEfmt ')
  view.setUint32(16, 16, true); view.setUint16(20, 1, true); view.setUint16(22, 1, true)
  view.setUint32(24, rate, true); view.setUint32(28, rate, true); view.setUint16(32, 1, true); view.setUint16(34, 8, true)
  text(36, 'data'); view.setUint32(40, n, true)
  for (let i = 0; i < n; i++) bytes[44 + i] = 128 + Math.round(6 * Math.sin((2 * Math.PI * 220 * i) / rate))
  let binary = ''
  for (const b of bytes) binary += String.fromCharCode(b)
  return `data:audio/wav;base64,${btoa(binary)}`
}

const actions: Record<string, () => Promise<unknown>> = {
  'platform': () => shell.platformInfo(),
  'min': () => shell.window.minimize(),
  'min-c': () => shell.window.minimize(),
  'max': () => shell.window.toggleMaximize(),
  'max-t': () => shell.window.toggleMaximize(),
  'close': () => shell.window.close(),
  'close-c': () => shell.window.close(),
  'drag-test': async () => {
    // Где у страницы перетаскиваемый заголовок — для сценария проверки
    const r = document.querySelector('.topbar')!.getBoundingClientRect()
    return { x: r.left + 200, y: r.top + r.height / 2, inner: [innerWidth, innerHeight] }
  },
  'size': async () => ({ inner: [innerWidth, innerHeight], dpr: devicePixelRatio }),
  'is-max': () => shell.window.isMaximized(),
  'theme-dark': () => shell.window.setTheme('dark'),
  'theme-light': () => shell.window.setTheme('light'),
  'login': () => shell.auth.login(),
  'cancel': () => shell.auth.cancel(),
  'logout': () => shell.auth.logout(),
  'token': async () => {
    const token = await shell.auth.getToken()
    return token ? `${token.slice(0, 8)}… (${token.length})` : null
  },
  'session': () => shell.auth.session(),
  'np-play': () => shell.nowPlaying.update({
    title: 'Float', artist: 'Call Me Karizma', album: 'Float',
    artworkUrl: 'https://e-cdns-images.dzcdn.net/images/cover/2e018122cb56986277102d2041a592c8/500x500-000000-80-0-0.jpg',
    durationMs: 169_000, positionMs: 75_000, playing: true,
  }),
  'np-pause': () => shell.nowPlaying.update({
    title: 'Float', artist: 'Call Me Karizma', album: 'Float', durationMs: 169_000, positionMs: 80_000, playing: false,
  }),
  'np-clear': () => shell.nowPlaying.update(null),
  'external': () => shell.openExternal('https://wave.daly.rest'),
  'external-bad': () => shell.openExternal('file:///etc/passwd'),
  // Звук в самом WebView: публикует ли WebKit свою плашку «Сейчас играет» рядом с нашей
  'audio-play': async () => {
    audio ??= new Audio(tone(30))
    await audio.play()
    return { playing: !audio.paused }
  },
  'audio-pause': async () => {
    audio?.pause()
    return { playing: false }
  },
  'audio-state': async () => ({ playing: audio ? !audio.paused : false, time: audio?.currentTime ?? 0 }),
  'reload': async () => location.reload(),
  'server': () => shell.server.get(),
  'locale-en': () => shell.setLocale('en'),
  'locale-ru': () => shell.setLocale('ru'),
  'update': () => shell.updates.check(),
}
for (const [id, run] of Object.entries(actions)) {
  document.getElementById(`btn-${id}`)?.addEventListener('click', () => void step(id, run))
}
// Для автоматической проверки: window.__run('login') из стенда
;(window as unknown as { __run: (id: string) => unknown }).__run = (id: string) => step(id, actions[id]!)

report('boot', { href: location.href, info })
void (async () => {
  await step('platform', actions['platform']!)
  await step('is-max', actions['is-max']!)
  await step('server', actions['server']!)
  await step('session', actions['session']!)
  await step('token', actions['token']!)
  for (const id of (params.get('auto') ?? '').split(',').filter(Boolean)) {
    await step(id, actions[id]!)
  }
})()

// Команды из сценария проверки: curl -d login localhost:3499/command
setInterval(async () => {
  try {
    const res = await fetch(`${BASE}/command`)
    if (res.status !== 200) return
    const id = (await res.text()).trim()
    if (actions[id]) void step(id, actions[id])
    else report('unknown command', id)
  } catch { /* стенд выключен */ }
}, 400)
