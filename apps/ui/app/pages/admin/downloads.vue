<script setup lang="ts">
import type { AcquisitionStatus, AdminAcquisition, Job, JobStatus, Lane } from '~/api/types'

// Загрузки: что качается прямо сейчас (живой шаг), полосы очереди с паузой, задачи с повтором
// и отменой, история скачиваний с кандидатами и проверкой
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const route = useRoute()
const router = useRouter()
const lang = useLang()
const L = useAdminLabels()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })
const { failed, attempt } = useAttempts()

const active = ref<AdminAcquisition[] | null>(null)
const lanes = ref<Lane[] | null>(null)
const jobs = ref<Job[] | null>(null)
const jobsCursor = ref<string | null>(null)
const history = ref<AdminAcquisition[] | null>(null)
const historyCursor = ref<string | null>(null)
const names = ref<Record<string, string>>({})

type JobFilter = 'all' | JobStatus
const JOB_FILTERS: JobFilter[] = ['all', 'failed', 'queued', 'running', 'done']
const jobFilter = ref<JobFilter>(JOB_FILTERS.includes(route.query.status as JobFilter) ? route.query.status as JobFilter : 'all')
const laneFilter = ref<string>(typeof route.query.lane === 'string' ? route.query.lane : '')
type HistoryFilter = 'all' | AcquisitionStatus
const historyFilter = ref<HistoryFilter>('all')
const laneTitles = computed(() => Object.fromEntries((lanes.value ?? []).map(l => [l.lane, l.title])))

const loadActive = () => attempt('active', () => bridge.admin.acquisitions({ active: true, limit: 200 }), p => (active.value = p.items))
// Идущие — первыми, за ними очередь по порядку; длинная очередь сворачивается
const ordered = computed(() => [...(active.value ?? [])].sort((x, y) => {
  const rx = x.acquisition.status === 'queued' ? 1 : 0
  const ry = y.acquisition.status === 'queued' ? 1 : 0
  return rx - ry || (x.acquisition.queuePosition ?? 1e9) - (y.acquisition.queuePosition ?? 1e9)
}))
const showAllActive = ref(false)
const shownActive = computed(() => (showAllActive.value ? ordered.value : ordered.value.slice(0, 10)))
const runningCount = computed(() => ordered.value.filter(a => a.acquisition.status !== 'queued').length)
const waiting = computed(() => ordered.value.some(a => a.acquisition.waitingForDownloader))
const loadLanes = () => attempt('lanes', () => bridge.admin.lanes(), l => (lanes.value = l.items))
const jobQuery = () => ({ status: jobFilter.value === 'all' ? undefined : jobFilter.value, lane: laneFilter.value || undefined, limit: 30 })
const loadJobs = () => attempt('jobs', () => bridge.admin.jobs(jobQuery()), (p) => {
  jobs.value = p.items
  jobsCursor.value = p.nextCursor
})
const historyQuery = () => (historyFilter.value === 'all' ? { active: false } : { status: historyFilter.value })
const loadHistory = () => attempt('history', () => bridge.admin.acquisitions({ ...historyQuery(), limit: 20 }), (p) => {
  history.value = p.items
  historyCursor.value = p.nextCursor
})
function loadAll() {
  void loadActive()
  void loadLanes()
  void loadJobs()
  void loadHistory()
  void bridge.admin.sources().then((s) => {
    names.value = Object.fromEntries(s.downloaders.map(d => [d.id, d.name]))
  }).catch(() => {})
}
onMounted(loadAll)

watch(jobFilter, (v) => {
  void router.replace({ query: { ...route.query, status: v === 'all' ? undefined : v } })
  void loadJobs()
})
watch(laneFilter, (v) => {
  void router.replace({ query: { ...route.query, lane: v || undefined } })
  void loadJobs()
})
watch(historyFilter, () => void loadHistory())

