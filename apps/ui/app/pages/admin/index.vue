<script setup lang="ts">
import type { AdminOverview, Health, Lane, RecoOverview, Sources } from '~/api/types'
import type { MeterPart } from '~/components/admin/Meter.vue'

// Обзор: что требует внимания (с действием в одно нажатие), сутки в цифрах, записи по состоянию,
// место и его цена, источники и рекомендации
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const L = useAdminLabels()
const { counts, refresh: refreshCounts } = useAdminCounts()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const health = ref<Health | null>(null)
const overview = ref<AdminOverview | null>(null)
const sources = ref<Sources | null>(null)
const reco = ref<RecoOverview | null>(null)
const { failed, attempt } = useAttempts()

// Загрузчик выключен на сервере: ни источников, ни «качать некому»
const server = useServerStore()
const loadOverview = () => attempt('overview', () => bridge.admin.overview(), v => (overview.value = v))
function loadAll() {
  void attempt('health', () => bridge.admin.health(), v => (health.value = v))
  void loadOverview()
  if (server.downloader) void attempt('sources', () => bridge.admin.sources(), v => (sources.value = v))
  void attempt('reco', () => bridge.admin.reco(), v => (reco.value = v))
  void refreshCounts()
}
onMounted(loadAll)
const refreshSoon = useDebounced(() => void loadOverview())
useAdminLive({ 'job.updated': refreshSoon, 'acquisition.updated': refreshSoon }, loadAll)

const uptime = computed(() => {
  const s = health.value?.uptimeS ?? 0
  if (s < 3600) return t('uptime.min', { n: Math.max(1, Math.round(s / 60)) })
  if (s < 86400) return t('uptime.h', { n: Math.round(s / 3600) })
  return t('uptime.d', { n: Math.round(s / 86400) })
})

// ---------- Что требует внимания ----------

interface Problem { key: string, icon: string, tone: 'warn' | 'bad', text: string, action: string, to?: string, run?: () => Promise<void> }
const online = computed(() => sources.value?.downloaders.filter(d => d.online && !d.revokedAt).length ?? 0)
const queuedDownloads = computed(() => overview.value?.recordingsByStatus.find(s => s.status === 'queued')?.count ?? 0)
async function resume(l: Lane) {
  try {
    await bridge.admin.updateLane(l.lane, { paused: false })
    toast.show({ text: t('problem.resumed', { title: l.title }), icon: 'ph:play-circle' })
    await loadOverview()
  }
  catch {
    toast.error(t('problem.failed'))
  }
}
const problems = computed<Problem[]>(() => {
  const out: Problem[] = []
  if (counts.value.disputed) out.push({ key: 'disputed', icon: 'ph:scales', tone: 'warn', text: t('problem.disputed', { n: counts.value.disputed }), action: t('problem.review'), to: '/admin/disputed' })
  if (counts.value.waiting) out.push({ key: 'waiting', icon: 'ph:user-plus', tone: 'warn', text: t('problem.waiting', { n: counts.value.waiting }), action: t('problem.open'), to: '/admin/access' })
  if (sources.value && !online.value && !sources.value.server.enabled && queuedDownloads.value) {
    out.push({ key: 'nobody', icon: 'ph:plugs', tone: 'bad', text: t('problem.nobody', { n: formatNumber(queuedDownloads.value, lang.value) }), action: t('problem.sources'), to: '/admin/sources' })
  }
  if (sources.value?.server.enabled && sources.value.server.reachable === false) out.push({ key: 'fetcher', icon: 'ph:cloud-warning', tone: 'bad', text: t('problem.fetcher'), action: t('problem.sources'), to: '/admin/sources' })
  const failedJobs = overview.value?.lanes.reduce((s, l) => s + l.failed, 0) ?? 0
  if (failedJobs) out.push({ key: 'jobs', icon: 'ph:warning-circle', tone: 'bad', text: t('problem.jobs', { n: failedJobs }), action: t('problem.open'), to: server.downloader ? '/admin/downloads?status=failed' : '/admin/settings' })
  for (const l of overview.value?.lanes ?? []) {
    if (l.paused && l.queued) out.push({ key: `lane-${l.lane}`, icon: 'ph:pause-circle', tone: 'warn', text: t('problem.paused', { title: l.title, n: formatNumber(l.queued, lang.value) }), action: t('problem.resume'), run: () => resume(l) })
  }
  if (reco.value && !reco.value.ml.reachable) out.push({ key: 'ml', icon: 'ph:waveform', tone: 'warn', text: t('problem.ml'), action: t('problem.open'), to: '/admin/reco' })
  return out
})
const running = ref<string | null>(null)
async function act(p: Problem) {
  if (p.to) return navigateTo(p.to)
  running.value = p.key
  await p.run?.()
  running.value = null
}

