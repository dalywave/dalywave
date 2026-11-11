<script setup lang="ts">
import type { AdminOverview, AdminUser, AudioStatus, AuditEvent, Health, Job, JobStatus, Lane, Role, SessionInfo } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Админка — обзор: здоровье сервера, сутки в цифрах, записи по состояниям, очереди задач,
// последние задачи (живьём по WebSocket), доступ людей, сессии и журнал
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const health = ref<Health | null>(null)
const overview = ref<AdminOverview | null>(null)
const lanes = ref<Lane[] | null>(null)
const jobs = ref<Job[] | null>(null)
const users = ref<AdminUser[] | null>(null)
const sessions = ref<SessionInfo[] | null>(null)
const audit = ref<AuditEvent[] | null>(null)
const auditCursor = ref<string | null>(null)
const failed = ref<Record<string, boolean>>({})
const jobFilter = ref<'all' | JobStatus>('all')

async function attempt<T>(key: string, job: () => Promise<T>, apply: (v: T) => void) {
  try {
    apply(await job())
    failed.value = { ...failed.value, [key]: false }
  }
  catch {
    failed.value = { ...failed.value, [key]: true }
  }
}

const loadJobs = () => attempt('jobs', () => bridge.admin.jobs({ status: jobFilter.value === 'all' ? undefined : jobFilter.value, limit: 30 }), p => (jobs.value = p.items))
const loadOverview = () => attempt('overview', () => bridge.admin.overview(), (o) => {
  overview.value = o
  lanes.value = o.lanes
})

function loadAll() {
  void attempt('health', () => bridge.admin.health(), h => (health.value = h))
  void loadOverview()
  void loadJobs()
  void attempt('users', () => bridge.admin.users(), l => (users.value = [...l.items].sort((a, b) => Number(a.roles.length > 0) - Number(b.roles.length > 0))))
  void attempt('sessions', () => bridge.admin.sessions(), l => (sessions.value = l.items))
  void attempt('audit', () => bridge.admin.audit(null, 30), (p) => {
    audit.value = p.items
    auditCursor.value = p.nextCursor
  })
}

onMounted(() => {
  loadAll()
  bridge.live.connect()
})
watch(jobFilter, () => void loadJobs())

// Живые события: задачи меняют состояние, загрузки двигают цифры
let refresh: ReturnType<typeof setTimeout> | undefined
function refreshSoon() {
  clearTimeout(refresh)
  refresh = setTimeout(() => void loadOverview(), 800)
}
const offs = [
  bridge.live.on('job.updated', (e) => {
    const list = jobs.value
    if (list) {
      const i = list.findIndex(j => j.id === e.id)
      if (i >= 0) list.splice(i, 1, { ...list[i]!, status: e.status, attempts: e.attempts, lastError: e.lastError, runAfter: e.runAfter, updatedAt: new Date().toISOString() })
    }
    refreshSoon()
  }),
  bridge.live.on('acquisition.updated', refreshSoon),
  bridge.live.onReconnect(loadAll),
]
onBeforeUnmount(() => offs.forEach(off => off()))

// ---------- Обзор ----------

const day = computed(() => overview.value?.last24h)
const tiles = computed(() => {
  const d = day.value
  if (!d) return []
  return [
    { key: 'plays', icon: 'ph:play-circle', value: d.plays },
    { key: 'likes', icon: 'ph:heart', value: d.likes },
    { key: 'newTracks', icon: 'ph:music-notes-plus', value: d.newTracks },
    { key: 'acquired', icon: 'ph:download-simple', value: d.acquisitionsReady },
    { key: 'acquireFailed', icon: 'ph:warning-circle', value: d.acquisitionsFailed, bad: d.acquisitionsFailed > 0 },
    { key: 'jobs', icon: 'ph:list-checks', value: d.jobsDone, extra: d.jobsFailed ? t('tiles.jobsFailed', { n: d.jobsFailed }) : null, bad: d.jobsFailed > 0 },
  ]
})

const STATUS_COLOR: Record<AudioStatus, string> = {
  ready: 'var(--success)',
  queued: '#8ed8f5',
  acquiring: 'var(--accent)',
  none: 'rgba(232, 236, 244, 0.22)',
  disputed: 'var(--warning)',
  unavailable: 'rgba(232, 236, 244, 0.45)',
  failed: 'var(--danger)',
}
const recTotal = computed(() => overview.value?.recordingsByStatus.reduce((s, x) => s + x.count, 0) ?? 0)
const recParts = computed(() => (overview.value?.recordingsByStatus ?? []).filter(x => x.count > 0))
const uptime = computed(() => {
  const s = health.value?.uptimeS ?? 0
  if (s < 3600) return t('uptime.min', { n: Math.max(1, Math.round(s / 60)) })
  if (s < 86400) return t('uptime.h', { n: Math.round(s / 3600) })
  return t('uptime.d', { n: Math.round(s / 86400) })
})

// ---------- Очереди ----------

async function toggleLane(l: Lane, paused: boolean) {
  const was = l.paused
  l.paused = paused
  try {
    await bridge.admin.updateLane(l.lane, { paused })
    toast.show({ text: paused ? t('lanes.paused', { title: l.title }) : t('lanes.resumed', { title: l.title }), icon: paused ? 'ph:pause-circle' : 'ph:play-circle', undo: () => void toggleLane(l, was) })
  }
  catch {
    l.paused = was
    toast.error(t('lanes.changeFailed'))
  }
}

// ---------- Задачи ----------