const moreJobs = ref(false)
async function loadMoreJobs() {
  if (!jobsCursor.value) return
  moreJobs.value = true
  await attempt('moreJobs', () => bridge.admin.jobs({ ...jobQuery(), cursor: jobsCursor.value }), (p) => {
    jobs.value = [...(jobs.value ?? []), ...p.items]
    jobsCursor.value = p.nextCursor
  })
  moreJobs.value = false
}
const moreHistory = ref(false)
async function loadMoreHistory() {
  if (!historyCursor.value) return
  moreHistory.value = true
  await attempt('moreHistory', () => bridge.admin.acquisitions({ ...historyQuery(), limit: 20, cursor: historyCursor.value }), (p) => {
    history.value = [...(history.value ?? []), ...p.items]
    historyCursor.value = p.nextCursor
  })
  moreHistory.value = false
}

// Живые события: шаг скачивания меняется на месте, остальное — одним обновлением на пачку
const refreshActive = useDebounced(() => {
  void loadActive()
  void loadLanes()
}, 700)
const refreshHistory = useDebounced(() => void loadHistory(), 2000)
useAdminLive({
  'acquisition.updated': (e) => {
    const item = active.value?.find(a => a.acquisition.id === e.id)
    if (item) {
      item.acquisition = { ...item.acquisition, status: e.status, audioStatus: e.audioStatus, error: e.error, updatedAt: new Date().toISOString() }
    }
    if (!ACQ_ACTIVE.includes(e.status)) refreshHistory()
    refreshActive()
  },
  'job.updated': (e) => {
    const list = jobs.value
    if (list) {
      const i = list.findIndex(j => j.id === e.id)
      if (i >= 0) list.splice(i, 1, { ...list[i]!, status: e.status, attempts: e.attempts, lastError: e.lastError, runAfter: e.runAfter, updatedAt: new Date().toISOString() })
    }
    refreshActive()
  },
}, loadAll)

const jobOptions = computed(() => JOB_FILTERS.map(v => ({ value: v, label: v === 'all' ? t('all') : L.jobStatus(v) })))
const laneOptions = computed(() => [{ value: '', label: t('jobs.anyLane') }, ...(lanes.value ?? []).map(l => ({ value: l.lane, label: l.title }))])
const historyOptions = computed(() => (['all', 'ready', 'disputed', 'unavailable', 'failed', 'cancelled'] as HistoryFilter[]).map(v => ({ value: v, label: v === 'all' ? t('all') : L.acqStatus(v) })))
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <AdminStatus class="dl__live" :tone="bridge.live.state === 'open' ? 'ok' : 'idle'">{{ bridge.live.state === 'open' ? t('liveOn') : t('liveOff') }}</AdminStatus>
    </header>

    <AdminSection :title="t('now.title')" :hint="active?.length ? t('now.summary', { running: runningCount, queued: `${formatNumber(ordered.length - runningCount, lang)}${ordered.length >= 200 ? '+' : ''}` }) : undefined">
      <AdminState :loading="!active && !failed.active" :error="failed.active && !active" :empty="!!active && !active.length" empty-icon="ph:check-circle" :empty-title="t('now.empty')" :rows="3" @retry="loadActive">
        <p v-if="waiting" class="dl__notice"><Icon name="ph:plugs" />{{ t('now.waiting') }}<NuxtLink to="/admin/sources" class="dl__link">{{ t('now.sources') }}</NuxtLink></p>
        <div class="ap-table">
          <AdminAcqRow header />
          <AdminAcqRow v-for="a in shownActive" :key="a.acquisition.id" :acq="a" :names="names" @changed="loadActive" />
          <div v-if="ordered.length > 10" class="ap-more">
            <UiButton variant="ghost" size="sm" @click="showAllActive = !showAllActive">{{ showAllActive ? t('now.less') : t('now.all', { n: formatNumber(ordered.length, lang) }) }}</UiButton>
          </div>
        </div>
      </AdminState>
    </AdminSection>

    <AdminSection :title="t('lanes.title')" :hint="t('lanes.hint')">
      <AdminState :loading="!lanes && !failed.lanes" :error="failed.lanes && !lanes" :rows="5" @retry="loadLanes">
        <AdminLaneTable v-if="lanes" :lanes="lanes" />
      </AdminState>
    </AdminSection>

    <AdminSection :title="t('jobs.title')">
      <template #tools>
        <AdminSelect v-model="laneFilter" :options="laneOptions" :label="t('jobs.lane')" align="end" />
        <UiSegmentedControl v-model="jobFilter" :options="jobOptions" :label="t('jobs.filter')" size="sm" />
      </template>
      <AdminState :loading="!jobs && !failed.jobs" :error="failed.jobs && !jobs" :empty="!!jobs && !jobs.length" empty-icon="ph:check-circle" :empty-title="t('jobs.empty')" @retry="loadJobs">
        <div class="ap-table">
          <AdminJobRow header />
          <AdminJobRow v-for="j in jobs" :key="j.id" :job="j" :lanes="laneTitles" :names="names" />
          <div v-if="jobsCursor" class="ap-more"><UiButton variant="ghost" size="sm" :loading="moreJobs" @click="loadMoreJobs">{{ t('more') }}</UiButton></div>
        </div>
      </AdminState>
    </AdminSection>

    <AdminSection :title="t('history.title')" :hint="t('history.hint')">
      <template #tools>
        <UiSegmentedControl v-model="historyFilter" :options="historyOptions" :label="t('history.filter')" size="sm" />
      </template>
      <AdminState :loading="!history && !failed.history" :error="failed.history && !history" :empty="!!history && !history.length" empty-icon="ph:clock-counter-clockwise" :empty-title="t('history.empty')" @retry="loadHistory">
        <div class="ap-table">
          <AdminAcqRow header />
          <AdminAcqRow v-for="a in history" :key="a.acquisition.id" :acq="a" :names="names" />
          <div v-if="historyCursor" class="ap-more"><UiButton variant="ghost" size="sm" :loading="moreHistory" @click="loadMoreHistory">{{ t('more') }}</UiButton></div>
        </div>
      </AdminState>
    </AdminSection>
    <span class="sr-only" aria-live="polite">{{ active ? t('now.count', { n: formatNumber(active.length, lang) }) : '' }}</span>
  </div>
