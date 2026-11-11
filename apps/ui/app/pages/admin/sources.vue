<script setup lang="ts">
import type { AdminAcquisition, Downloader, Sources } from '~/api/types'
import type { BarDay, BarSeries } from '~/components/admin/Bars.vue'
import type { MenuEntry } from '~/types/ui'

// Источники: домашние загрузчики (на связи, когда был, версии, работа за сутки, отзыв),
// серверный загрузчик, доля успеха по дням и «проверить скачивание» — трек вне очереди
// с живым ходом скачивания
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
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
const live = computed(() => data.value?.downloaders.filter(d => !d.revokedAt) ?? [])
const revoked = computed(() => data.value?.downloaders.filter(d => d.revokedAt) ?? [])

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
  await navigator.clipboard.writeText(token.value).then(() => toast.show({ text: t('dl.copied'), icon: 'ph:copy' }), () => toast.error(t('failed')))
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
  { key: 'unavailable', label: t('days.unavailable'), color: 'rgba(232, 236, 244, 0.4)' },
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
        <div v-if="live.length" class="ap-table src__table">
          <div class="ap-tr ap-tr--head">
            <span>{{ t('dl.name') }}</span>
            <span>{{ t('dl.state') }}</span>
            <span class="ap-hide-phone">{{ t('dl.versions') }}</span>
            <span class="ap-num">{{ t('dl.running') }}</span>
            <span class="ap-num">{{ t('dl.ok') }}</span>
            <span class="ap-num">{{ t('dl.failed') }}</span>
            <span />
          </div>
          <div v-for="d in live" :key="d.id" class="ap-tr">
            <span class="ap-cell"><b>{{ d.name }}</b><small>{{ d.workerId ?? t('dl.noMachine') }}</small></span>
            <span class="src__state" :class="{ 'ap-ok': d.online }"><i class="ap-dot" :class="{ 'is-ok': d.online }" />{{ seen(d) }}</span>
            <span class="ap-cell ap-hide-phone">
              <b>{{ d.ytDlpVersion ? `yt-dlp ${d.ytDlpVersion}` : '—' }}</b>
              <small>{{ [d.version ? `fetcher ${d.version}` : null, abilities(d)].filter(Boolean).join(' · ') }}</small>
            </span>
            <span class="ap-num" :class="{ 'ap-on': d.running }">{{ d.running || '—' }}</span>
            <span class="ap-num">{{ d.ok24h || '—' }}</span>
            <span class="ap-num" :class="{ 'ap-bad': d.failed24h }">{{ d.failed24h || '—' }}</span>
            <span class="ap-end">
              <UiMenu :items="menu(d)"><UiIconButton icon="ph:dots-three" :label="t('dl.actions')" size="sm" :tooltip="false" /></UiMenu>
            </span>
          </div>
        </div>
        <p v-else class="ap-hint src__none">{{ t('dl.none') }}</p>
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
        <details v-if="revoked.length" class="src__revoked">
          <summary>{{ t('dl.revokedList', { n: revoked.length }) }}</summary>
          <div class="ap-table">
            <div v-for="d in revoked" :key="d.id" class="ap-tr src__rev">
              <span class="ap-cell"><b>{{ d.name }}</b><small>{{ d.workerId ?? '' }}</small></span>
              <span class="ap-muted">{{ seen(d) }}</span>
            </div>
          </div>
        </details>
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
.src__table { --cols: minmax(140px, 1.2fr) minmax(130px, 1fr) minmax(150px, 1.2fr) 64px 64px 64px 40px; }
.src__state { display: inline-flex; align-items: center; gap: 8px; min-width: 0; color: var(--text-2); }
.src__state.ap-ok { color: var(--success); }
.src__none { padding: 8px 0; }
.src__new { display: flex; gap: 8px; max-width: 520px; padding-top: 12px; }
.src__new .ap-input { flex: 1; }
.src__token { display: flex; flex-direction: column; gap: 8px; padding: 12px 0; }
.src__token-row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.src__token code { padding: 8px 10px; border-radius: var(--r-2); background: var(--fill-1); color: var(--text-1); word-break: break-all; user-select: all; }
.src__revoked { padding-top: 8px; font-size: var(--fs-small); color: var(--text-3); }
.src__revoked summary { cursor: pointer; padding: 8px 0; }
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