const JOB_KINDS: Record<string, string> = { acquire: 'jobs.kind.acquire', sync_discography: 'jobs.kind.discography', mirror_image: 'jobs.kind.mirror', refresh_artist: 'jobs.kind.refresh' }
const jobKind = (k: string) => (JOB_KINDS[k] ? t(JOB_KINDS[k]!) : k)
const JOB_TONE: Record<JobStatus, string> = { queued: 'meta', running: 'new', done: 'success', failed: 'danger', cancelled: 'meta' }
function jobSubject(j: Job) {
  const p = j.payload as Record<string, unknown>
  const owner = typeof p.owner === 'string' ? (p.owner === 'album' ? t('jobs.album') : t('jobs.artist')) : null
  const id = typeof p.id === 'string' ? p.id : typeof p.trackId === 'string' ? p.trackId : typeof p.artistId === 'string' ? p.artistId : null
  return [owner, id ? id.slice(-8) : null].filter(Boolean).join(' · ')
}
async function retryJob(j: Job) {
  try {
    await bridge.admin.retryJob(j.id)
    j.status = 'queued'
    toast.show({ text: t('jobs.retried'), icon: 'ph:arrow-clockwise' })
  }
  catch {
    toast.error(t('jobs.actionFailed'))
  }
}
async function cancelJob(j: Job) {
  try {
    await bridge.admin.cancelJob(j.id)
    j.status = 'cancelled'
    toast.show({ text: t('jobs.cancelled'), icon: 'ph:x-circle' })
  }
  catch {
    toast.error(t('jobs.actionFailed'))
  }
}

// ---------- Доступ ----------

const waiting = computed(() => users.value?.filter(u => !u.roles.length && !u.disabled) ?? [])
async function setRoles(u: AdminUser, roles: Role[], text: string) {
  const was = u.roles
  u.roles = roles
  try {
    await bridge.admin.updateUser(u.id, { roles })
    toast.show({ text, icon: 'ph:user-check', undo: () => void setRoles(u, was, t('access.restored')) })
  }
  catch {
    u.roles = was
    toast.error(t('access.failed'))
  }
}
async function setDisabled(u: AdminUser, disabled: boolean) {
  const was = u.disabled
  u.disabled = disabled
  try {
    await bridge.admin.updateUser(u.id, { disabled })
    toast.show({ text: disabled ? t('access.disabledDone', { name: u.name }) : t('access.enabledDone', { name: u.name }), icon: disabled ? 'ph:user-minus' : 'ph:user-plus', undo: () => void setDisabled(u, was) })
  }
  catch {
    u.disabled = was
    toast.error(t('access.failed'))
  }
}
function userMenu(u: AdminUser): MenuEntry[] {
  const isAdmin = u.roles.includes('admin')
  const self = u.id === auth.user?.id
  return [
    { kind: 'heading', label: u.name, hint: u.email ?? undefined },
    isAdmin
      ? { label: t('access.removeAdmin'), icon: 'ph:shield-slash', disabled: u.idpAdmin || self, hint: u.idpAdmin ? t('access.idpAdmin') : self ? t('access.self') : undefined, onSelect: () => setRoles(u, u.roles.filter(r => r !== 'admin'), t('access.adminRemoved', { name: u.name })) }
      : { label: t('access.makeAdmin'), icon: 'ph:shield-check', onSelect: () => setRoles(u, [...new Set<Role>([...u.roles, 'admin'])], t('access.adminGiven', { name: u.name })) },
    u.roles.includes('listener')
      ? { label: t('access.removeListener'), icon: 'ph:headphones', disabled: self, onSelect: () => setRoles(u, u.roles.filter(r => r !== 'listener'), t('access.listenerRemoved', { name: u.name })) }
      : { label: t('access.makeListener'), icon: 'ph:headphones', onSelect: () => setRoles(u, [...u.roles, 'listener'], t('access.listenerGiven', { name: u.name })) },
    { kind: 'separator' },
    { label: u.disabled ? t('access.enable') : t('access.disable'), icon: u.disabled ? 'ph:user-plus' : 'ph:user-minus', danger: !u.disabled, disabled: self, onSelect: () => setDisabled(u, !u.disabled) },
  ]
}

// ---------- Сессии ----------

async function revoke(s: SessionInfo) {
  try {
    await bridge.admin.revokeSession(s.id)
    sessions.value = sessions.value?.filter(x => x.id !== s.id) ?? null
    toast.show({ text: t('sessions.revoked', { name: s.userName }), icon: 'ph:sign-out' })
  }
  catch {
    toast.error(t('sessions.failed'))
  }
}
function agent(s: SessionInfo) {
  if (s.device) return `${s.device.name} · ${s.device.platform}${s.device.appVersion ? ` ${s.device.appVersion}` : ''}`
  const ua = s.userAgent ?? ''
  const browser = /Edg\//.test(ua) ? 'Edge' : /Chrome\//.test(ua) ? 'Chrome' : /Firefox\//.test(ua) ? 'Firefox' : /Safari\//.test(ua) ? 'Safari' : t('sessions.browser')
  const os = /Mac OS X/.test(ua) ? 'macOS' : /Windows/.test(ua) ? 'Windows' : /iPhone|iPad/.test(ua) ? 'iOS' : /Android/.test(ua) ? 'Android' : /Linux/.test(ua) ? 'Linux' : ''
  return [browser, os].filter(Boolean).join(' · ')
}

// ---------- Журнал ----------

