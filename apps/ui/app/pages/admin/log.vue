<script setup lang="ts">
import type { AdminUser, AuditEvent } from '~/api/types'

// Журнал: входы, роли, сессии, очередь, загрузчики, решения по спорным, настройки — с фильтрами
// по виду и по человеку
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const route = useRoute()
const router = useRouter()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const GROUPS = ['auth.', 'user.', 'session.', 'lane.', 'job.', 'downloader.', 'acquisition.', 'recording.', 'source.', 'reco.', 'settings.'] as const
const kind = ref<string>(typeof route.query.kind === 'string' ? route.query.kind : '')
const actor = ref<string>(typeof route.query.actor === 'string' ? route.query.actor : '')
const users = ref<AdminUser[]>([])
const items = ref<AuditEvent[] | null>(null)
const cursor = ref<string | null>(null)
const failed = ref(false)
let seq = 0

async function load() {
  const my = ++seq
  failed.value = false
  try {
    const p = await bridge.admin.audit(null, 50, { kind: kind.value, actor: actor.value })
    if (my !== seq) return
    items.value = p.items
    cursor.value = p.nextCursor
  }
  catch {
    if (my === seq) failed.value = true
  }
}
onMounted(() => {
  void load()
  void bridge.admin.users().then(l => (users.value = l.items)).catch(() => {})
})
watch([kind, actor], () => {
  void router.replace({ query: { kind: kind.value || undefined, actor: actor.value || undefined } })
  void load()
})
const more = ref(false)
async function loadMore() {
  if (!cursor.value) return
  more.value = true
  try {
    const p = await bridge.admin.audit(cursor.value, 50, { kind: kind.value, actor: actor.value })
    items.value = [...(items.value ?? []), ...p.items]
    cursor.value = p.nextCursor
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    more.value = false
  }
}

const kindOptions = computed(() => [{ value: '', label: t('allKinds') }, ...GROUPS.map(g => ({ value: g as string, label: t(`group.${g.slice(0, -1)}`) }))])
const actorOptions = computed(() => [{ value: '', label: t('anyone') }, ...users.value.map(u => ({ value: u.id, label: u.name }))])

