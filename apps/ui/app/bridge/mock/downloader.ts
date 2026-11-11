// Мок загрузчика на компьютере — ТОЛЬКО РАЗРАБОТКА: подключается динамическим импортом за
// `import.meta.dev` (web/index.ts) и в сборку не попадает. Интерфейс выглядит как в приложении,
// но без оболочки и без процесса: события `downloader:state` идут по таймеру, как у настоящего
// загрузчика (ход задания — 4 раза в секунду). Сценарий — из адреса страницы:
//   ?downloader=idle | working | finishing | paused | offline | resting | starting | restarting
//               | disabled | token | forbidden | broken | signed-out
import type { DownloaderSnapshot, DownloaderStage, DownloaderStatus } from '../types'
import type { DownloaderSource } from '../tauri/downloader'

const SERVER = 'https://wave.daly.rest'
const WORKER = 'Mac MacBook Pro Захара'
const TICK_MS = 250

// Длительность этапов задания, мс: Deezer и поиск → скачивание → отпечаток и громкость → в хранилище
const STAGES: [DownloaderStage, number][] = [['matching', 1800], ['downloading', 7000], ['verifying', 2200], ['uploading', 2600]]

const JOBS = [
  { deezerTrackId: 4262016331, track: 'Another World', artist: 'CONTROL Resonant' },
  { deezerTrackId: 3107728361, track: 'Пыль', artist: 'Mzlff' },
  { deezerTrackId: 4059700721, track: 'Сигнал', artist: 'Rushex' },
  { deezerTrackId: 3580793301, track: 'Северный ветер', artist: 'Lowborn' },
]

const iso = (ms: number) => new Date(ms).toISOString()

