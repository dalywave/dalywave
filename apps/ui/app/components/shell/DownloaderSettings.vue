<script setup lang="ts">
import { formatNumber } from '~/utils/format'

// Раздел настроек «Загрузчик на этом Mac» (Windows — «на этом ПК»): переключатель, что делает
// сейчас, счётчик за сегодня, журнал. На сайте и телефонах — одна строка: загрузчик живёт в
// приложении для компьютера. Кирпичи — <SettingsGroup> и <SettingsRow>, тексты — короткие
const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const route = useRoute()
const dl = useDownloader()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })

const pc = bridge.platform === 'windows'
const view = computed(() => dl.view.value)
const snap = computed(() => dl.snapshot.value)
/** Токен загрузчика заводит только администратор */
const allowed = computed(() => auth.isAdmin && view.value.kind !== 'forbidden')
const enabled = computed(() => snap.value?.enabled ?? true)
const paused = computed(() => snap.value?.paused ?? false)
const busy = ref(false)

const job = computed(() => (view.value.kind === 'working' ? view.value.job : null))
const percent = computed(() => Math.floor((job.value?.progress ?? 0) * 100))

function time(ms: number) {
  return new Date(ms).toLocaleTimeString(lang.value === 'en' ? 'en-US' : 'ru-RU', { hour: '2-digit', minute: '2-digit' })
}

/** Что загрузчик делает прямо сейчас — одной строкой */
const state = computed(() => {
  const v = view.value
  if (v.kind === 'working') {
    if (v.finishing) return t('state.finishing')
    if (!v.job?.track) return t('state.workingUnknown')
    return v.job.artist ? t('state.working', { track: v.job.track, artist: v.job.artist }) : t('state.workingTrack', { track: v.job.track })
  }
  if (v.kind === 'resting' && v.retryAt) return t('state.restingUntil', { time: time(v.retryAt) })
  return t(`state.${v.kind}`)
})

/** Вне приложения раздел — подсказка администратору с компьютера; остальным и на телефоне он лишний */
const server = useServerStore()
const shown = computed(() => server.downloader && (dl.available || (auth.isAdmin && !ui.isPhone)))

/** Пауза имеет смысл, пока загрузчик работает или вот-вот начнёт */
const canPause = computed(() => enabled.value && ['working', 'idle', 'paused', 'offline', 'resting', 'starting', 'restarting'].includes(view.value.kind))

