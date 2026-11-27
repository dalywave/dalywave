<script setup lang="ts">
import type { DownloadPolicy, Lane, LanePatch, ServerSettings, SettingsSection } from '~/api/types'

// Настройки сервера по вкладкам: общее (часовой пояс, итоги года), вход, музыка (подписки, скорость
// и расписание загрузок), хранилище, клипы, рекомендации, уведомления, интеграции. На каждой
// вкладке внизу — что задано переменными окружения: видно, но меняется только на сервере
definePageMeta({ layout: 'admin' })

type Tab = 'general' | 'signin' | 'music' | 'storage' | 'clips' | 'reco' | 'notifications' | 'integrations'
const TABS: { key: Tab, icon: string, section: SettingsSection }[] = [
  { key: 'general', icon: 'ph:gear-six', section: 'general' },
  { key: 'signin', icon: 'ph:sign-in', section: 'sign_in' },
  { key: 'music', icon: 'ph:music-notes', section: 'music' },
  { key: 'storage', icon: 'ph:hard-drives', section: 'storage' },
  { key: 'clips', icon: 'ph:film-strip', section: 'clips' },
  { key: 'reco', icon: 'ph:sparkle', section: 'reco' },
  { key: 'notifications', icon: 'ph:bell', section: 'notifications' },
  { key: 'integrations', icon: 'ph:plugs', section: 'integrations' },
]
const route = useRoute()
const router = useRouter()
const tab = computed<Tab>({
  get: () => TABS.find(x => x.key === route.query.tab)?.key ?? 'general',
  set: v => void router.replace({ query: { ...route.query, tab: v === 'general' ? undefined : v } }),
})
const section = computed(() => TABS.find(x => x.key === tab.value)!.section)
// Телефон: выбранная вкладка — в видимой части строки (пришли по ссылке на «Клипы»)
const tabsEl = ref<HTMLElement | null>(null)
function revealTab() {
  tabsEl.value?.querySelector<HTMLElement>('.is-active')?.scrollIntoView({ block: 'nearest', inline: 'center', behavior: 'instant' as ScrollBehavior })
}
onMounted(() => void nextTick(revealTab))
watch(tab, () => void nextTick(revealTab))

const bridge = useBridge()
const toast = useToast()
// Докачка по подпискам — релизы Deezer и загрузчик: без них настройки нет
const server = useServerStore()
const lang = useLang()
const { t } = useScopedI18n('pages.admin.settings')
useHead({ title: () => t('title') })
const { failed, attempt } = useAttempts()

const settings = ref<ServerSettings | null>(null)
const lanes = ref<Lane[] | null>(null)
const loadSettings = () => attempt('settings', () => bridge.admin.settings(), v => (settings.value = v))
const loadLanes = () => attempt('lanes', () => bridge.admin.lanes(), v => (lanes.value = v.items))
onMounted(() => {
  void loadSettings()
  void loadLanes()
})

// ---------- Подписки ----------

const POLICY: (keyof DownloadPolicy)[] = ['albums', 'eps', 'singles', 'compilations', 'live', 'appearsOn']
async function setPolicy(key: keyof DownloadPolicy, value: boolean) {
  const s = settings.value
  if (!s) return
  const was = s.followPolicy
  s.followPolicy = { ...was, [key]: value }
  try {
    settings.value = await bridge.admin.updateSettings({ followPolicy: s.followPolicy })
    toast.show({ text: t('saved'), icon: 'ph:check' })
  }
  catch {
    s.followPolicy = was
    toast.error(t('failed'))
  }
}

// ---------- Часовой пояс ----------

const OFFSETS = [-720, -660, -600, -540, -480, -420, -360, -300, -240, -180, -120, -60, 0, 60, 120, 180, 210, 240, 270, 300, 330, 345, 360, 390, 420, 480, 540, 570, 600, 660, 720, 780, 840]
const offsetLabel = (m: number) => {
  const sign = m < 0 ? '−' : '+'
  const a = Math.abs(m)
  return `UTC${sign}${Math.floor(a / 60)}${a % 60 ? `:${String(a % 60).padStart(2, '0')}` : ''}`
}
const offsetOptions = OFFSETS.map(o => ({ value: o, label: offsetLabel(o) }))
async function setOffset(v: number) {
  try {
    settings.value = await bridge.admin.updateSettings({ utcOffsetMin: v })
    toast.show({ text: t('saved'), icon: 'ph:check' })
    void loadLanes()
  }
  catch {
    toast.error(t('failed'))
  }
}

// ---------- Полосы: скорость и расписание ----------

