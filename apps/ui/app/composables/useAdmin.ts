import type { AcquireReason, AcquisitionStatus, AttemptOutcome, AudioStatus, JobStatus, ServerEventType } from '~/api/types'
import type { ServerEventOf } from '~/bridge/types'
import type { Locale } from '~/utils/format'

// Общее для страниц админки: подписи статусов и видов задач, счётчики в навигации, живые события

/** Подписи из общего словаря `admin.*` — одинаковые на всех страницах админки */
export function useAdminLabels() {
  const { t, te } = useI18n({ useScope: 'global' })
  const pick = (key: string, raw: string) => (te(key) ? t(key) : raw)
  return {
    jobKind: (k: string) => pick(`admin.job.kind.${k}`, k),
    jobStatus: (s: JobStatus) => pick(`admin.job.status.${s}`, s),
    acqStatus: (s: AcquisitionStatus) => pick(`admin.acq.status.${s}`, s),
    reason: (r: AcquireReason) => pick(`admin.acq.reason.${r}`, r),
    outcome: (o: AttemptOutcome) => pick(`admin.acq.outcome.${o}`, o),
    audio: (s: AudioStatus) => pick(`admin.audio.${s}`, s),
    /** `server` → «Сервер», `device:…` → имя загрузчика, если известно */
    executor: (e: string | null | undefined, names?: Record<string, string>) => {
      if (!e) return '—'
      if (e === 'server') return t('admin.executor.server')
      const id = e.startsWith('device:') ? e.slice(7) : e
      return names?.[id] ?? t('admin.executor.device')
    },
  }
}

/** Тон точки состояния (AdminStatus) */
export type StatusTone = 'ok' | 'warn' | 'bad' | 'on' | 'idle'
export const JOB_TONE: Record<JobStatus, StatusTone> = { queued: 'idle', running: 'on', done: 'ok', failed: 'bad', cancelled: 'idle' }
export const ACQ_ACTIVE: AcquisitionStatus[] = ['queued', 'matching', 'downloading', 'verifying', 'processing']
export const AUDIO_COLOR: Record<AudioStatus, string> = {
  ready: 'var(--success)',
  queued: 'var(--chart-2)',
  acquiring: 'var(--accent)',
  none: 'rgba(var(--fg-rgb), 0.22)',
  disputed: 'var(--warning)',
  unavailable: 'rgba(var(--fg-rgb), 0.45)',
  failed: 'var(--danger)',
}

/** Счётчики в навигации: спорные записи и люди, ждущие доступа. Общие для раскладки и страниц */
export function useAdminCounts() {
  const counts = useState('admin-counts', () => ({ disputed: 0, waiting: 0 }))
  const bridge = useBridge()
  async function refresh() {
    const [overview, users, clips] = await Promise.allSettled([bridge.admin.overview(), bridge.admin.users(), bridge.admin.clipReports()])
    // «Спорные» — записи и клипы, которые слушатели скрыли у себя
    const clipCount = clips.status === 'fulfilled' ? clips.value.items.length : 0
    if (overview.status === 'fulfilled') counts.value.disputed = (overview.value.recordingsByStatus.find(s => s.status === 'disputed')?.count ?? 0) + clipCount
    if (users.status === 'fulfilled') counts.value.waiting = users.value.items.filter(u => !u.roles.length && !u.disabled).length
  }
  return { counts, refresh }
}

/** Подписка на живые события на время жизни страницы; `onReconnect` — данные могли устареть */
export function useAdminLive(handlers: { [K in ServerEventType]?: (e: ServerEventOf<K>) => void }, onReconnect?: () => void) {
  const bridge = useBridge()
  const offs: (() => void)[] = []
  onMounted(() => {
    bridge.live.connect()
    for (const [type, cb] of Object.entries(handlers)) {
      offs.push(bridge.live.on(type as ServerEventType, cb as never))
    }
    if (onReconnect) offs.push(bridge.live.onReconnect(onReconnect))
  })
  onBeforeUnmount(() => offs.forEach(off => off()))
}

/** Вызов с запоминанием, удался ли он: для страниц из нескольких независимых блоков */
export function useAttempts() {
  const failed = ref<Record<string, boolean>>({})
  async function attempt<T>(key: string, job: () => Promise<T>, apply: (v: T) => void) {
    try {
      apply(await job())
      failed.value = { ...failed.value, [key]: false }
    }
    catch {
      failed.value = { ...failed.value, [key]: true }
    }
  }
  return { failed, attempt }
}

/** Отложенный вызов: пачка живых событий — одно обновление */
export function useDebounced(fn: () => void, ms = 800) {
  let timer: ReturnType<typeof setTimeout> | undefined
  onBeforeUnmount(() => clearTimeout(timer))
  return () => {
    clearTimeout(timer)
    timer = setTimeout(fn, ms)
  }
}

/** Цена хранения в Cloudflare R2: $0,015 за ГБ в месяц, первые 10 ГБ бесплатно */
export const R2_PRICE_GB_MONTH = 0.015
export const R2_FREE_GB = 10
export function r2MonthlyUsd(bytes: number): number {
  const gb = bytes / 1024 ** 3
  return Math.max(0, gb - R2_FREE_GB) * R2_PRICE_GB_MONTH
}
export function formatUsd(v: number, locale: Locale): string {
  return new Intl.NumberFormat(intlLocale(locale, 'en-US'), { style: 'currency', currency: 'USD', maximumFractionDigits: v < 1 ? 2 : 0 }).format(v)
}
