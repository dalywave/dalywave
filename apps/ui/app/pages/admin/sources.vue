<script setup lang="ts">
import type { AdminAcquisition, Downloader, Sources } from '~/api/types'
import type { BarDay, BarSeries } from '~/components/admin/Bars.vue'
import type { ViewPreset } from '~/components/admin/SavedViews.vue'
import type { MenuEntry } from '~/types/ui'
import { copyText } from '~/utils/web-api'

// Источники: домашние загрузчики (на связи, когда был, версии, работа за сутки, отзыв),
// серверный загрузчик, доля успеха по дням и «проверить скачивание» — трек вне очереди
// с живым ходом скачивания
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useScopedI18n('pages.admin.sources')
useHead({ title: () => t('title') })

const data = ref<Sources | null>(null)
const failed = ref(false)
async function load() {
  try {
    data.value = await bridge.admin.sources()
    failed.value = false
  }
  catch {
    failed.value = true
  }
}
onMounted(load)
// «На связи» — две минуты с последнего обращения: список освежается сам
useIntervalFn(() => void load(), 30_000)
const names = computed(() => Object.fromEntries((data.value?.downloaders ?? []).map(d => [d.id, d.name])))
// Загрузчики — общей таблицей админки: поиск, сортировка по колонкам, виды (на связи, рабочие,
// отозванные) и свои сохранённые
const downloaders = computed(() => data.value?.downloaders ?? [])
const live = computed(() => downloaders.value.filter(d => !d.revokedAt))
const seenAt = (d: Downloader) => (d.online ? Number.MAX_SAFE_INTEGER : d.lastSeenAt ? new Date(d.lastSeenAt).getTime() : 0)
const view = useTableView(downloaders, {
  search: d => [d.name, d.workerId, d.version, d.ytDlpVersion],
  sorts: { name: d => d.name.toLowerCase(), seen: seenAt, running: d => d.running, ok: d => d.ok24h, failed: d => d.failed24h },
  filters: { state: (d, v) => (v === 'online' ? d.online && !d.revokedAt : v === 'active' ? !d.revokedAt : v === 'revoked' ? !!d.revokedAt : true) },
  initial: { sort: 'seen', desc: true, filters: { state: 'active' } },
})
const BASE = { sort: 'seen', desc: true }
const viewPresets = computed<ViewPreset[]>(() => [
  { key: 'active', label: t('view.active'), state: { ...BASE, state: 'active' } },
  { key: 'online', label: t('view.online'), state: { ...BASE, state: 'online' } },
  { key: 'failing', label: t('view.failing'), state: { sort: 'failed', desc: true, state: 'active' } },
  { key: 'revoked', label: t('view.revoked'), state: { ...BASE, state: 'revoked' } },
  { key: 'all', label: t('view.all'), state: { ...BASE } },
])

// ---------- Проверка скачивания ----------

const check = ref<AdminAcquisition | null>(null)
const checking = ref(false)
async function runCheck() {
  checking.value = true
  try {
    check.value = await bridge.admin.checkDownload()
    toast.show({ text: t('check.started', { title: check.value.track?.title ?? '' }), icon: 'ph:download-simple' })
  }
  catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : t('failed'))
  }
  finally {
    checking.value = false
  }
}
const refreshCheck = useDebounced(async () => {
  if (!check.value) return
  check.value = await bridge.admin.acquisition(check.value.acquisition.id).catch(() => check.value)
  void load()
}, 400)
useAdminLive({ 'acquisition.updated': e => (e.id === check.value?.acquisition.id ? refreshCheck() : undefined) }, load)

// ---------- Загрузчики ----------

function seen(d: Downloader) {
  if (d.revokedAt) return t('dl.revokedAt', { when: formatRelative(d.revokedAt, lang.value) })
  if (d.online) return t('dl.online')
  return d.lastSeenAt ? t('dl.seen', { when: formatRelative(d.lastSeenAt, lang.value) }) : t('dl.never')
}
function abilities(d: Downloader) {
  const c = d.capabilities
  return [c.potProvider ? t('dl.pot') : null, c.cookies ? t('dl.cookies') : null].filter(Boolean).join(' · ')
}
async function revoke(d: Downloader) {
  try {
    await bridge.admin.revokeDownloader(d.id)
    toast.show({ text: t('dl.revoked', { name: d.name }), icon: 'ph:plugs' })
    await load()
  }
  catch {
    toast.error(t('failed'))
  }
}
const menu = (d: Downloader): MenuEntry[] => [
  { kind: 'heading', label: d.name, hint: d.workerId ?? undefined },
  { label: t('dl.revoke'), icon: 'ph:prohibit', danger: true, onSelect: () => revoke(d) },
]

const newName = ref('')
const creating = ref(false)
const token = ref<string | null>(null)
async function create() {
  const name = newName.value.trim()
  if (!name) return
  creating.value = true
  try {
    const res = await bridge.admin.createDownloader(name)
    token.value = res.token
    newName.value = ''
    await load()
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    creating.value = false
  }
}
async function copyToken() {
  if (!token.value) return
  await copyText(token.value).then(() => toast.show({ text: t('dl.copied'), icon: 'ph:copy' }), () => toast.error(t('failed')))
}