</template>

<style scoped>
.dl__notice { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 0; padding: 10px 0; font-size: var(--fs-small); color: var(--warning); }
.dl__notice :deep(.iconify) { font-size: 18px; }
.dl__link { color: var(--text-1); font-weight: 600; text-decoration: underline; text-underline-offset: 3px; }
.dl__live { font-size: var(--fs-caption); }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Загрузки",
    "liveOn": "Обновляется вживую",
    "liveOff": "Без живых обновлений",
    "all": "Все",
    "more": "Показать ещё",
    "now": {
      "title": "Сейчас",
      "summary": "Качается {running} · ждут {queued}",
      "waiting": "Качать некому: загрузчик не на связи.",
      "sources": "Источники",
      "all": "Показать все {n}",
      "less": "Свернуть",
      "empty": "Сейчас ничего не качается",
      "count": "Качается: {n}"
    },
    "lanes": { "title": "Полосы", "hint": "Полосы работают независимо: пауза одной не задерживает остальные" },
    "jobs": { "title": "Задачи", "filter": "Какие задачи", "lane": "Полоса", "anyLane": "Все полосы", "empty": "Таких задач нет" },
    "history": { "title": "История скачиваний", "hint": "Чем закончились: кандидаты, оценки, проверка отпечатком", "filter": "Чем закончилось", "empty": "Пока ничего не скачивалось" }
  },
  "en": {
    "title": "Downloads",
    "liveOn": "Updating live",
    "liveOff": "No live updates",
    "all": "All",
    "more": "Show more",
    "now": {
      "title": "Now",
      "summary": "{running} downloading · {queued} waiting",
      "waiting": "No one to download: the downloader is offline.",
      "sources": "Sources",
      "all": "Show all {n}",
      "less": "Collapse",
      "empty": "Nothing is downloading right now",
      "count": "Downloading: {n}"
    },
    "lanes": { "title": "Lanes", "hint": "Lanes run independently: pausing one doesn't hold up the others" },
    "jobs": { "title": "Jobs", "filter": "Which jobs", "lane": "Lane", "anyLane": "All lanes", "empty": "No such jobs" },
    "history": { "title": "Download history", "hint": "How downloads ended: candidates, scores, fingerprint check", "filter": "Outcome", "empty": "Nothing has been downloaded yet" }
  }
}
</i18n>
