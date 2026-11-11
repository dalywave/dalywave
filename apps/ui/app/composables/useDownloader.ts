import type { DownloaderSnapshot, DownloaderStage, DownloaderStatus } from '~/bridge/types'

// Загрузчик музыки на этом компьютере (приложение macOS и Windows): одно состояние на всё
// приложение — его показывают индикатор в боковой панели и раздел в настройках. На сайте и
// телефонах загрузчика нет (`available: false`).

/** Что показать: состояние приложения и загрузчика, сведённое к одному виду */
export type DownloaderKind =
  | 'disabled' | 'signed-out' | 'forbidden' | 'starting' | 'idle' | 'working' | 'paused'
  | 'offline' | 'resting' | 'stopping' | 'restarting' | 'token' | 'broken'

export interface DownloaderJobView {
  track: string | null
  artist: string | null
  stage: DownloaderStage
  /** Ход этапа, 0…1 */
  stageProgress: number
  /** Ход всего задания, 0…1 */
  progress: number
  /** Номер кандидата: после неудачной проверки скачивается следующий */
  attempt: number
}

export interface DownloaderView {
  kind: DownloaderKind
  job: DownloaderJobView | null
  /** Пауза попрошена — доделывает текущий трек */
  finishing: boolean
  /** Причина: от загрузчика (`bot_check`, `server_unreachable`…) или оболочки (`token_rejected`…) */
  error: { code: string, message: string } | null
  /** Когда попробует снова, мс */
  retryAt: number | null
  counters: { doneToday: number, failedToday: number } | null
  pot: DownloaderStatus['pot'] | null
}

const STAGE_SPAN: Record<DownloaderStage, [number, number]> = {
  matching: [0, 0.1],
  downloading: [0.1, 0.65],
  verifying: [0.65, 0.8],
  uploading: [0.8, 1],
}

/** Ход всего задания по этапу и доле внутри него — те же доли, что в меню трея (downloader.rs) */
export function jobProgress(stage: DownloaderStage, progress: number): number {
  const [from, to] = STAGE_SPAN[stage] ?? [0, 0]
  const p = Number.isFinite(progress) ? Math.min(1, Math.max(0, progress)) : 0
  return from + (to - from) * p
}

export function describeDownloader(s: DownloaderSnapshot | null): DownloaderView {
  const view = (kind: DownloaderKind, extra: Partial<DownloaderView> = {}): DownloaderView => ({
    kind, job: null, finishing: false, error: s?.error ?? null, retryAt: s?.retryAtMs ?? null, counters: null, pot: null, ...extra,
  })
  if (!s) return view('starting')
  switch (s.phase) {
    case 'disabled': return view('disabled')
    case 'signed-out': return view('signed-out')
    case 'forbidden': return view('forbidden')
    case 'provisioning':
    case 'starting': return view('starting')
    case 'restarting': return view('restarting')
    case 'token-rejected': return view('token')
    case 'broken': return view('broken')
    case 'stopping':
    case 'running': break
  }
  const st = s.status
  if (!st) return view(s.phase === 'stopping' ? 'stopping' : 'starting')
  const common: Partial<DownloaderView> = {
    counters: { doneToday: st.counters.doneToday, failedToday: st.counters.failedToday },
    pot: st.pot,
    error: st.error,
    retryAt: st.retryAt ? Date.parse(st.retryAt) : null,
  }
  if (s.phase === 'stopping') return view('stopping', common)
  switch (st.state) {
    case 'idle': return view('idle', common)
    case 'paused': return view('paused', common)
    case 'offline': return view('offline', common)
    case 'stopping': return view('stopping', common)
    case 'starting': return view('starting', common)
    case 'error': {
      const code = st.error?.code
      if (code === 'token_rejected' || code === 'no_token') return view('token', common)
      if (code === 'tools_missing') return view('broken', common)
      return view('resting', common)
    }
    case 'working': {
      const j = st.job
      return view('working', {
        ...common,
        finishing: st.paused,
        job: j
          ? { track: j.track, artist: j.artist, stage: j.stage, stageProgress: j.progress, progress: jobProgress(j.stage, j.progress), attempt: j.attempt }
          : null,
      })
    }
  }
  return view('starting', common)
}

const snapshot = ref<DownloaderSnapshot | null>(null)
let subscribed = false

export function useDownloader() {
  const bridge = useBridge()
  const downloader = bridge.downloader
  if (downloader && !subscribed && import.meta.client) {
    subscribed = true
    // Ответ на `state()` может прийти позже первого события — тогда он уже устарел
    let fresh = false
    downloader.onState((s) => {
      fresh = true
      snapshot.value = s
    })
    void downloader.state().then((s) => {
      if (!fresh) snapshot.value = s
    }).catch(() => {})
  }

  async function apply(run: Promise<DownloaderSnapshot> | undefined) {
    const next = await run
    if (next) snapshot.value = next
  }

  return {
    /** Загрузчик есть только в приложении для компьютера */
    available: !!downloader,
    snapshot: computed(() => snapshot.value),
    view: computed(() => describeDownloader(snapshot.value)),
    setEnabled: (enabled: boolean) => apply(downloader?.setEnabled(enabled)),
    setPaused: (paused: boolean) => apply(downloader?.setPaused(paused)),
    recreateToken: () => apply(downloader?.recreateToken()),
    openLog: async () => downloader?.openLog(),
  }
}