const DOWNLOAD_LANES = ['now', 'wave', 'library', 'discography', 'discovery']
const BACKGROUND_LANES = ['library', 'discography', 'discovery', 'analysis', 'scout']
const downloadLanes = computed(() => lanes.value?.filter(l => DOWNLOAD_LANES.includes(l.lane)) ?? [])
const scheduleLanes = computed(() => lanes.value?.filter(l => BACKGROUND_LANES.includes(l.lane)) ?? [])
const PAUSES = [0, 1000, 2000, 5000, 8000, 15000, 30000, 60000]
const pauseLabel = (ms: number) => (ms ? t('pauseS', { s: (ms / 1000).toLocaleString(intlLocale(lang.value, 'en-US')) }) : t('noPause'))
const HOURS = Array.from({ length: 24 }, (_, h) => h * 60)
const hhmm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`

async function patchLane(l: Lane, patch: LanePatch) {
  try {
    const updated = await bridge.admin.updateLane(l.lane, patch).then(() => bridge.admin.lanes())
    lanes.value = updated.items
    toast.show({ text: t('laneSaved', { title: l.title }), icon: 'ph:check' })
  }
  catch {
    toast.error(t('failed'))
  }
}
const setPause = (l: Lane, v: number) => patchLane(l, { minIntervalMs: v })
const setConcurrency = (l: Lane, v: number) => patchLane(l, { concurrency: v })
const pauseOptions = (l: Lane) => [...PAUSES, ...(PAUSES.includes(l.minIntervalMs) ? [] : [l.minIntervalMs])].map(p => ({ value: p, label: pauseLabel(p) }))
const concurrencyOptions = computed(() => [1, 2, 3, 4, 6, 8].map(c => ({ value: c, label: t('speed.workers', { n: c }) })))
const hourOptions = (current: number) => [...HOURS, ...(current % 60 ? [current] : [])].map(h => ({ value: h, label: hhmm(h) }))
function setWindowed(l: Lane, on: boolean) {
  // Включённое расписание по умолчанию — ночь: с 01:00 до 07:00
  void patchLane(l, { schedule: on ? { from: 60, until: 420 } : { from: null, until: null } })
}
function setBound(l: Lane, which: 'from' | 'until', v: number) {
  const from = which === 'from' ? v : l.activeFrom ?? 60
  const until = which === 'until' ? v : l.activeUntil ?? 420
  if (from === until) return toast.error(t('sameBounds'))
  void patchLane(l, { schedule: { from, until } })
}
</script>

<template>
  <div class="ap se">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <nav ref="tabsEl" class="ap-chips se__tabs" :aria-label="t('tabs')">
      <button
        v-for="x in TABS"
        :key="x.key"
        type="button"
        class="ap-chip"
        :class="{ 'is-active': tab === x.key }"
        :aria-pressed="tab === x.key"
        @click="tab = x.key"
      >
        <Icon :name="x.icon" />{{ t(`tab.${x.key}`) }}
      </button>
    </nav>

    <template v-if="tab === 'general'">
      <SettingsGroup v-if="settings" :title="t('tz.title')">
        <SettingsRow :title="t('tz.label')" :description="t('tz.hint')">
          <AdminSelect :model-value="settings.utcOffsetMin" :options="offsetOptions" :label="t('tz.label')" align="end" @update:model-value="setOffset" />
        </SettingsRow>
      </SettingsGroup>
      <!-- Итоги года: стиль историй и когда они открыты -->
      <AdminRewindEvent />
    </template>

    <template v-else-if="tab === 'signin'">
      <SettingsGroup :title="t('signin.title')">
        <SettingsRow as="button" :title="t('signin.access')" :description="t('signin.accessHint')" @click="navigateTo('/admin/access')">
          <Icon name="ph:caret-right" class="se__go" />
        </SettingsRow>
        <SettingsRow as="button" :title="t('signin.users')" :description="t('signin.usersHint')" @click="navigateTo('/admin/users')">
          <Icon name="ph:caret-right" class="se__go" />
        </SettingsRow>
      </SettingsGroup>
    </template>

    <template v-else-if="tab === 'storage'">
      <SettingsGroup :title="t('storage.title')">
        <SettingsRow as="button" :title="t('storage.open')" :description="t('storage.hint')" @click="navigateTo('/admin/storage')">
          <Icon name="ph:caret-right" class="se__go" />
        </SettingsRow>
        <SettingsRow as="button" :title="t('storage.backups')" :description="t('storage.backupsHint')" @click="navigateTo('/admin/backups')">
          <Icon name="ph:caret-right" class="se__go" />
        </SettingsRow>
      </SettingsGroup>
    </template>

    <template v-else-if="tab === 'clips'">
      <AdminState :loading="!settings && !failed.settings" :error="failed.settings && !settings" :rows="6" @retry="loadSettings">
        <AdminClipSettings v-if="settings" :clips="settings.clips" @saved="c => settings && (settings.clips = c)" />
      </AdminState>
    </template>

    <template v-else-if="tab === 'reco'">
      <SettingsGroup :title="t('reco.title')">
        <SettingsRow as="button" :title="t('reco.open')" :description="t('reco.hint')" @click="navigateTo('/admin/reco')">
          <Icon name="ph:caret-right" class="se__go" />
        </SettingsRow>
      </SettingsGroup>
    </template>

    <template v-else-if="tab === 'notifications'">
      <p class="ap-hint">{{ t('notifications.hint') }}</p>
    </template>

    <template v-else-if="tab === 'integrations'">
      <p class="ap-hint">{{ t('integrations.hint') }}</p>
    </template>

    <template v-if="tab === 'music'">
    <AdminState :loading="!settings && !failed.settings" :error="failed.settings && !settings" :rows="6" @retry="loadSettings">
      <SettingsGroup v-if="settings && server.deezer && server.downloader" :title="t('follow.title')" :footnote="t('follow.note')">
        <SettingsRow v-for="k in POLICY" :key="k" :title="t(`follow.${k}`)">
          <UiSwitch :model-value="settings.followPolicy[k]" :label="t(`follow.${k}`)" hide-label @update:model-value="v => setPolicy(k, v)" />
        </SettingsRow>
      </SettingsGroup>
    </AdminState>

    <AdminState :loading="!lanes && !failed.lanes" :error="failed.lanes && !lanes" :rows="5" @retry="loadLanes">
      <SettingsGroup :title="t('speed.title')" :footnote="t('speed.note')">
        <SettingsRow v-for="l in downloadLanes" :key="l.lane" :title="l.title" stack>
          <AdminSelect class="se__pause" :model-value="l.minIntervalMs" :options="pauseOptions(l)" :label="t('speed.pause', { title: l.title })" align="end" @update:model-value="v => setPause(l, v)" />
          <AdminSelect class="se__workers" :model-value="l.concurrency" :options="concurrencyOptions" :label="t('speed.parallel', { title: l.title })" align="end" @update:model-value="v => setConcurrency(l, v)" />
        </SettingsRow>
      </SettingsGroup>
    </AdminState>

    <SettingsGroup v-if="lanes" :title="t('schedule.title')" :footnote="t('schedule.note')">
      <template v-for="l in scheduleLanes" :key="l.lane">
        <SettingsRow :title="l.title" :description="l.activeFrom != null && l.activeUntil != null ? t(l.open ? 'schedule.windowOpen' : 'schedule.windowClosed', { from: hhmm(l.activeFrom), until: hhmm(l.activeUntil) }) : t('schedule.always')">
          <UiSwitch :model-value="l.activeFrom != null" :label="t('schedule.toggle', { title: l.title })" hide-label @update:model-value="v => setWindowed(l, v)" />
        </SettingsRow>
        <SettingsRow v-if="l.activeFrom != null && l.activeUntil != null" indent>
          <div class="se__window">
            <span>{{ t('schedule.from') }}</span>
            <AdminSelect :model-value="l.activeFrom" :options="hourOptions(l.activeFrom)" :label="t('schedule.fromLabel', { title: l.title })" @update:model-value="v => setBound(l, 'from', v)" />
            <span>{{ t('schedule.until') }}</span>
            <AdminSelect :model-value="l.activeUntil" :options="hourOptions(l.activeUntil)" :label="t('schedule.untilLabel', { title: l.title })" @update:model-value="v => setBound(l, 'until', v)" />
          </div>
        </SettingsRow>
      </template>
    </SettingsGroup>

    </template>

    <AdminEnvList :key="section" :section="section" />
  </div>
</template>

<style scoped>
.se { max-width: 760px; }
.se__tabs { margin-bottom: -8px; }
@media (min-width: 700px) {
  .se__tabs { flex-wrap: wrap; overflow: visible; }
}
.se__go { color: var(--text-3); }
.se__window { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: var(--fs-small); color: var(--text-2); }
.se :deep(.sr__control) .asl { min-width: 0; }
/* Одинаковая ширина у выпадающих списков всех полос — колонки ровные */
.se :deep(.se__pause) { width: 140px; }
.se :deep(.se__workers) { width: 180px; }
@media (max-width: 699px) {
  .se :deep(.se__pause), .se :deep(.se__workers) { flex: 1 1 0; width: auto; }
}
</style>