// ---------- Цифры ----------

const day = computed(() => overview.value?.last24h)
const stats = computed(() => {
  const d = day.value
  if (!d) return []
  return [
    { key: 'plays', value: d.plays },
    { key: 'likes', value: d.likes },
    { key: 'newTracks', value: d.newTracks },
    { key: 'acquired', value: d.acquisitionsReady },
    { key: 'acquireFailed', value: d.acquisitionsFailed, tone: d.acquisitionsFailed ? 'is-warn' : '' },
    { key: 'jobs', value: d.jobsDone, extra: d.jobsFailed ? t('stats.jobsFailed', { n: d.jobsFailed }) : null },
  ]
})
const recTotal = computed(() => overview.value?.recordingsByStatus.reduce((s, x) => s + x.count, 0) ?? 0)
const recParts = computed<MeterPart[]>(() => (overview.value?.recordingsByStatus ?? []).map(p => ({ key: p.status, label: L.audio(p.status), value: p.count, color: AUDIO_COLOR[p.status] })))
const storageTotal = computed(() => (overview.value ? overview.value.storage.audioBytes + overview.value.storage.imageBytes : 0))
const storageParts = computed<MeterPart[]>(() => {
  const s = overview.value?.storage
  if (!s) return []
  return [
    { key: 'audio', label: t('storage.audio'), value: s.audioBytes, display: formatBytes(s.audioBytes, lang.value), color: 'var(--accent)', sub: formatCount(s.audioFiles, 'file', lang.value) },
    { key: 'images', label: t('storage.images'), value: s.imageBytes, display: formatBytes(s.imageBytes, lang.value), color: 'var(--chart-2)', sub: formatCount(s.images, 'image', lang.value) },
  ]
})
const cost = computed(() => formatUsd(r2MonthlyUsd(storageTotal.value), lang.value))
const analysedShare = computed(() => {
  const r = reco.value
  const ready = overview.value?.recordingsByStatus.find(s => s.status === 'ready')?.count ?? 0
  if (!r || !ready) return null
  return Math.min(100, Math.round((r.index.full / ready) * 100))
})
const fetcherVersion = computed(() => {
  const d = sources.value?.downloaders.find(x => x.online && !x.revokedAt)
  return d?.ytDlpVersion ?? null
})
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <div class="ap-head__side ov__status">
        <AdminStatus v-if="health" tone="ok">{{ t('health.ok', { version: health.version, uptime }) }}</AdminStatus>
        <AdminStatus v-else-if="failed.health" tone="bad">{{ t('health.down') }}</AdminStatus>
        <AdminStatus :tone="bridge.live.state === 'open' ? 'ok' : 'idle'">{{ bridge.live.state === 'open' ? t('live.on') : t('live.off') }}</AdminStatus>
      </div>
    </header>

    <AdminSection :title="t('problem.title')">
      <template v-if="problems.length" #badge>
        <span class="ov__count tnum">{{ problems.length }}</span>
      </template>
      <AdminState :loading="!overview && !failed.overview" :error="failed.overview && !overview" :rows="2" @retry="loadAll">
        <div v-if="problems.length" class="ap-table">
          <div v-for="p in problems" :key="p.key" class="ap-tr ov__problem">
            <Icon :name="p.icon" class="ov__picon" :class="p.tone === 'bad' ? 'ap-bad' : 'ap-warn'" />
            <span class="ov__ptext">{{ p.text }}</span>
            <UiButton size="sm" :variant="p.run ? 'primary' : 'secondary'" :loading="running === p.key" :icon-right="p.to ? 'ph:arrow-right' : undefined" @click="act(p)">{{ p.action }}</UiButton>
          </div>
        </div>
        <p v-else class="ov__fine"><Icon name="ph:check-circle" class="ap-ok" />{{ t('problem.none') }}</p>
      </AdminState>
    </AdminSection>

    <AdminSection :title="t('stats.title')">
      <div v-if="stats.length" class="ap-stats">
        <div v-for="s in stats" :key="s.key" class="ap-stat" :class="s.tone">
          <span class="ap-stat__value">{{ formatNumber(s.value, lang) }}</span>
          <span class="ap-stat__label">{{ t(`stats.${s.key}`) }}</span>
          <span v-if="s.extra" class="ap-stat__extra ap-bad">{{ s.extra }}</span>
        </div>
      </div>
      <AdminState v-else :loading="!failed.overview" :error="failed.overview" :rows="2" @retry="loadOverview" />
    </AdminSection>

    <div v-if="overview" class="ap-cols">
      <AdminSection :title="t('rec.title')">
        <template #tools><span class="ov__total tnum">{{ formatNumber(recTotal, lang) }}</span></template>
        <AdminMeter :parts="recParts" :label="t('rec.title')" />
      </AdminSection>
      <AdminSection :title="t('storage.title')">
        <template #tools><span class="ov__total tnum">{{ formatBytes(storageTotal, lang) }}</span></template>
        <AdminMeter :parts="storageParts" :label="t('storage.title')" />
        <dl class="ap-kv">
          <div><dt>{{ t('storage.cost') }}</dt><dd>{{ cost }}</dd></div>
        </dl>
      </AdminSection>
      <AdminSection :title="t('lib.title')">
        <dl class="ap-kv">
          <div><dt>{{ t('lib.tracks') }}</dt><dd>{{ formatNumber(overview.library.tracks, lang) }}</dd></div>
          <div><dt>{{ t('lib.albums') }}</dt><dd>{{ formatNumber(overview.library.albums, lang) }}</dd></div>
          <div><dt>{{ t('lib.artists') }}</dt><dd>{{ formatNumber(overview.library.artists, lang) }}</dd></div>
          <div><dt>{{ t('lib.liked') }}</dt><dd>{{ formatNumber(overview.library.likedTracks, lang) }}</dd></div>
          <div><dt>{{ t('lib.followed') }}</dt><dd>{{ formatNumber(overview.library.followedArtists, lang) }}</dd></div>
          <div><dt>{{ t('lib.playlists') }}</dt><dd>{{ formatNumber(overview.library.playlists, lang) }}</dd></div>
        </dl>
      </AdminSection>
      <AdminSection :title="t('sys.title')">
        <dl class="ap-kv">
          <div v-if="server.downloader">
            <dt>{{ t('sys.downloaders') }}</dt>
            <dd>
              <template v-if="sources">{{ online ? t('sys.online', { n: online }) : t('sys.offline') }}<span v-if="fetcherVersion" class="ap-muted"> · yt-dlp {{ fetcherVersion }}</span></template>
              <template v-else>—</template>
            </dd>
          </div>
          <div v-if="sources?.server.enabled">
            <dt>{{ t('sys.server') }}</dt>
            <dd :class="sources.server.reachable ? 'ap-ok' : 'ap-bad'">{{ sources.server.reachable ? t('sys.works') : t('sys.down') }}</dd>
          </div>
          <div><dt>{{ t('sys.queue') }}</dt><dd>{{ formatNumber(overview.lanes.reduce((s, l) => s + l.queued, 0), lang) }}</dd></div>
          <div><dt>{{ t('sys.analysed') }}</dt><dd>{{ analysedShare != null ? `${analysedShare}%` : '—' }}</dd></div>
          <div><dt>{{ t('sys.index') }}</dt><dd>{{ reco ? formatNumber(reco.index.size, lang) : '—' }}</dd></div>
          <div>
            <dt>{{ t('sys.ml') }}</dt>
            <dd :class="reco ? (reco.ml.reachable ? 'ap-ok' : 'ap-bad') : ''">{{ reco ? (reco.ml.reachable ? t('sys.works') : t('sys.down')) : '—' }}</dd>
          </div>
          <div><dt>{{ t('sys.model') }}</dt><dd>{{ reco?.modelVersion ?? '—' }}</dd></div>
        </dl>
      </AdminSection>
    </div>
  </div>