// ---------- Серверный загрузчик ----------

const server = computed(() => data.value?.server)
const health = computed(() => (server.value?.health ?? null) as Record<string, unknown> | null)
const tools = computed(() => (health.value?.tools ?? {}) as Record<string, unknown>)
const pot = computed(() => (health.value?.pot ?? null) as { reachable?: boolean } | null)
const cookies = computed(() => (health.value?.cookies ?? null) as { present?: boolean, expiresAt?: string | null } | null)

// ---------- По дням ----------

const SERIES = computed<BarSeries[]>(() => [
  { key: 'ok', label: t('days.ok'), color: 'var(--success)' },
  { key: 'disputed', label: t('days.disputed'), color: 'var(--warning)' },
  { key: 'unavailable', label: t('days.unavailable'), color: 'rgba(var(--fg-rgb), 0.4)' },
  { key: 'failed', label: t('days.failed'), color: 'var(--danger)' },
])
const days = computed<BarDay[]>(() => (data.value?.daily ?? []).map(d => ({ day: d.day, values: { ok: d.ok, disputed: d.disputed, unavailable: d.unavailable, failed: d.failed } })))
const rate = computed(() => {
  const all = data.value?.daily.reduce((s, d) => s + d.ok + d.disputed + d.unavailable + d.failed, 0) ?? 0
  const ok = data.value?.daily.reduce((s, d) => s + d.ok, 0) ?? 0
  return all ? Math.round((ok / all) * 100) : null
})
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <UiButton variant="primary" icon="ph:play-circle" :loading="checking" @click="runCheck">{{ t('check.button') }}</UiButton>
    </header>

    <AdminSection v-if="check" :title="t('check.title')" :hint="t('check.hint')">
      <div class="ap-table">
        <AdminAcqRow :acq="check" :names="names" />
      </div>
    </AdminSection>

    <AdminSection :title="t('dl.title')" :hint="t('dl.hint')">
      <AdminState :loading="!data && !failed" :error="failed && !data" :rows="3" @retry="load">
        <template v-if="downloaders.length">
          <AdminSavedViews storage-key="downloaders" :presets="viewPresets" :state="view.state" @apply="view.apply" />
          <div class="ap-filters src__filters">
            <input v-model="view.q" class="ap-input src__search" type="search" :placeholder="t('dl.search')" :aria-label="t('dl.search')">
          </div>
        </template>
        <div v-if="view.shown.length" class="ap-table src__table" role="table" :aria-label="t('dl.title')">
          <div class="ap-tr ap-tr--head" role="row">
            <AdminSortHead :label="t('dl.name')" k="name" :view="view" text />
            <AdminSortHead :label="t('dl.state')" k="seen" :view="view" />
            <span class="ap-hide-phone" role="columnheader">{{ t('dl.versions') }}</span>
            <AdminSortHead :label="t('dl.running')" k="running" :view="view" num />
            <AdminSortHead :label="t('dl.ok')" k="ok" :view="view" num />
            <AdminSortHead :label="t('dl.failed')" k="failed" :view="view" num />
            <span />
          </div>
          <div v-for="d in view.shown" :key="d.id" class="ap-tr" :class="{ 'src__rev': d.revokedAt }" role="row">
            <span class="ap-cell"><b>{{ d.name }}</b><small>{{ d.workerId ?? t('dl.noMachine') }}</small></span>
            <AdminStatus :tone="d.revokedAt ? 'idle' : d.online ? 'ok' : 'idle'">{{ seen(d) }}</AdminStatus>
            <span class="ap-cell ap-hide-phone">
              <b>{{ d.ytDlpVersion ? `yt-dlp ${d.ytDlpVersion}` : '—' }}</b>
              <small>{{ [d.version ? `fetcher ${d.version}` : null, abilities(d)].filter(Boolean).join(' · ') }}</small>
            </span>
            <span class="ap-num" :class="{ 'ap-on': d.running }">{{ d.running || '—' }}</span>
            <span class="ap-num">{{ d.ok24h || '—' }}</span>
            <span class="ap-num" :class="{ 'ap-bad': d.failed24h }">{{ d.failed24h || '—' }}</span>
            <span class="ap-end">
              <UiMenu v-if="!d.revokedAt" :items="menu(d)"><UiIconButton icon="ph:dots-three" :label="t('dl.actions')" size="sm" :tooltip="false" /></UiMenu>
            </span>
          </div>
        </div>
        <p v-else-if="downloaders.length" class="ap-hint src__empty">{{ t('dl.nothing') }}</p>
        <div v-if="!live.length" class="src__none">
          <Icon name="ph:plugs" />
          <span class="ap-cell"><b>{{ t('dl.none') }}</b><small>{{ t('dl.noneHint') }}</small></span>
        </div>
        <form class="src__new" @submit.prevent="create">
          <input v-model="newName" class="ap-input" maxlength="60" :placeholder="t('dl.newPlaceholder')" :aria-label="t('dl.newLabel')">
          <UiButton type="submit" icon="ph:plus" :disabled="!newName.trim()" :loading="creating">{{ t('dl.add') }}</UiButton>
        </form>
        <div v-if="token" class="src__token">
          <p class="ap-hint">{{ t('dl.tokenHint') }}</p>
          <div class="src__token-row">
            <code class="ap-mono">{{ token }}</code>
            <UiButton size="sm" icon="ph:copy" @click="copyToken">{{ t('dl.copy') }}</UiButton>
            <UiIconButton icon="ph:x" :label="t('dl.hideToken')" size="sm" @click="token = null" />
          </div>
        </div>
      </AdminState>
    </AdminSection>

    <div class="ap-cols">
      <AdminSection :title="t('days.title')">
        <template #tools><span v-if="rate != null" class="src__rate tnum">{{ t('days.rate', { p: rate }) }}</span></template>
        <AdminBars v-if="days.length" :days="days" :series="SERIES" :label="t('days.title')" />
        <AdminState v-else :loading="!failed" :error="failed" :rows="3" @retry="load" />
      </AdminSection>

      <AdminSection :title="t('server.title')">
        <dl v-if="server" class="ap-kv">
          <div>
            <dt>{{ t('server.state') }}</dt>
            <dd :class="!server.enabled ? 'ap-muted' : server.reachable ? 'ap-ok' : 'ap-bad'">{{ !server.enabled ? t('server.off') : server.reachable ? t('server.works') : t('server.down') }}</dd>
          </div>
          <template v-if="server.enabled">
            <div v-if="server.error"><dt>{{ t('server.error') }}</dt><dd class="ap-bad src__err">{{ server.error }}</dd></div>
            <div v-if="tools['yt-dlp'] || tools.ytDlp"><dt>yt-dlp</dt><dd>{{ tools['yt-dlp'] ?? tools.ytDlp }}</dd></div>
            <div v-if="pot"><dt>{{ t('server.pot') }}</dt><dd :class="pot.reachable ? 'ap-ok' : 'ap-bad'">{{ pot.reachable ? t('server.works') : t('server.down') }}</dd></div>
            <div v-if="cookies"><dt>{{ t('server.cookies') }}</dt><dd>{{ cookies.present === false ? t('server.noCookies') : cookies.expiresAt ? t('server.until', { when: formatShortDate(cookies.expiresAt, lang) }) : t('server.yes') }}</dd></div>
            <div><dt>{{ t('server.running') }}</dt><dd>{{ server.running }}</dd></div>
            <div><dt>{{ t('server.ok') }}</dt><dd>{{ server.ok24h }}</dd></div>
            <div><dt>{{ t('server.failed') }}</dt><dd :class="{ 'ap-bad': server.failed24h }">{{ server.failed24h }}</dd></div>
          </template>
        </dl>
        <p v-if="server && !server.enabled" class="ap-hint">{{ t('server.offHint') }}</p>
      </AdminSection>
    </div>
  </div>
