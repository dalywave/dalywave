<script setup lang="ts">
import type { AdminHealth, HealthCheck } from '~/api/types'

// Здоровье: каждая часть сервера с состоянием и временем ответа, очередь, упавшие за сутки задачи
// с переходом к ним. Обновляется раз в 30 с и по живым событиям очереди
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const lang = useLang()
const L = useAdminLabels()
const { t } = useScopedI18n('pages.admin.health')
useHead({ title: () => t('title') })

const data = ref<AdminHealth | null>(null)
const failed = ref(false)
async function load() {
  try {
    data.value = await bridge.admin.serverHealth()
    failed.value = false
  }
  catch {
    failed.value = true
  }
}
const soon = useDebounced(() => void load(), 1500)
useAdminLive({ 'job.updated': soon }, soon)
useIntervalFn(() => void load(), 30_000)
onMounted(load)

const TONE: Record<HealthCheck['state'], StatusTone> = { ok: 'ok', warn: 'warn', down: 'bad', off: 'idle' }
// Куда идти, если часть не в порядке
const LINK: Record<string, string> = { worker: '/admin/downloads', ml: '/admin/reco', fetcher: '/admin/sources', backups: '/admin/backups', storage: '/admin/storage' }
function detail(c: HealthCheck) {
  if (c.state === 'off') return t('off')
  if ((c.key === 'worker' || c.key === 'backups') && c.detail) return t(`detail.${c.key}`, { when: formatRelative(c.detail, lang.value) })
  if (c.key === 'backups' && c.state === 'warn') return t('detail.noBackups')
  if (c.state === 'ok') return c.latencyMs != null ? t('latency', { ms: c.latencyMs }) : t('ok')
  return c.detail ?? t('down')
}
const uptime = computed(() => {
  const s = data.value?.uptimeS ?? 0
  if (s < 3600) return t('uptime.min', { n: Math.max(1, Math.round(s / 60)) })
  if (s < 86400) return t('uptime.h', { n: Math.round(s / 3600) })
  return t('uptime.d', { n: Math.round(s / 86400) })
})
const bad = computed(() => data.value?.checks.filter(c => c.state === 'down' || c.state === 'warn').length ?? 0)
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <div v-if="data" class="ap-head__side">
        <AdminStatus :tone="bad ? 'warn' : 'ok'">{{ bad ? t('problems', { n: bad }) : t('allGood') }}</AdminStatus>
      </div>
    </header>

    <AdminState :loading="!data && !failed" :error="failed && !data" :rows="6" @retry="load">
      <template v-if="data">
        <div class="ap-stats" style="--n: 5">
          <div class="ap-stat"><span class="ap-stat__value">{{ data.version }}</span><span class="ap-stat__label">{{ t('stats.version', { uptime }) }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value tnum">{{ formatBytes(data.dbBytes, lang) }}</span><span class="ap-stat__label">{{ t('stats.db') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value tnum">{{ formatNumber(data.connections, lang) }}</span><span class="ap-stat__label">{{ t('stats.connections') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value tnum">{{ formatNumber(data.queued, lang) }}</span><span class="ap-stat__label">{{ t('stats.queue', { n: data.running }) }}</span></div>
          <div class="ap-stat" :class="{ 'is-bad': data.failed24h }"><span class="ap-stat__value tnum">{{ formatNumber(data.failed24h, lang) }}</span><span class="ap-stat__label">{{ t('stats.failed') }}</span></div>
        </div>

        <AdminSection :title="t('checks')">
          <div class="ap-table">
            <div v-for="c in data.checks" :key="c.key" class="ap-tr he__row">
              <span class="ap-cell"><b>{{ t(`part.${c.key}`) }}</b><small>{{ t(`about.${c.key}`) }}</small></span>
              <AdminStatus :tone="TONE[c.state]">{{ t(`state.${c.state}`) }}</AdminStatus>
              <span class="ap-muted he__detail" :class="{ 'ap-bad': c.state === 'down' }">{{ detail(c) }}</span>
              <span class="ap-end">
                <UiButton v-if="(c.state === 'down' || c.state === 'warn') && LINK[c.key]" size="sm" variant="secondary" icon-right="ph:arrow-right" :to="LINK[c.key]">{{ t('open') }}</UiButton>
              </span>
            </div>
          </div>
        </AdminSection>

        <AdminSection :title="t('failures.title')" :hint="t('failures.hint')">
          <template v-if="data.failures.length" #tools>
            <UiButton size="sm" variant="secondary" icon-right="ph:arrow-right" to="/admin/downloads?status=failed">{{ t('failures.all') }}</UiButton>
          </template>
          <p v-if="!data.failures.length" class="he__fine"><Icon name="ph:check-circle" class="ap-ok" />{{ t('failures.none') }}</p>
          <div v-else class="ap-table">
            <div v-for="f in data.failures" :key="f.jobId" class="ap-tr he__fail">
              <span class="ap-cell"><b>{{ L.jobKind(f.kind) }}</b><small>{{ f.lane }} · {{ formatRelative(f.at, lang) }}</small></span>
              <span class="ap-bad he__error">{{ f.error ?? '—' }}</span>
            </div>
          </div>
        </AdminSection>
      </template>
    </AdminState>
  </div>
</template>

<style scoped>
.he__row { --cols: minmax(180px, 1.3fr) 140px minmax(0, 1.4fr) 120px; min-height: 60px; }
.he__detail { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.he__fail { --cols: minmax(160px, 0.8fr) minmax(0, 2fr); }
.he__error { overflow: hidden; font-size: var(--fs-caption); text-overflow: ellipsis; white-space: nowrap; }
.he__fine { display: flex; align-items: center; gap: 10px; margin: 0; min-height: 44px; color: var(--text-2); }
.he__fine :deep(.iconify) { font-size: 20px; }
@media (max-width: 1099px) {
  .ap-stats { --n: 3 !important; row-gap: 20px; }
  .ap-stat:nth-child(4) { padding-left: 0; border-left: 0; }
}
@media (max-width: 699px) {
  .he__row { --cols: minmax(0, 1fr) auto; }
  .he__detail { grid-column: 1 / -1; }
  .he__row .ap-end { display: none; }
  .he__fail { --cols: minmax(0, 1fr); }
  .ap-stat:nth-child(4) { padding-left: 16px; border-left: 1px solid var(--hair); }
  .ap-stat:nth-child(odd) { padding-left: 0; border-left: 0; }
}
</style>