async function run(action: () => Promise<unknown>) {
  if (busy.value) return
  busy.value = true
  try {
    await action()
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}

// Переход из боковой панели (/settings#downloader): содержимое прокручивается своим контейнером
onMounted(() => {
  if (route.hash === '#downloader') setTimeout(() => document.getElementById('downloader')?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 120)
})
</script>

<template>
  <SettingsGroup v-if="shown" id="downloader" class="dls" :title="!dl.available ? t('titleWeb') : pc ? t('titlePc') : t('titleMac')">
    <SettingsRow v-if="!dl.available" :title="t('web')" />

    <template v-else>
      <SettingsRow :title="pc ? t('switchPc') : t('switchMac')">
        <UiSwitch
          :model-value="allowed && enabled"
          :label="pc ? t('switchPc') : t('switchMac')"
          hide-label
          :disabled="!allowed || busy"
          @update:model-value="v => run(() => dl.setEnabled(v))"
        />
      </SettingsRow>

      <SettingsRow v-if="!allowed" :title="t('forbidden')" />

      <template v-else>
        <SettingsRow class="live" :class="`live--${view.kind}`" role="status" aria-live="polite">
          <template #lead>
            <ShellDownloaderRing :view="view" :size="32" :pc="pc" />
          </template>
          <template #title>
            <span class="live__state">{{ state }}</span><span v-if="job && !view.finishing" class="live__pct">·&nbsp;{{ percent }}&nbsp;%</span>
          </template>
          <UiButton v-if="view.kind === 'token'" variant="primary" size="sm" :loading="busy" @click="run(dl.recreateToken)">{{ t('recreate') }}</UiButton>
          <UiButton v-else-if="canPause" size="sm" :disabled="busy" @click="run(() => dl.setPaused(!paused))">{{ paused ? t('resume') : t('pause') }}</UiButton>
        </SettingsRow>
        <SettingsRow v-if="job" indent>
          <div class="bar" role="progressbar" :aria-valuenow="percent" aria-valuemin="0" aria-valuemax="100" :aria-label="t('progress')">
            <span :style="{ width: `${percent}%` }" />
          </div>
        </SettingsRow>

        <SettingsRow v-if="view.counters" :title="t('today')">
          <span class="value tnum">{{ t('counters', { done: formatNumber(view.counters.doneToday, lang), failed: formatNumber(view.counters.failedToday, lang) }) }}</span>
        </SettingsRow>
        <SettingsRow as="button" :title="t('log')" @click="run(dl.openLog)" />
      </template>
    </template>
  </SettingsGroup>
</template>

<style scoped>
.dls { scroll-margin-top: calc(var(--topbar-h) + 16px); }

/* Знак загрузчика перед строкой состояния */
.live :deep(.sr__lead) { background: none; border-radius: 50%; overflow: visible; }
.live :deep(.sr__title) { flex-wrap: nowrap; min-width: 0; }
.live__state { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.live__pct { flex: none; font-variant-numeric: tabular-nums; color: var(--accent); }
.live--offline .live__state, .live--resting .live__state { color: var(--warning); }
.live--token .live__state, .live--broken .live__state { color: var(--danger); }

/* Ход задания — подстрока цветом настроения */
.bar { position: relative; flex: 1; height: 4px; margin-bottom: 4px; overflow: hidden; border-radius: var(--r-pill); background: var(--fill-3); }
.bar span { position: absolute; inset: 0 auto 0 0; border-radius: inherit; background: var(--accent); transition: width var(--dur-3) var(--ease-out); }

.value { font-size: var(--fs-small); color: var(--text-2); text-align: right; }
.tnum { font-variant-numeric: tabular-nums; }
</style>

<i18n lang="json">
{
  "ru": {
    "titleMac": "Загрузчик на этом Mac",
    "titlePc": "Загрузчик на этом ПК",
    "titleWeb": "Загрузчик",
    "web": "Работает в приложении для Mac и ПК",
    "switchMac": "Скачивать на этом Mac",
    "switchPc": "Скачивать на этом ПК",
    "forbidden": "Доступен администратору",
    "state": {
      "working": "Качает: {track} — {artist}",
      "workingTrack": "Качает: {track}",
      "workingUnknown": "Качает",
      "finishing": "Доделывает трек, потом пауза",
      "idle": "Ждёт заданий",
      "paused": "На паузе",
      "offline": "Нет связи с сервером",
      "resting": "Отдыхает",
      "restingUntil": "Отдыхает до {time}",
      "starting": "Запускается…",
      "stopping": "Останавливается…",
      "restarting": "Перезапускается…",
      "token": "Сервер не принял токен",
      "broken": "Установка повреждена",
      "disabled": "Выключен",
      "signed-out": "Нужен вход",
      "forbidden": "Доступен администратору"
    },
    "progress": "Ход задания",
    "pause": "Пауза",
    "resume": "Продолжить",
    "recreate": "Пересоздать токен",
    "today": "Сегодня",
    "counters": "скачано {done} · не удалось {failed}",
    "log": "Журнал",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "en": {
    "titleMac": "Downloader on this Mac",
    "titlePc": "Downloader on this PC",
    "titleWeb": "Downloader",
    "web": "Works in the app for Mac and PC",
    "switchMac": "Download on this Mac",
    "switchPc": "Download on this PC",
    "forbidden": "Available to admins",
    "state": {
      "working": "Downloading: {track} — {artist}",
      "workingTrack": "Downloading: {track}",
      "workingUnknown": "Downloading",
      "finishing": "Finishing the track, then pausing",
      "idle": "Waiting for jobs",
      "paused": "Paused",
      "offline": "Can't reach the server",
      "resting": "Resting",
      "restingUntil": "Resting until {time}",
      "starting": "Starting…",
      "stopping": "Stopping…",
      "restarting": "Restarting…",
      "token": "Token rejected by the server",
      "broken": "Installation is damaged",
      "disabled": "Off",
      "signed-out": "Sign-in required",
      "forbidden": "Available to admins"
    },
    "progress": "Job progress",
    "pause": "Pause",
    "resume": "Resume",
    "recreate": "Recreate token",
    "today": "Today",
    "counters": "{done} downloaded · {failed} failed",
    "log": "Log",
    "failed": "That didn't work — try again"
  }
}
</i18n>