const AUDIT: Record<string, string> = {
  'auth.sign_in': 'audit.signIn',
  'auth.sign_out': 'audit.signOut',
  'user.created': 'audit.userCreated',
  'user.roles_changed': 'audit.rolesChanged',
  'user.disabled': 'audit.userDisabled',
  'user.enabled': 'audit.userEnabled',
  'session.revoked': 'audit.sessionRevoked',
  'lane.changed': 'audit.laneChanged',
  'job.retried': 'audit.jobRetried',
  'job.cancelled': 'audit.jobCancelled',
  'device.created': 'audit.deviceCreated',
}
const auditKind = (k: string) => (AUDIT[k] ? t(AUDIT[k]!) : k)
function auditDetails(e: AuditEvent) {
  const p = e.payload as Record<string, unknown>
  const parts: string[] = []
  if (typeof p.via === 'string') parts.push(p.via === 'password' ? t('audit.viaPassword') : p.via === 'oidc' ? 'daly.rest' : String(p.via))
  if (Array.isArray(p.roles)) parts.push(p.roles.join(', ') || t('audit.noRoles'))
  if (Array.isArray(p.to)) parts.push(`${(Array.isArray(p.from) && p.from.join(', ')) || t('audit.noRoles')} → ${p.to.join(', ') || t('audit.noRoles')}`)
  if (p.reason === 'provider') parts.push(t('audit.byProvider'))
  if (typeof p.platform === 'string') parts.push(p.platform)
  if (typeof p.email === 'string') parts.push(p.email)
  if (typeof p.paused === 'boolean') parts.push(p.paused ? t('audit.pausedTrue') : t('audit.pausedFalse'))
  if (typeof p.lane === 'string') parts.push(p.lane)
  if (typeof p.ip === 'string') parts.push(p.ip)
  return parts.join(' · ')
}
async function moreAudit() {
  if (!auditCursor.value) return
  const p = await bridge.admin.audit(auditCursor.value, 30).catch(() => null)
  if (!p) return toast.error(t('audit.failed'))
  audit.value = [...(audit.value ?? []), ...p.items]
  auditCursor.value = p.nextCursor
}
</script>