</template>

<style scoped>
.ov__status { gap: 6px 20px; font-size: var(--fs-caption); }
.ov__count { display: inline-grid; place-items: center; min-width: 22px; height: 22px; padding: 0 6px; border-radius: var(--r-pill); background: color-mix(in oklab, var(--warning) 20%, transparent); font-size: 12px; font-weight: 700; color: var(--warning); }
.ov__problem { --cols: 20px minmax(0, 1fr) auto; min-height: 56px; }
.ov__picon { font-size: 20px; }
.ov__ptext { font-size: var(--fs-body); }
.ov__fine { display: flex; align-items: center; gap: 10px; margin: 0; min-height: 44px; color: var(--text-2); }
.ov__fine :deep(.iconify) { font-size: 20px; }
.ov__total { font-size: var(--fs-h4); font-weight: 700; }
@media (max-width: 1099px) {
  .ap-stats { --n: 3; row-gap: 20px; }
  .ap-stat:nth-child(4) { padding-left: 0; border-left: 0; }
}
@media (max-width: 699px) {
  .ov__status { width: 100%; }
  .ov__problem { --cols: 20px minmax(0, 1fr); }
  .ov__problem > :last-child { grid-column: 2; justify-self: start; }
  .ov__ptext { font-size: var(--fs-small); }
  .ap-stat:nth-child(4) { padding-left: 16px; border-left: 1px solid var(--hair); }
  .ap-stat:nth-child(odd) { padding-left: 0; border-left: 0; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Обзор",
    "health": { "ok": "Сервер работает · {version} · {uptime}", "down": "Сервер не отвечает" },
    "uptime": { "min": "{n} мин", "h": "{n} ч", "d": "{n} дн." },
    "live": { "on": "Живые события", "off": "Без живых событий" },
    "problem": {
      "title": "Требует внимания",
      "none": "Всё в порядке",
      "disputed": "Спорных записей: {n} — нужно выбрать, что скачать",
      "waiting": "Ждут доступа: {n}",
      "nobody": "Качать некому: загрузчик не на связи, в очереди {n}",
      "fetcher": "Серверный загрузчик не отвечает",
      "jobs": "Задач с ошибкой: {n}",
      "paused": "«{title}» на паузе, ждут {n}",
      "ml": "Сервис анализа звука не отвечает",
      "review": "Разобрать",
      "open": "Открыть",
      "sources": "Источники",
      "resume": "Запустить",
      "resumed": "«{title}» снова работает",
      "failed": "Не получилось — попробуйте ещё раз"
    },
    "stats": {
      "title": "За сутки",
      "plays": "прослушиваний",
      "likes": "лайков",
      "newTracks": "новых треков",
      "acquired": "скачано",
      "acquireFailed": "не скачалось",
      "jobs": "задач выполнено",
      "jobsFailed": "{n} с ошибкой"
    },
    "rec": { "title": "Записи по состоянию" },
    "storage": { "title": "Хранилище", "audio": "Звук", "images": "Обложки и фото", "cost": "Цена в R2 за месяц" },
    "lib": { "title": "Каталог и медиатека", "tracks": "Треков", "albums": "Альбомов", "artists": "Артистов", "liked": "Лайков", "followed": "Подписок", "playlists": "Плейлистов" },
    "sys": {
      "title": "Источники и рекомендации",
      "downloaders": "Загрузчики",
      "online": "на связи: {n}",
      "offline": "никого на связи",
      "server": "Серверный загрузчик",
      "works": "работает",
      "down": "не отвечает",
      "queue": "Задач в очереди",
      "analysed": "Скачанное проанализировано",
      "index": "Индекс каталога",
      "ml": "Анализ звука",
      "model": "Версия модели"
    }
  },
  "en": {
    "title": "Overview",
    "health": { "ok": "Server running · {version} · {uptime}", "down": "Server not responding" },
    "uptime": { "min": "{n} min", "h": "{n} h", "d": "{n} d" },
    "live": { "on": "Live events", "off": "No live events" },
    "problem": {
      "title": "Needs attention",
      "none": "All good",
      "disputed": "Disputed recordings: {n} — pick what to download",
      "waiting": "Waiting for access: {n}",
      "nobody": "No one to download: downloader offline, {n} queued",
      "fetcher": "Server downloader not responding",
      "jobs": "Failed jobs: {n}",
      "paused": "“{title}” paused, {n} waiting",
      "ml": "Audio analysis service not responding",
      "review": "Review",
      "open": "Open",
      "sources": "Sources",
      "resume": "Resume",
      "resumed": "“{title}” running again",
      "failed": "Didn't work — try again"
    },
    "stats": {
      "title": "Last 24 hours",
      "plays": "plays",
      "likes": "likes",
      "newTracks": "new tracks",
      "acquired": "downloaded",
      "acquireFailed": "failed downloads",
      "jobs": "jobs done",
      "jobsFailed": "{n} failed"
    },
    "rec": { "title": "Recordings by state" },
    "storage": { "title": "Storage", "audio": "Audio", "images": "Covers and photos", "cost": "R2 cost per month" },
    "lib": { "title": "Catalog and library", "tracks": "Tracks", "albums": "Albums", "artists": "Artists", "liked": "Likes", "followed": "Follows", "playlists": "Playlists" },
    "sys": {
      "title": "Sources and recommendations",
      "downloaders": "Downloaders",
      "online": "online: {n}",
      "offline": "none online",
      "server": "Server downloader",
      "works": "working",
      "down": "not responding",
      "queue": "Jobs queued",
      "analysed": "Downloads analysed",
      "index": "Catalog index",
      "ml": "Audio analysis",
      "model": "Model version"
    }
  }
}
</i18n>