</template>

<style scoped>
.src__filters { margin-bottom: 0; }
.src__search { flex: 1 1 220px; max-width: 320px; }
.src__empty { padding: 12px 0; }
.src__rev { opacity: 0.6; }
.src__table { --cols: minmax(140px, 1.2fr) minmax(130px, 1fr) minmax(150px, 1.2fr) 64px 64px 64px 40px; }
.src__none { display: flex; align-items: center; gap: 14px; min-height: 64px; padding: 8px 0; border-bottom: 1px solid var(--hair); font-size: var(--fs-small); }
.src__none > .iconify { flex: none; font-size: 22px; color: var(--text-3); }
.src__new { display: flex; gap: 8px; max-width: 520px; padding-top: 12px; }
.src__new .ap-input { flex: 1; }
.src__token { display: flex; flex-direction: column; gap: 8px; padding: 12px 0; }
.src__token-row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.src__token code { padding: 8px 10px; border-radius: var(--r-2); background: var(--fill-1); color: var(--text-1); word-break: break-all; user-select: all; }
.src__revoked { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; padding-top: 8px; margin-left: -12px; }
.src__revoked .ap-table { align-self: stretch; margin-left: 12px; font-size: var(--fs-small); color: var(--text-2); }
.src__rev { --cols: minmax(0, 1fr) auto; }
.src__rate { font-size: var(--fs-small); font-weight: 600; color: var(--text-2); }
.src__err { max-width: 60%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
@media (max-width: 899px) {
  .src__table { --cols: minmax(0, 1.3fr) minmax(0, 1fr) 40px 40px 40px 36px; }
}
@media (max-width: 699px) {
  .src__table { --cols: minmax(0, 1fr) minmax(0, 0.9fr) 36px; }
  .src__table .ap-num { display: none; }
  .src__new .ap-input { height: 44px; }
}
</style>