<template>
  <div class="ad">
    <!-- Обзор -->
    <section id="overview" class="ad__sec">
      <header class="ad__head">
        <h1 class="ad__title">{{ t('title') }}</h1>
        <div class="ad__status">
          <span v-if="health" class="pill pill--ok"><i />{{ t('health.ok', { version: health.version, uptime }) }}</span>
          <span v-else-if="failed.health" class="pill pill--bad"><i />{{ t('health.down') }}</span>
          <span class="pill" :class="bridge.live.state === 'open' ? 'pill--live' : 'pill--off'"><i />{{ bridge.live.state === 'open' ? t('live.on') : t('live.off') }}</span>
        </div>
      </header>

      <div v-if="tiles.length" class="tiles">
        <div v-for="tl in tiles" :key="tl.key" class="tile" :class="{ 'tile--bad': tl.bad }">
          <Icon :name="tl.icon" class="tile__icon" />
          <span class="tile__value tnum">{{ formatNumber(tl.value, lang) }}</span>
          <span class="tile__label">{{ t(`tiles.${tl.key}`) }}</span>
          <span v-if="tl.extra" class="tile__extra">{{ tl.extra }}</span>
        </div>
      </div>
      <div v-else-if="!failed.overview" class="tiles"><UiSkeleton v-for="n in 6" :key="n" height="104px" /></div>
      <p class="ad__caption">{{ t('tiles.caption') }}</p>

      <div v-if="overview" class="ad__grid">
        <div class="card">
          <h3 class="card__title">{{ t('rec.title') }}</h3>
          <div class="rec__total tnum">{{ formatNumber(recTotal, lang) }} <span>{{ t('rec.records') }}</span></div>
          <div class="rec__bar" role="img" :aria-label="recParts.map(p => `${t(`rec.status.${p.status}`)}: ${p.count}`).join(', ')">
            <i v-for="p in recParts" :key="p.status" :style="{ flexGrow: p.count, background: STATUS_COLOR[p.status] }" />
          </div>
          <ul class="rec__legend">
            <li v-for="p in overview.recordingsByStatus" :key="p.status" :class="{ 'is-zero': !p.count }">
              <i :style="{ background: STATUS_COLOR[p.status] }" />
              <span>{{ t(`rec.status.${p.status}`) }}</span>
              <b class="tnum">{{ formatNumber(p.count, lang) }}</b>
            </li>
          </ul>
        </div>
        <div class="card">
          <h3 class="card__title">{{ t('lib.title') }}</h3>
          <dl class="kv">
            <dt>{{ t('lib.tracks') }}</dt><dd class="tnum">{{ formatNumber(overview.library.tracks, lang) }}</dd>
            <dt>{{ t('lib.recordings') }}</dt><dd class="tnum">{{ formatNumber(overview.library.recordings, lang) }}</dd>
            <dt>{{ t('lib.albums') }}</dt><dd class="tnum">{{ formatNumber(overview.library.albums, lang) }}</dd>
            <dt>{{ t('lib.artists') }}</dt><dd class="tnum">{{ formatNumber(overview.library.artists, lang) }}</dd>
            <dt>{{ t('lib.liked') }}</dt><dd class="tnum">{{ formatNumber(overview.library.likedTracks, lang) }}</dd>
            <dt>{{ t('lib.followed') }}</dt><dd class="tnum">{{ formatNumber(overview.library.followedArtists, lang) }}</dd>
            <dt>{{ t('lib.playlists') }}</dt><dd class="tnum">{{ formatNumber(overview.library.playlists, lang) }}</dd>
          </dl>
        </div>
        <div class="card">
          <h3 class="card__title">{{ t('storage.title') }}</h3>
          <dl class="kv">
            <dt>{{ t('storage.audio') }}</dt><dd class="tnum">{{ formatBytes(overview.storage.audioBytes, lang) }} · {{ formatNumber(overview.storage.audioFiles, lang) }} {{ t('storage.files') }}</dd>
            <dt>{{ t('storage.images') }}</dt><dd class="tnum">{{ formatBytes(overview.storage.imageBytes, lang) }} · {{ formatNumber(overview.storage.images, lang) }} {{ t('storage.covers') }}</dd>
          </dl>
          <p class="card__note">{{ t('storage.note') }}</p>
        </div>
      </div>
      <UiEmptyState v-else-if="failed.overview" icon="ph:cloud-warning" :title="t('failed')" tone="error" compact>
        <UiButton icon="ph:arrow-clockwise" @click="loadOverview">{{ t('retry') }}</UiButton>
      </UiEmptyState>
    </section>

    <!-- Очереди -->
    <section id="lanes" class="ad__sec">
      <header class="ad__sechead">
        <h2 class="ad__h2">{{ t('lanes.title') }}</h2>
        <p class="ad__hint">{{ t('lanes.hint') }}</p>
      </header>
      <div v-if="lanes" class="card card--flush">
        <div class="lanes" role="table" :aria-label="t('lanes.title')">
          <div class="lanes__row lanes__row--head" role="row">
            <span role="columnheader">{{ t('lanes.lane') }}</span>
            <span role="columnheader" class="num">{{ t('lanes.queued') }}</span>
            <span role="columnheader" class="num">{{ t('lanes.running') }}</span>
            <span role="columnheader" class="num">{{ t('lanes.failed') }}</span>
            <span role="columnheader" class="num">{{ t('lanes.done') }}</span>
            <span role="columnheader" class="num">{{ t('lanes.concurrency') }}</span>
            <span role="columnheader">{{ t('lanes.state') }}</span>
          </div>
          <div v-for="l in lanes" :key="l.lane" class="lanes__row" :class="{ 'is-paused': l.paused }" role="row">
            <span role="cell" class="lanes__name"><b>{{ l.title }}</b><code>{{ l.lane }}</code></span>
            <span role="cell" class="num tnum">{{ l.queued || '—' }}</span>
            <span role="cell" class="num tnum" :class="{ 'is-on': l.running }">{{ l.running || '—' }}</span>
            <span role="cell" class="num tnum" :class="{ 'is-bad': l.failed }">{{ l.failed || '—' }}</span>
            <span role="cell" class="num tnum">{{ l.done24h || '—' }}</span>
            <span role="cell" class="num tnum">{{ l.concurrency }}<small v-if="l.minIntervalMs"> · {{ (l.minIntervalMs / 1000).toLocaleString(lang) }} с</small></span>
            <span role="cell" class="lanes__state">
              <UiSwitch :model-value="!l.paused" :label="l.paused ? t('lanes.pausedLabel') : t('lanes.runningLabel')" @update:model-value="v => toggleLane(l, !v)" />
            </span>
          </div>
        </div>
      </div>
      <div v-else-if="!failed.overview" class="card"><UiSkeleton v-for="n in 4" :key="n" variant="line" height="18px" style="margin: 12px 0" /></div>
    </section>

    <!-- Задачи -->
    <section id="jobs" class="ad__sec">
      <header class="ad__sechead ad__sechead--row">
        <div>
          <h2 class="ad__h2">{{ t('jobs.title') }}</h2>
          <p class="ad__hint">{{ t('jobs.hint') }}</p>
        </div>
        <UiSegmentedControl
          v-model="jobFilter"
          :options="[{ value: 'all', label: t('jobs.all') }, { value: 'failed', label: t('jobs.status.failed') }, { value: 'queued', label: t('jobs.status.queued') }, { value: 'running', label: t('jobs.status.running') }]"
          :label="t('jobs.filter')"
          size="sm"
        />
      </header>
      <div v-if="jobs?.length" class="card card--flush">
        <div v-for="j in jobs" :key="j.id" class="job">
          <UiBadge :variant="JOB_TONE[j.status] as 'meta'" class="job__status">{{ t(`jobs.status.${j.status}`) }}</UiBadge>
          <div class="job__main">
            <div class="job__title">{{ jobKind(j.kind) }} <span class="job__subject">{{ jobSubject(j) }}</span></div>
            <div class="job__meta">
              <code>{{ j.lane }}</code>
              <span>{{ t('jobs.attempts', { n: j.attempts, max: j.maxAttempts }) }}</span>
              <span>{{ formatRelative(j.updatedAt, lang) }}</span>
            </div>
            <div v-if="j.lastError" class="job__error" :title="j.lastError">{{ j.lastError }}</div>
          </div>
          <div class="job__actions">
            <UiButton v-if="j.status === 'failed' || j.status === 'cancelled'" size="sm" icon="ph:arrow-clockwise" @click="retryJob(j)">{{ t('jobs.retry') }}</UiButton>
            <UiButton v-else-if="j.status === 'queued' || j.status === 'running'" size="sm" variant="ghost" icon="ph:x" @click="cancelJob(j)">{{ t('jobs.cancel') }}</UiButton>
          </div>
        </div>
      </div>
      <UiEmptyState v-else-if="jobs" icon="ph:check-circle" :title="t('jobs.empty')" compact />
      <UiEmptyState v-else-if="failed.jobs" icon="ph:cloud-warning" :title="t('failed')" tone="error" compact>
        <UiButton icon="ph:arrow-clockwise" @click="loadJobs">{{ t('retry') }}</UiButton>
      </UiEmptyState>
    </section>

    <!-- Доступ -->
    <section id="access" class="ad__sec">
      <header class="ad__sechead">
        <h2 class="ad__h2">{{ t('access.title') }}<UiBadge v-if="waiting.length" variant="new" class="ad__count">{{ t('access.waiting', { n: waiting.length }) }}</UiBadge></h2>
        <p class="ad__hint">{{ t('access.hint') }}</p>
      </header>
      <div v-if="users" class="card card--flush">
        <div v-for="u in users" :key="u.id" class="user" :class="{ 'is-waiting': !u.roles.length && !u.disabled, 'is-disabled': u.disabled }">
          <span class="user__avatar">
            <img v-if="u.avatarUrl" :src="u.avatarUrl" alt="">
            <span v-else>{{ u.name.charAt(0).toUpperCase() }}</span>
          </span>
          <div class="user__main">
            <div class="user__name">{{ u.name }}<UiBadge v-if="u.id === auth.user?.id" variant="meta">{{ t('access.you') }}</UiBadge></div>
            <div class="user__meta">
              <span>{{ u.email ?? t('access.noEmail') }}</span>
              <span>{{ u.issuer ? 'daly.rest' : t('access.local') }}</span>
              <span v-if="u.lastLoginAt">{{ t('access.lastLogin', { when: formatRelative(u.lastLoginAt, lang) }) }}</span>
            </div>
          </div>
          <div class="user__roles">
            <UiBadge v-if="u.disabled" variant="danger">{{ t('access.disabled') }}</UiBadge>
            <template v-else-if="u.roles.length">
              <UiBadge v-for="r in u.roles" :key="r" :variant="r === 'admin' ? 'new' : 'meta'" :icon="r === 'admin' && u.idpAdmin ? 'ph:link-simple' : undefined">{{ t(`access.role.${r}`) }}</UiBadge>
            </template>
            <UiButton v-else variant="primary" size="sm" icon="ph:user-check" @click="setRoles(u, ['listener'], t('access.listenerGiven', { name: u.name }))">{{ t('access.give') }}</UiButton>
          </div>
          <UiMenu :items="userMenu(u)">
            <UiIconButton icon="ph:dots-three" :label="t('access.actions')" size="sm" :tooltip="false" />
          </UiMenu>
        </div>
      </div>
      <UiEmptyState v-else-if="failed.users" icon="ph:cloud-warning" :title="t('failed')" tone="error" compact />
    </section>

    <!-- Сессии -->
    <section id="sessions" class="ad__sec">
      <header class="ad__sechead">
        <h2 class="ad__h2">{{ t('sessions.title') }}</h2>
        <p class="ad__hint">{{ t('sessions.hint') }}</p>
      </header>
      <div v-if="sessions?.length" class="card card--flush">
        <div v-for="s in sessions" :key="s.id" class="sess">
          <span class="sess__icon"><Icon :name="s.kind === 'device' ? 'ph:laptop' : 'ph:browser'" /></span>
          <div class="sess__main">
            <div class="sess__name">{{ s.userName }}<UiBadge v-if="s.current" variant="success">{{ t('sessions.current') }}</UiBadge></div>
            <div class="sess__meta">
              <span>{{ agent(s) }}</span>
              <span>{{ s.via === 'password' ? t('sessions.viaPassword') : s.via === 'device' ? t('sessions.viaDevice') : 'daly.rest' }}</span>
              <span v-if="s.ip">{{ s.ip }}</span>
              <span>{{ t('sessions.seen', { when: formatRelative(s.lastSeenAt, lang) }) }}</span>
            </div>
          </div>
          <UiButton v-if="!s.current" size="sm" variant="ghost" icon="ph:sign-out" @click="revoke(s)">{{ t('sessions.revoke') }}</UiButton>
        </div>
      </div>
      <UiEmptyState v-else-if="sessions" icon="ph:devices" :title="t('sessions.empty')" compact />
    </section>

    <!-- Журнал -->
    <section id="audit" class="ad__sec">
      <header class="ad__sechead">
        <h2 class="ad__h2">{{ t('audit.title') }}</h2>
        <p class="ad__hint">{{ t('audit.hint') }}</p>
      </header>
      <div v-if="audit?.length" class="card card--flush">
        <div v-for="e in audit" :key="e.id" class="log">
          <span class="log__time tnum">{{ formatDateTime(e.at, lang) }}</span>
          <span class="log__kind">{{ auditKind(e.kind) }}</span>
          <span class="log__who">{{ e.actor?.name ?? t('audit.system') }}</span>
          <span class="log__details">{{ auditDetails(e) }}</span>
        </div>
        <div v-if="auditCursor" class="log__more"><UiButton variant="ghost" size="sm" @click="moreAudit">{{ t('audit.more') }}</UiButton></div>
      </div>
      <UiEmptyState v-else-if="audit" icon="ph:scroll" :title="t('audit.empty')" compact />
    </section>
  </div>