<i18n lang="json">
{
  "ru": {
    "title": "Источники",
    "failed": "Не получилось — попробуйте ещё раз",
    "check": {
      "button": "Проверить скачивание",
      "title": "Проверка",
      "hint": "Трек скачивается вне очереди — видно, как работают источники",
      "started": "Проверяем на «{title}»"
    },
    "dl": {
      "title": "Загрузчики",
      "hint": "Качают аудио с вашего компьютера: YouTube не отдаёт его серверу",
      "name": "Загрузчик",
      "state": "Связь",
      "versions": "Версии",
      "running": "Идёт",
      "ok": "Скачано",
      "failed": "Сбои",
      "online": "на связи",
      "seen": "был {when}",
      "never": "ещё не подключался",
      "revokedAt": "отозван {when}",
      "noMachine": "машина неизвестна",
      "pot": "PO-токены",
      "cookies": "cookies",
      "actions": "Действия",
      "revoke": "Отозвать токен",
      "revoked": "Токен «{name}» больше не действует",
      "none": "Загрузчиков пока нет",
      "newLabel": "Имя нового загрузчика",
      "newPlaceholder": "Например, «Mac Захара»",
      "add": "Добавить",
      "tokenHint": "Токен показывается один раз: укажите его в загрузчике (make downloader TOKEN=…)",
      "copy": "Копировать",
      "copied": "Токен скопирован",
      "hideToken": "Скрыть токен",
      "revokedList": "Отозванные: {n}"
    },
    "days": {
      "title": "По дням",
      "rate": "успешно {p}% за 14 дней",
      "ok": "Скачано",
      "disputed": "Спорные",
      "unavailable": "Не найдено",
      "failed": "Сбои"
    },
    "server": {
      "title": "Серверный загрузчик",
      "state": "Состояние",
      "off": "выключен",
      "offHint": "Всё качают домашние загрузчики",
      "works": "работает",
      "down": "не отвечает",
      "error": "Ошибка",
      "pot": "Сервис PO-токенов",
      "cookies": "Cookies",
      "noCookies": "нет",
      "yes": "есть",
      "until": "до {when}",
      "running": "Сейчас качает",
      "ok": "Скачано за сутки",
      "failed": "Сбоев за сутки"
    }
  },
  "en": {
    "title": "Sources",
    "failed": "Didn't work — try again",
    "check": {
      "button": "Test a download",
      "title": "Test",
      "hint": "A track downloads out of turn — you see how the sources work",
      "started": "Testing with “{title}”"
    },
    "dl": {
      "title": "Downloaders",
      "hint": "They download audio from your computer: YouTube won't give it to the server",
      "name": "Downloader",
      "state": "Connection",
      "versions": "Versions",
      "running": "Running",
      "ok": "Done",
      "failed": "Errors",
      "online": "online",
      "seen": "seen {when}",
      "never": "never connected",
      "revokedAt": "revoked {when}",
      "noMachine": "unknown machine",
      "pot": "PO tokens",
      "cookies": "cookies",
      "actions": "Actions",
      "revoke": "Revoke token",
      "revoked": "The “{name}” token no longer works",
      "none": "No downloaders yet",
      "newLabel": "New downloader name",
      "newPlaceholder": "For example, “Zakhar's Mac”",
      "add": "Add",
      "tokenHint": "The token is shown once: give it to the downloader (make downloader TOKEN=…)",
      "copy": "Copy",
      "copied": "Token copied",
      "hideToken": "Hide token",
      "revokedList": "Revoked: {n}"
    },
    "days": {
      "title": "By day",
      "rate": "{p}% successful in 14 days",
      "ok": "Downloaded",
      "disputed": "Disputed",
      "unavailable": "Not found",
      "failed": "Errors"
    },
    "server": {
      "title": "Server downloader",
      "state": "State",
      "off": "off",
      "offHint": "Home downloaders do all the work",
      "works": "working",
      "down": "not responding",
      "error": "Error",
      "pot": "PO token service",
      "cookies": "Cookies",
      "noCookies": "none",
      "yes": "present",
      "until": "until {when}",
      "running": "Downloading now",
      "ok": "Downloaded in 24 h",
      "failed": "Errors in 24 h"
    }
  }
}
</i18n>
