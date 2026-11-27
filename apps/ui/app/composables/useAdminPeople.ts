import type { AdminUser, LiveNow, LiveOverview, PlayContext } from '~/api/types'

// Раздел «Люди» админки: «Сейчас» с живыми обновлениями, подписи статусов, роли и способы входа

/**
 * «Сейчас»: кто в сети и что слушает. Обновляется событием `admin.presence` (пачка — одним запросом),
 * при восстановлении связи и раз в 30 с на всякий случай: устройство пропадает молча, без событий.
 */
export function useAdminLiveFeed() {
  const bridge = useBridge()
  const data = ref<LiveOverview | null>(null)
  const failed = ref(false)
  async function load() {
    try {
      data.value = await bridge.admin.live()
      failed.value = false
    }
    catch {
      failed.value = true
    }
  }
  const soon = useDebounced(() => void load(), 400)
  useAdminLive({ 'admin.presence': soon }, soon)
  let timer: ReturnType<typeof setInterval> | undefined
  onMounted(() => {
    void load()
    timer = setInterval(() => void load(), 30_000)
  })
  onBeforeUnmount(() => clearInterval(timer))
  return { data, failed, load }
}

/** Часы, которые идут сами: позиция трека на экране без запросов к серверу */
export function useTicker(ms = 1000) {
  const now = ref(Date.now())
  let timer: ReturnType<typeof setInterval> | undefined
  onMounted(() => (timer = setInterval(() => (now.value = Date.now()), ms)))
  onBeforeUnmount(() => clearInterval(timer))
  return now
}

/** Позиция на момент `now`: пока играет, время идёт */
export function livePosition(n: LiveNow, now: number): number {
  if (!n.playing) return n.positionMs
  const pos = n.positionMs + Math.max(0, now - new Date(n.at).getTime())
  return n.durationMs > 0 ? Math.min(pos, n.durationMs) : pos
}

/** Подписи раздела из общего словаря `admin.people.*` */
export function usePeopleLabels() {
  const { t } = useI18n({ useScope: 'global' })
  const lang = useLang()
  return {
    /** «Волна», «Плейлист «Ночь»», «Альбом «…»» */
    context: (c: PlayContext | null | undefined) => {
      if (!c) return null
      const key = `admin.people.context.${c.type}`
      return t(key, { title: c.title })
    },
    /** В сети / был N назад / ждёт доступа / заблокирован */
    status: (u: AdminUser, online: boolean, lastSeenAt: string | null): { tone: StatusTone, text: string } => {
      if (u.disabled) return { tone: 'bad', text: t('admin.people.status.disabled') }
      if (online) return { tone: 'ok', text: t('admin.people.status.online') }
      if (!u.roles.length) return { tone: 'warn', text: t('admin.people.status.waiting') }
      if (lastSeenAt) return { tone: 'idle', text: t('admin.people.status.seen', { when: formatRelative(lastSeenAt, lang.value) }) }
      return { tone: 'idle', text: t('admin.people.status.never') }
    },
    role: (r: string) => t(`admin.people.role.${r}`),
    /** Как входит: провайдеры и свой логин */
    methods: (u: AdminUser) => {
      const list = [...u.providers]
      if (u.username && u.hasPassword) list.push(t('admin.people.login', { name: u.username }))
      return list.length ? list.join(', ') : t('admin.people.emergency')
    },
    /** «12 ч 30 мин», «45 мин», «—» */
    hours: (ms: number) => {
      if (ms < 60_000) return '—'
      const m = Math.round(ms / 60_000)
      const h = Math.floor(m / 60)
      if (lang.value === 'en') return h ? `${h} h ${m % 60} min` : `${m} min`
      if (lang.value === 'uk') return h ? `${h}\u00A0год ${m % 60}\u00A0хв` : `${m}\u00A0хв`
      return h ? `${h}\u00A0ч ${m % 60}\u00A0мин` : `${m}\u00A0мин`
    },
  }
}