</template>

<style scoped>
.ad { display: flex; flex-direction: column; gap: 48px; max-width: 1160px; padding: 32px 32px 80px; }
.ad__sec { display: flex; flex-direction: column; gap: 16px; scroll-margin-top: 16px; }
.ad__head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 16px; }
.ad__title { margin: 0; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); }
.ad__status { display: flex; flex-wrap: wrap; gap: 8px; }
.pill { display: inline-flex; align-items: center; gap: 8px; height: 30px; padding: 0 12px; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); font-size: var(--fs-caption); font-weight: 600; color: var(--text-2); }
.pill i { width: 7px; height: 7px; border-radius: 50%; background: var(--text-4); }
.pill--ok i, .pill--live i { background: var(--success); box-shadow: 0 0 8px var(--success); }
.pill--live i { animation: pulse 2s var(--ease-in-out) infinite; }
.pill--bad { color: var(--danger); }
.pill--bad i { background: var(--danger); }
@keyframes pulse { 50% { opacity: 0.4; } }
.ad__sechead { display: flex; flex-direction: column; gap: 2px; }
.ad__sechead--row { flex-direction: row; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 12px; }
.ad__h2 { display: flex; align-items: center; gap: 10px; margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.ad__hint, .ad__caption { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.ad__caption { margin-top: -6px; }
.tiles { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 12px; }
.tile { position: relative; display: flex; flex-direction: column; gap: 2px; min-width: 0; padding: 16px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.tile__icon { font-size: 20px; color: var(--text-3); margin-bottom: 6px; }
.tile__value { font-size: var(--fs-h2); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.1; }
.tile__label { font-size: var(--fs-small); color: var(--text-2); }
.tile__extra { font-size: var(--fs-caption); color: var(--danger); }
.tile--bad .tile__icon { color: var(--danger); }
.ad__grid { display: grid; grid-template-columns: 1.3fr 1fr 1fr; gap: 12px; }
.card { display: flex; flex-direction: column; gap: 10px; min-width: 0; padding: 18px 20px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.card--flush { gap: 0; padding: 4px 0; }
.card__title { margin: 0; font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.card__note { margin: auto 0 0; font-size: var(--fs-caption); color: var(--text-3); }
.rec__total { font-size: var(--fs-h2); font-weight: 700; letter-spacing: var(--tr-title); }
.rec__total span { font-size: var(--fs-small); font-weight: 500; color: var(--text-2); }
.rec__bar { display: flex; gap: 2px; height: 10px; overflow: hidden; border-radius: 5px; background: var(--fill-2); }
.rec__bar i { display: block; min-width: 4px; height: 100%; }
.rec__legend { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 16px; margin: 4px 0 0; padding: 0; list-style: none; font-size: var(--fs-small); }
.rec__legend li { display: flex; align-items: center; gap: 8px; min-width: 0; }
.rec__legend li.is-zero { color: var(--text-4); }
.rec__legend i { width: 8px; height: 8px; flex: none; border-radius: 2px; }
.rec__legend b { margin-left: auto; font-weight: 600; }
.kv { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px 16px; margin: 0; font-size: var(--fs-small); }
.kv dt { color: var(--text-2); }
.kv dd { margin: 0; font-weight: 600; text-align: right; }
.lanes__row { display: grid; grid-template-columns: minmax(180px, 2fr) repeat(5, minmax(60px, 0.6fr)) minmax(150px, 1.2fr); align-items: center; gap: 12px; min-height: 52px; padding: 6px 20px; border-top: 1px solid var(--hair); font-size: var(--fs-small); }
.lanes__row--head { min-height: 40px; border-top: 0; font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); }
.lanes__row.is-paused .lanes__name b { color: var(--text-2); }
.lanes__name { display: flex; flex-direction: column; min-width: 0; }
.lanes__name b { font-weight: 600; }
.lanes__name code, .job__meta code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; color: var(--text-3); }
.num { text-align: right; }
.num small { color: var(--text-3); }
.is-on { color: var(--accent); font-weight: 600; }
.is-bad { color: var(--danger); font-weight: 600; }
.lanes__state { display: flex; justify-content: flex-end; }
.job, .user, .sess { display: flex; align-items: center; gap: 14px; padding: 12px 20px; border-top: 1px solid var(--hair); }
.job:first-child, .user:first-child, .sess:first-child, .log:first-child { border-top: 0; }
.job__status { flex: none; min-width: 92px; justify-content: center; }
.job__main, .user__main, .sess__main { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.job__title, .user__name, .sess__name { display: flex; align-items: center; gap: 8px; font-weight: 600; }
.job__subject { font-weight: 450; color: var(--text-3); font-variant-numeric: tabular-nums; }
.job__meta, .user__meta, .sess__meta { display: flex; flex-wrap: wrap; gap: 4px 12px; font-size: var(--fs-caption); color: var(--text-3); }
.job__error { overflow: hidden; font-size: var(--fs-caption); color: var(--danger); text-overflow: ellipsis; white-space: nowrap; }
.job__actions { flex: none; }
.user.is-waiting { background: color-mix(in oklab, var(--accent) 7%, transparent); }
.user.is-disabled { opacity: 0.6; }
.user__avatar { display: grid; place-items: center; width: 40px; height: 40px; flex: none; overflow: hidden; border-radius: 50%; background: radial-gradient(120% 120% at 25% 15%, #4b5568, #222834); font-weight: 700; }
.user__avatar img { width: 100%; height: 100%; object-fit: cover; }
.user__roles { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; }
.sess__icon { display: grid; place-items: center; width: 36px; height: 36px; flex: none; border-radius: 10px; background: var(--fill-2); color: var(--text-2); font-size: 18px; }
.log { display: grid; grid-template-columns: 130px minmax(150px, 0.9fr) minmax(100px, 0.6fr) minmax(0, 1.4fr); gap: 12px; align-items: baseline; padding: 10px 20px; border-top: 1px solid var(--hair); font-size: var(--fs-small); }
.log__time { color: var(--text-3); }
.log__kind { font-weight: 600; }
.log__who { color: var(--text-2); }
.log__details { overflow: hidden; color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
.log__more { display: flex; justify-content: center; padding: 8px; border-top: 1px solid var(--hair); }
.ad__count { margin-left: 2px; }
@media (max-width: 1199px) {
  .tiles { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .ad__grid { grid-template-columns: 1fr 1fr; }
}
@media (max-width: 899px) {
  .lanes__row { grid-template-columns: minmax(140px, 1.6fr) repeat(3, minmax(48px, 0.6fr)) minmax(120px, 1fr); }
  .lanes__row > :nth-child(5), .lanes__row > :nth-child(6) { display: none; }
  .log { grid-template-columns: 110px minmax(0, 1fr); }
  .log__who, .log__details { grid-column: 2; }
}
@media (max-width: 699px) {
  .ad { gap: 36px; padding: 16px 16px 64px; }
  .ad__title { font-size: 28px; }
  .tiles { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .ad__grid { grid-template-columns: minmax(0, 1fr); }
  .lanes__row { grid-template-columns: minmax(0, 1fr) 56px auto; padding: 6px 14px; }
  .lanes__row > :nth-child(3), .lanes__row > :nth-child(4) { display: none; }
  .job, .user, .sess { flex-wrap: wrap; padding: 12px 14px; }
  .job__status { min-width: 0; }
  .user__roles { width: 100%; justify-content: flex-start; padding-left: 54px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Обзор",
    "failed": "Не загрузилось",
    "retry": "Повторить",
    "health": { "ok": "Сервер работает · {version} · {uptime}", "down": "Сервер не отвечает" },
    "uptime": { "min": "{n} мин", "h": "{n} ч", "d": "{n} дн." },
    "live": { "on": "Живые события", "off": "Без живых событий" },
    "tiles": {
      "plays": "прослушиваний",
      "likes": "лайков",
      "newTracks": "новых треков",
      "acquired": "скачано",
      "acquireFailed": "не скачалось",
      "jobs": "задач выполнено",
      "jobsFailed": "{n} с ошибкой",
      "caption": "За последние сутки"
    },
    "rec": {
      "title": "Записи по состоянию",
      "records": "записей",
      "status": { "ready": "На сервере", "queued": "В очереди", "acquiring": "Скачиваются", "none": "Не скачаны", "disputed": "Спорные", "unavailable": "Нет в источниках", "failed": "Ошибка" }
    },
    "lib": { "title": "Каталог и медиатека", "tracks": "Треков", "recordings": "Записей", "albums": "Альбомов", "artists": "Артистов", "liked": "Лайков", "followed": "Подписок", "playlists": "Плейлистов" },
    "storage": { "title": "Хранилище", "audio": "Звук", "images": "Обложки и фото", "files": "файлов", "covers": "картинок", "note": "Обложки копируются в своё хранилище — без них интерфейс не зависит от Deezer" },
    "lanes": {
      "title": "Очереди задач",
      "hint": "Полосы работают независимо: пауза одной не задерживает остальные",
      "lane": "Полоса",
      "queued": "Ждут",
      "running": "Идут",
      "failed": "Ошибки",
      "done": "За сутки",
      "concurrency": "Потоков",
      "state": "Работает",
      "pausedLabel": "На паузе",
      "runningLabel": "Работает",
      "paused": "«{title}» на паузе",
      "resumed": "«{title}» снова работает",
      "changeFailed": "Не получилось изменить очередь"
    },
    "jobs": {
      "title": "Задачи",
      "hint": "Обновляются сами, пока страница открыта",
      "filter": "Какие задачи",
      "all": "Все",
      "empty": "Задач нет",
      "attempts": "попытка {n} из {max}",
      "retry": "Повторить",
      "cancel": "Отменить",
      "retried": "Задача снова в очереди",
      "cancelled": "Задача отменена",
      "actionFailed": "Не получилось — задача уже изменилась, обновите страницу",
      "album": "альбом",
      "artist": "артист",
      "status": { "queued": "В очереди", "running": "Идёт", "done": "Готово", "failed": "Ошибка", "cancelled": "Отменена" },
      "kind": { "acquire": "Скачивание записи", "discography": "Дискография", "mirror": "Копия обложки", "refresh": "Обновление артиста" }
    },
    "access": {
      "title": "Доступ",
      "hint": "Новые люди входят через daly.rest и ждут, пока им выдадут роль. Администраторы daly.rest — администраторы и здесь",
      "waiting": "ждут: {n}",
      "you": "это вы",
      "noEmail": "почта не указана",
      "local": "локальный вход",
      "lastLogin": "входил {when}",
      "disabled": "Отключён",
      "give": "Выдать доступ",
      "actions": "Действия",
      "role": { "admin": "Администратор", "listener": "Слушатель" },
      "makeAdmin": "Сделать администратором",
      "removeAdmin": "Снять роль администратора",
      "idpAdmin": "выдана daly.rest",
      "self": "это вы",
      "makeListener": "Выдать роль слушателя",
      "removeListener": "Снять роль слушателя",
      "disable": "Отключить",
      "enable": "Включить",
      "adminGiven": "{name} теперь администратор",
      "adminRemoved": "{name} больше не администратор",
      "listenerGiven": "{name} получил доступ к музыке",
      "listenerRemoved": "{name} больше не слушатель",
      "disabledDone": "{name} отключён: все его сессии закрыты",
      "enabledDone": "{name} снова может входить",
      "restored": "Роли возвращены",
      "failed": "Не получилось изменить доступ — попробуйте ещё раз"
    },
    "sessions": {
      "title": "Сессии",
      "hint": "Вкладки сайта и приложения, где выполнен вход",
      "current": "эта сессия",
      "browser": "Браузер",
      "viaPassword": "аварийный вход",
      "viaDevice": "приложение",
      "seen": "активна {when}",
      "revoke": "Завершить",
      "revoked": "Сессия {name} завершена",
      "failed": "Не получилось завершить сессию",
      "empty": "Открытых сессий нет"
    },
    "audit": {
      "title": "Журнал",
      "hint": "Входы, смены ролей, отзывы сессий и изменения очередей",
      "system": "система",
      "more": "Показать ещё",
      "empty": "Журнал пуст",
      "failed": "Не получилось загрузить журнал",
      "viaPassword": "паролем",
      "noRoles": "без ролей",
      "byProvider": "по daly.rest",
      "pausedTrue": "пауза",
      "pausedFalse": "запуск",
      "signIn": "Вход",
      "signOut": "Выход",
      "userCreated": "Новый пользователь",
      "rolesChanged": "Роли изменены",
      "userDisabled": "Пользователь отключён",
      "userEnabled": "Пользователь включён",
      "sessionRevoked": "Сессия завершена",
      "laneChanged": "Очередь изменена",
      "jobRetried": "Задача повторена",
      "jobCancelled": "Задача отменена",
      "deviceCreated": "Новое устройство"
    }
  },
  "en": {
    "title": "Overview",
    "failed": "Didn't load",
    "retry": "Retry",
    "health": { "ok": "Server running · {version} · {uptime}", "down": "Server not responding" },
    "uptime": { "min": "{n} min", "h": "{n} h", "d": "{n} d" },
    "live": { "on": "Live events", "off": "No live events" },
    "tiles": {
      "plays": "plays",
      "likes": "likes",
      "newTracks": "new tracks",
      "acquired": "downloaded",
      "acquireFailed": "failed downloads",
      "jobs": "jobs done",
      "jobsFailed": "{n} failed",
      "caption": "Last 24 hours"
    },
    "rec": {
      "title": "Recordings by state",
      "records": "recordings",
      "status": { "ready": "On the server", "queued": "Queued", "acquiring": "Downloading", "none": "Not downloaded", "disputed": "Disputed", "unavailable": "Not in sources", "failed": "Failed" }
    },
    "lib": { "title": "Catalog and library", "tracks": "Tracks", "recordings": "Recordings", "albums": "Albums", "artists": "Artists", "liked": "Likes", "followed": "Follows", "playlists": "Playlists" },
    "storage": { "title": "Storage", "audio": "Audio", "images": "Covers and photos", "files": "files", "covers": "images", "note": "Covers are copied to our own storage — the UI doesn't depend on Deezer" },
    "lanes": {
      "title": "Job lanes",
      "hint": "Lanes run independently: pausing one doesn't hold up the others",
      "lane": "Lane",
      "queued": "Queued",
      "running": "Running",
      "failed": "Failed",
      "done": "24 h",
      "concurrency": "Workers",
      "state": "Running",
      "pausedLabel": "Paused",
      "runningLabel": "Running",
      "paused": "“{title}” paused",
      "resumed": "“{title}” running again",
      "changeFailed": "Couldn't change the lane"
    },
    "jobs": {
      "title": "Jobs",
      "hint": "Updates live while the page is open",
      "filter": "Which jobs",
      "all": "All",
      "empty": "No jobs",
      "attempts": "attempt {n} of {max}",
      "retry": "Retry",
      "cancel": "Cancel",
      "retried": "Job queued again",
      "cancelled": "Job cancelled",
      "actionFailed": "Didn't work — the job already changed, reload the page",
      "album": "album",
      "artist": "artist",
      "status": { "queued": "Queued", "running": "Running", "done": "Done", "failed": "Failed", "cancelled": "Cancelled" },
      "kind": { "acquire": "Recording download", "discography": "Discography", "mirror": "Cover copy", "refresh": "Artist refresh" }
    },
    "access": {
      "title": "Access",
      "hint": "New people sign in with daly.rest and wait for a role. daly.rest admins are admins here too",
      "waiting": "waiting: {n}",
      "you": "you",
      "noEmail": "no email",
      "local": "local sign-in",
      "lastLogin": "signed in {when}",
      "disabled": "Disabled",
      "give": "Give access",
      "actions": "Actions",
      "role": { "admin": "Admin", "listener": "Listener" },
      "makeAdmin": "Make admin",
      "removeAdmin": "Remove admin role",
      "idpAdmin": "granted by daly.rest",
      "self": "that's you",
      "makeListener": "Give listener role",
      "removeListener": "Remove listener role",
      "disable": "Disable",
      "enable": "Enable",
      "adminGiven": "{name} is now an admin",
      "adminRemoved": "{name} is no longer an admin",
      "listenerGiven": "{name} can now listen",
      "listenerRemoved": "{name} is no longer a listener",
      "disabledDone": "{name} disabled: all sessions closed",
      "enabledDone": "{name} can sign in again",
      "restored": "Roles restored",
      "failed": "Couldn't change access — try again"
    },
    "sessions": {
      "title": "Sessions",
      "hint": "Website tabs and apps that are signed in",
      "current": "this session",
      "browser": "Browser",
      "viaPassword": "emergency sign-in",
      "viaDevice": "app",
      "seen": "active {when}",
      "revoke": "End",
      "revoked": "Session of {name} ended",
      "failed": "Couldn't end the session",
      "empty": "No open sessions"
    },
    "audit": {
      "title": "Log",
      "hint": "Sign-ins, role changes, revoked sessions and lane changes",
      "system": "system",
      "more": "Show more",
      "empty": "The log is empty",
      "failed": "Couldn't load the log",
      "viaPassword": "password",
      "noRoles": "no roles",
      "byProvider": "via daly.rest",
      "pausedTrue": "paused",
      "pausedFalse": "resumed",
      "signIn": "Sign-in",
      "signOut": "Sign-out",
      "userCreated": "New user",
      "rolesChanged": "Roles changed",
      "userDisabled": "User disabled",
      "userEnabled": "User enabled",
      "sessionRevoked": "Session ended",
      "laneChanged": "Lane changed",
      "jobRetried": "Job retried",
      "jobCancelled": "Job cancelled",
      "deviceCreated": "New device"
    }
  }
}
</i18n>