export function createDevDownloader(scenario: string): DownloaderSource {
  const listeners = new Set<(s: DownloaderSnapshot) => void>()
  let jobIndex = 0
  let stageIndex = 1
  let stageElapsed = 0
  let idleFor = 0

  const status = (state: DownloaderStatus['state'], extra: Partial<DownloaderStatus> = {}): DownloaderStatus => ({
    state,
    paused: false,
    server: SERVER,
    workerName: WORKER,
    job: null,
    error: null,
    retryAt: null,
    counters: { date: new Date().toISOString().slice(0, 10), doneToday: 12, failedToday: 1 },
    pot: { state: 'ready', version: '2.0.2' },
    ...extra,
  })

  const snap: DownloaderSnapshot = {
    enabled: true,
    paused: false,
    phase: 'running',
    status: status('idle'),
    server: SERVER,
    workerName: WORKER,
    logFile: '/Users/zakhar/Library/Logs/DalyWave/fetcher.log',
    version: '0.2.0',
    ytDlp: '2026.10.26',
    error: null,
    retryAtMs: null,
  }

  function job() {
    const stage = STAGES[stageIndex] ?? STAGES[0]!
    const base = JOBS[jobIndex % JOBS.length]!
    return {
      id: `01a1ca44-2432-77cc-ad23-${String(jobIndex).padStart(12, '0')}`,
      ...base,
      stage: stage[0],
      progress: Math.min(1, stageElapsed / stage[1]),
      attempt: 1,
    }
  }

  function working(paused = false) {
    snap.phase = 'running'
    snap.status = status('working', { job: job(), paused, counters: snap.status?.counters ?? status('idle').counters })
  }

  // Начальное состояние сценария
  switch (scenario) {
    case 'working':
    case 'finishing':
      // Скачивание на 64 %: всё задание — 45 %
      stageIndex = 1
      stageElapsed = 0.64 * STAGES[1]![1]
      snap.paused = scenario === 'finishing'
      working(snap.paused)
      break
    case 'paused':
      snap.paused = true
      snap.status = status('paused', { paused: true })
      break
    case 'offline':
      snap.status = status('offline', {
        error: { code: 'server_unreachable', message: 'нет связи с сервером: Connection refused' },
        retryAt: iso(Date.now() + 40_000),
      })
      break
    case 'resting':
      snap.status = status('error', {
        error: { code: 'bot_check', message: 'YouTube попросил подтвердить, что это не бот' },
        retryAt: iso(Date.now() + 12 * 60_000),
      })
      break
    case 'starting':
      snap.phase = 'starting'
      snap.status = null
      break
    case 'restarting':
      snap.phase = 'restarting'
      snap.status = null
      snap.error = { code: 'crashed', message: 'загрузчик упал (код 1)' }
      snap.retryAtMs = Date.now() + 20_000
      break
    case 'disabled':
      snap.enabled = false
      snap.phase = 'disabled'
      snap.status = null
      break
    case 'token':
      snap.phase = 'token-rejected'
      snap.status = null
      snap.error = { code: 'token_rejected', message: 'сервер не принял токен загрузчика — его отозвали' }
      break
    case 'forbidden':
      snap.phase = 'forbidden'
      snap.status = null
      break
    case 'broken':
      snap.phase = 'broken'
      snap.status = null
      snap.error = { code: 'tools_missing', message: 'не запускаются: deno' }
      break
    case 'signed-out':
      snap.phase = 'signed-out'
      snap.status = null
      break
    default:
      break
  }

  const emit = () => {
    const copy = structuredClone(snap)
    listeners.forEach(cb => cb(copy))
  }

  // Жизнь загрузчика: этапы задания, готовый файл, пауза между заданиями
  setInterval(() => {
    const s = snap.status
    if (snap.phase !== 'running' || !s) return
    if (s.state === 'working') {
      stageElapsed += TICK_MS
      if (stageElapsed >= STAGES[stageIndex]![1]) {
        stageIndex++
        stageElapsed = 0
      }
      if (stageIndex >= STAGES.length) {
        s.counters.doneToday++
        jobIndex++
        stageIndex = 0
        idleFor = 0
        snap.status = status(snap.paused ? 'paused' : 'idle', { paused: snap.paused, counters: s.counters })
      }
      else {
        working(snap.paused)
      }
      emit()
    }
    else if (s.state === 'idle') {
      idleFor += TICK_MS
      if (idleFor >= 3000) {
        working(false)
        emit()
      }
    }
  }, TICK_MS)

  /** Запуск: «Запуск…», затем на связи */
  function start() {
    snap.phase = 'starting'
    snap.status = null
    snap.error = null
    snap.retryAtMs = null
    emit()
    setTimeout(() => {
      if (snap.phase !== 'starting') return
      snap.phase = 'running'
      snap.status = status(snap.paused ? 'paused' : 'idle', { paused: snap.paused })
      idleFor = 0
      emit()
    }, 1500)
  }

  return {
    state: async () => structuredClone(snap),
    listen(cb) {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    async setEnabled(enabled) {
      snap.enabled = enabled
      if (!enabled) {
        snap.phase = 'disabled'
        snap.status = null
        emit()
      }
      else if (snap.phase === 'disabled') {
        start()
      }
      return structuredClone(snap)
    },
    async setPaused(paused) {
      snap.paused = paused
      const s = snap.status
      if (s?.state === 'working') s.paused = paused
      else if (s && (s.state === 'idle' || s.state === 'paused')) {
        snap.status = status(paused ? 'paused' : 'idle', { paused, counters: s.counters })
        idleFor = 0
      }
      emit()
      return structuredClone(snap)
    },
    async recreateToken() {
      if (snap.phase !== 'token-rejected') throw Object.assign(new Error('токен загрузчика действует — новый не нужен'), { code: 'invalid' })
      snap.phase = 'provisioning'
      snap.error = null
      emit()
      setTimeout(start, 1200)
      return structuredClone(snap)
    },
    async openLog() {
      console.info(`[мок загрузчика] журнал: ${snap.logFile}`)
    },
  }
}