const KNOWN = new Set([
  'auth.sign_in', 'auth.sign_in_failed', 'auth.sign_in_denied', 'auth.logout', 'auth.app_code_reused', 'user.created', 'user.roles_changed',
  'user.disabled', 'user.enabled', 'session.revoked', 'job.retried', 'job.cancelled', 'lane.changed', 'downloader.created', 'downloader.revoked',
  'acquisition.resolved', 'reco.settings_changed', 'reco.run', 'settings.changed', 'recording.reacquire', 'source.check', 'device.created',
  'recording.reported',
])
const kindLabel = (k: string) => (KNOWN.has(k) ? t(`kind.${k.replace('.', '_')}`) : k)
const BAD = new Set(['auth.sign_in_failed', 'auth.sign_in_denied', 'auth.app_code_reused'])
const hhmm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`

function details(e: AuditEvent) {
  const p = e.payload as Record<string, unknown>
  const parts: string[] = []
  if (typeof p.via === 'string') parts.push(p.via === 'password' ? t('d.password') : p.via === 'oidc' ? 'daly.rest' : String(p.via))
  if (Array.isArray(p.to)) parts.push(`${(Array.isArray(p.from) && p.from.join(', ')) || t('d.noRoles')} → ${p.to.join(', ') || t('d.noRoles')}`)
  else if (Array.isArray(p.roles)) parts.push(p.roles.join(', ') || t('d.noRoles'))
  if (p.reason === 'provider') parts.push(t('d.byProvider'))
  if (typeof p.platform === 'string') parts.push(p.platform)
  if (typeof p.email === 'string') parts.push(p.email)
  if (typeof p.name === 'string') parts.push(p.name)
  if (e.kind === 'lane.changed') {
    if (e.subject) parts.push(e.subject)
    if (typeof p.paused === 'boolean') parts.push(p.paused ? t('d.paused') : t('d.resumed'))
    if (typeof p.concurrency === 'number') parts.push(t('d.concurrency', { n: p.concurrency }))
    if (typeof p.minIntervalMs === 'number') parts.push(t('d.interval', { s: p.minIntervalMs / 1000 }))
    const w = p.schedule as { from?: number | null, until?: number | null } | null | undefined
    if (w) parts.push(w.from != null && w.until != null ? `${hhmm(w.from)}–${hhmm(w.until)}` : t('d.always'))
  }
  else if (typeof p.lane === 'string') parts.push(p.lane)
  if (typeof p.kind === 'string' && e.kind.startsWith('job.')) parts.push(p.kind)
  if (typeof p.action === 'string') parts.push(t(`d.action.${p.action}`))
  if (typeof p.url === 'string') parts.push(p.url)
  if (typeof p.queued === 'number') parts.push(t('d.queued', { n: p.queued }))
  if (e.kind === 'reco.run' && e.subject) parts.push(e.subject)
  if (typeof p.ip === 'string') parts.push(p.ip)
  return parts.join(' · ')
}

// Строки по дням; подряд идущие одинаковые события (десяток входов паролем) — одной строкой «×N»
const locale = computed(() => (lang.value === 'en' ? 'en-GB' : 'ru-RU'))
const dayKey = (iso: string) => new Date(iso).toLocaleDateString('sv-SE')
function dayLabel(key: string) {
  const today = dayKey(new Date().toISOString())
  const yesterday = dayKey(new Date(Date.now() - 86400_000).toISOString())
  if (key === today) return t('today')
  if (key === yesterday) return t('yesterday')
  const [y, m, d] = key.split('-').map(Number)
  const date = new Date(y!, m! - 1, d!)
  return new Intl.DateTimeFormat(locale.value, { day: 'numeric', month: 'long', ...(y === new Date().getFullYear() ? {} : { year: 'numeric' }) }).format(date)
}
const time = (iso: string) => new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
interface Row { e: AuditEvent, details: string, count: number, times: string[] }
const days = computed(() => {
  const out: { key: string, label: string, rows: Row[] }[] = []
  for (const e of items.value ?? []) {
    const key = dayKey(e.at)
    let day = out.at(-1)
    if (day?.key !== key) {
      day = { key, label: dayLabel(key), rows: [] }
      out.push(day)
    }
    const d = details(e)
    const last = day.rows.at(-1)
    if (last && last.e.kind === e.kind && last.e.actor?.id === e.actor?.id && last.details === d) {
      last.count++
      last.times.push(time(e.at))
    }
    else day.rows.push({ e, details: d, count: 1, times: [time(e.at)] })
  }
  return out
})
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <div class="ap-filters">
      <AdminSelect v-model="kind" :options="kindOptions" :label="t('filterKind')" />
      <AdminSelect v-model="actor" :options="actorOptions" :label="t('filterActor')" />
    </div>

    <AdminState :loading="!items && !failed" :error="failed" :empty="!!items && !items.length" empty-icon="ph:scroll" :empty-title="t('empty')" :rows="8" @retry="load">
      <div class="ap-table lg">
        <template v-for="d in days" :key="d.key">
          <div class="ap-group"><span>{{ d.label }}</span></div>
          <div v-for="r in d.rows" :key="r.e.id" class="ap-tr">
            <span class="ap-muted tnum" :title="r.count > 1 ? r.times.join(', ') : undefined">{{ r.times[0] }}</span>
            <span class="lg__kind" :class="{ 'ap-bad': BAD.has(r.e.kind) }">{{ kindLabel(r.e.kind) }}<span v-if="r.count > 1" class="lg__count tnum">×{{ r.count }}</span></span>
            <span class="lg__who">{{ r.e.actor?.name ?? t('system') }}</span>
            <span class="lg__details" :title="r.details">{{ r.details }}</span>
            <span class="lg__sub">{{ [r.e.actor?.name ?? t('system'), r.details].filter(Boolean).join(' · ') }}</span>
          </div>
        </template>
        <div v-if="cursor" class="ap-more"><UiButton variant="ghost" size="sm" :loading="more" @click="loadMore">{{ t('more') }}</UiButton></div>
      </div>
    </AdminState>
  </div>
</template>

<style scoped>
.lg { --cols: 56px minmax(150px, 0.9fr) minmax(100px, 0.6fr) minmax(0, 1.5fr); }
.lg .ap-tr { align-items: baseline; min-height: 44px; padding: 12px 0; }
.lg__kind { display: flex; align-items: baseline; gap: 8px; min-width: 0; font-weight: 600; }
.lg__count { font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); }
.lg__who { overflow: hidden; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.lg__details { overflow: hidden; color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
.lg__sub { display: none; }
@media (max-width: 899px) {
  .lg { --cols: 48px minmax(0, 1fr); }
  .lg .ap-tr { row-gap: 2px; }
  .lg__who, .lg__details { display: none; }
  .lg__sub { display: block; grid-column: 2; overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
}
@media (max-width: 699px) {
  .lg .ap-tr { row-gap: 0; padding: 10px 0; }
  .lg .ap-tr > :first-child { font-size: var(--fs-caption); }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Журнал",
    "filterKind": "Вид событий",
    "filterActor": "Кто",
    "allKinds": "Все события",
    "anyone": "Все люди",
    "empty": "Таких событий нет",
    "failed": "Не получилось загрузить журнал",
    "more": "Показать ещё",
    "system": "система",
    "today": "Сегодня",
    "yesterday": "Вчера",
    "group": { "auth": "Входы и выходы", "user": "Люди и роли", "session": "Сессии", "lane": "Полосы очереди", "job": "Задачи", "downloader": "Загрузчики", "acquisition": "Спорные", "recording": "Каталог", "source": "Проверки скачивания", "reco": "Рекомендации", "settings": "Настройки" },
    "kind": {
      "auth_sign_in": "Вход",
      "auth_sign_in_failed": "Неудачный вход",
      "auth_sign_in_denied": "Вход отклонён",
      "auth_logout": "Выход",
      "auth_app_code_reused": "Повтор кода приложения",
      "user_created": "Новый пользователь",
      "user_roles_changed": "Роли изменены",
      "user_disabled": "Пользователь отключён",
      "user_enabled": "Пользователь включён",
      "session_revoked": "Сессия завершена",
      "job_retried": "Задача повторена",
      "job_cancelled": "Задача отменена",
      "lane_changed": "Полоса изменена",
      "downloader_created": "Загрузчик добавлен",
      "downloader_revoked": "Загрузчик отозван",
      "acquisition_resolved": "Решение по спорной",
      "reco_settings_changed": "Настройки разведки",
      "reco_run": "Запуск вне расписания",
      "settings_changed": "Настройки сервера",
      "recording_reacquire": "Запись скачивается заново",
      "recording_reported": "Жалоба «Неверная запись»",
      "source_check": "Проверка скачивания",
      "device_created": "Новое устройство"
    },
    "d": {
      "password": "паролем",
      "noRoles": "без ролей",
      "byProvider": "по daly.rest",
      "paused": "пауза",
      "resumed": "запуск",
      "always": "круглосуточно",
      "concurrency": "одновременно {n}",
      "interval": "пауза {s} с",
      "queued": "задач: {n}",
      "action": { "choose": "выбрано видео", "url": "по ссылке", "unavailable": "нет в источниках", "retry": "искать заново" }
    }
  },
  "en": {
    "title": "Log",
    "filterKind": "Event kind",
    "filterActor": "Who",
    "allKinds": "All events",
    "anyone": "Everyone",
    "empty": "No such events",
    "failed": "Couldn't load the log",
    "more": "Show more",
    "system": "system",
    "today": "Today",
    "yesterday": "Yesterday",
    "group": { "auth": "Sign-ins and sign-outs", "user": "People and roles", "session": "Sessions", "lane": "Queue lanes", "job": "Jobs", "downloader": "Downloaders", "acquisition": "Disputed", "recording": "Catalog", "source": "Download tests", "reco": "Recommendations", "settings": "Settings" },
    "kind": {
      "auth_sign_in": "Sign-in",
      "auth_sign_in_failed": "Failed sign-in",
      "auth_sign_in_denied": "Sign-in denied",
      "auth_logout": "Sign-out",
      "auth_app_code_reused": "App code reused",
      "user_created": "New user",
      "user_roles_changed": "Roles changed",
      "user_disabled": "User disabled",
      "user_enabled": "User enabled",
      "session_revoked": "Session ended",
      "job_retried": "Job retried",
      "job_cancelled": "Job cancelled",
      "lane_changed": "Lane changed",
      "downloader_created": "Downloader added",
      "downloader_revoked": "Downloader revoked",
      "acquisition_resolved": "Disputed resolved",
      "reco_settings_changed": "Scouting settings",
      "reco_run": "Run out of schedule",
      "settings_changed": "Server settings",
      "recording_reacquire": "Recording re-downloading",
      "recording_reported": "Wrong recording reported",
      "source_check": "Download test",
      "device_created": "New device"
    },
    "d": {
      "password": "password",
      "noRoles": "no roles",
      "byProvider": "via daly.rest",
      "paused": "paused",
      "resumed": "resumed",
      "always": "around the clock",
      "concurrency": "{n} at once",
      "interval": "{s} s pause",
      "queued": "jobs: {n}",
      "action": { "choose": "video chosen", "url": "from a link", "unavailable": "not in sources", "retry": "search again" }
    }
  }
}
</i18n>
