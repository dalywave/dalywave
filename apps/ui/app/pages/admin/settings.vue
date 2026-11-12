<script setup lang="ts">
import type { DownloadPolicy, Lane, LanePatch, ServerSettings } from '~/api/types'

// Настройки сервера: что качать по новой подписке, скорость загрузок по полосам, расписание
// (например, дискографии — только ночью) и часовой пояс владельца
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
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
const pauseLabel = (ms: number) => (ms ? t('pauseS', { s: (ms / 1000).toLocaleString(lang.value === 'en' ? 'en-US' : 'ru-RU') }) : t('noPause'))
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

    <AdminState :loading="!settings && !failed.settings" :error="failed.settings && !settings" :rows="6" @retry="loadSettings">
      <SettingsGroup v-if="settings" :title="t('follow.title')" :footnote="t('follow.note')">
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

    <SettingsGroup v-if="settings" :title="t('tz.title')">
      <SettingsRow :title="t('tz.label')" :description="t('tz.hint')">
        <AdminSelect :model-value="settings.utcOffsetMin" :options="offsetOptions" :label="t('tz.label')" align="end" @update:model-value="setOffset" />
      </SettingsRow>
    </SettingsGroup>

    <!-- Итоги года: стиль историй и когда они открыты -->
    <AdminRewindEvent />
  </div>
</template>

<style scoped>
.se { max-width: 760px; }
.se__window { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: var(--fs-small); color: var(--text-2); }
.se :deep(.sr__control) .asl { min-width: 0; }
/* Одинаковая ширина у выпадающих списков всех полос — колонки ровные */
.se :deep(.se__pause) { width: 140px; }
.se :deep(.se__workers) { width: 180px; }
@media (max-width: 699px) {
  .se :deep(.se__pause), .se :deep(.se__workers) { flex: 1 1 0; width: auto; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Настройки",
    "saved": "Сохранено",
    "failed": "Не получилось сохранить — попробуйте ещё раз",
    "laneSaved": "«{title}»: сохранено",
    "follow": {
      "title": "Подписка на артиста",
      "note": "Что докачивать по новой подписке, если человек не выбрал сам",
      "albums": "Альбомы",
      "eps": "Мини-альбомы",
      "singles": "Синглы",
      "compilations": "Сборники",
      "live": "Концертные записи",
      "appearsOn": "Участие в чужих релизах"
    },
    "speed": {
      "title": "Скорость загрузок",
      "note": "Пауза между скачиваниями бережёт от проверок YouTube на бота",
      "pause": "Пауза в «{title}»",
      "parallel": "Одновременно в «{title}»",
      "workers": "{n} одновременно"
    },
    "noPause": "Без паузы",
    "pauseS": "Пауза {s} с",
    "schedule": {
      "title": "Расписание",
      "note": "Включите, чтобы полоса работала только в своё окно. «Сейчас» и «Волна» работают всегда",
      "always": "Круглосуточно",
      "windowOpen": "С {from} до {until}, сейчас работает",
      "windowClosed": "С {from} до {until}, сейчас ждёт",
      "toggle": "«{title}» только по расписанию",
      "from": "с",
      "until": "до",
      "fromLabel": "Начало окна «{title}»",
      "untilLabel": "Конец окна «{title}»"
    },
    "sameBounds": "Начало и конец окна должны различаться",
    "tz": {
      "title": "Время",
      "label": "Часовой пояс",
      "hint": "Для расписания и времени суток во вкусе"
    }
  },
  "en": {
    "title": "Settings",
    "saved": "Saved",
    "failed": "Couldn't save — try again",
    "laneSaved": "“{title}”: saved",
    "follow": {
      "title": "Following an artist",
      "note": "What to download for a new follow unless the person chose themselves",
      "albums": "Albums",
      "eps": "EPs",
      "singles": "Singles",
      "compilations": "Compilations",
      "live": "Live recordings",
      "appearsOn": "Appearances on others' releases"
    },
    "speed": {
      "title": "Download speed",
      "note": "A pause between downloads keeps YouTube's bot checks away",
      "pause": "Pause in “{title}”",
      "parallel": "At once in “{title}”",
      "workers": "{n} at once"
    },
    "noPause": "No pause",
    "pauseS": "{s} s pause",
    "schedule": {
      "title": "Schedule",
      "note": "Turn on to run a lane only in its window. “Now” and “Wave” always run",
      "always": "Around the clock",
      "windowOpen": "From {from} to {until}, running now",
      "windowClosed": "From {from} to {until}, waiting now",
      "toggle": "“{title}” on schedule only",
      "from": "from",
      "until": "until",
      "fromLabel": "Window start for “{title}”",
      "untilLabel": "Window end for “{title}”"
    },
    "sameBounds": "The window start and end must differ",
    "tz": {
      "title": "Time",
      "label": "Time zone",
      "hint": "For the schedule and time of day in taste"
    }
  }
}
</i18n>
