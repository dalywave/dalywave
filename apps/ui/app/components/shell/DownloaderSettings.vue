<script setup lang="ts">
import { formatNumber } from '~/utils/format'

// Раздел настроек «Загрузчик на этом Mac» (Windows — «на этом ПК»): пока приложение открыто,
// компьютер качает музыку для сервера со своего адреса. Переключатель, живое состояние с треком
// и ходом задания, счётчики за сегодня, сервер, имя в админке, журнал. На сайте и телефонах —
// объяснение, что загрузчик живёт в приложении для компьютера. Собран из тех же кирпичей,
// что и страница настроек (<SettingsGroup>, <SettingsRow>)
const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const route = useRoute()
const dl = useDownloader()
const { t } = useI18n({ useScope: 'local' })

const pc = bridge.platform === 'windows'
const view = computed(() => dl.view.value)
const snap = computed(() => dl.snapshot.value)
/** Токен загрузчика заводит только администратор */
const allowed = computed(() => auth.isAdmin && view.value.kind !== 'forbidden')
const enabled = computed(() => snap.value?.enabled ?? true)
const paused = computed(() => snap.value?.paused ?? false)
const busy = ref(false)

const title = computed(() => (!dl.available ? t('titleWeb') : pc ? t('titlePc') : t('titleMac')))

// Отсчёт до следующей попытки — раз в секунду, пока раздел на экране
const now = ref(Date.now())
useIntervalFn(() => (now.value = Date.now()), 1000)
function when(ms: number | null): string {
  if (ms == null) return ''
  const s = Math.round((ms - now.value) / 1000)
  if (s <= 1) return t('soon')
  if (s < 60) return t('inSeconds', { n: s })
  return t('at', { time: new Date(ms).toLocaleTimeString(lang.value === 'en' ? 'en-US' : 'ru-RU', { hour: '2-digit', minute: '2-digit' }) })
}

const host = computed(() => {
  const server = snap.value?.server
  if (!server) return '—'
  try {
    return new URL(server).host
  }
  catch {
    return server
  }
})

const job = computed(() => (view.value.kind === 'working' ? view.value.job : null))
const percent = computed(() => Math.floor((job.value?.progress ?? 0) * 100))

/** Заголовок строки состояния: что загрузчик делает */
const state = computed(() => {
  const v = view.value
  switch (v.kind) {
    case 'working':
      if (v.finishing) return t('state.finishing', { track: v.job?.track ?? t('trackPending') })
      return v.job?.track ? t('state.working', { track: v.job.track }) : t('state.workingUnknown')
    case 'resting': return v.retryAt ? t('state.restingUntil', { time: when(v.retryAt) }) : t('state.resting')
    default: return t(`state.${v.kind}`)
  }
})

/** Описание под ним: артист и этап — или почему и что дальше */
const hint = computed(() => {
  const v = view.value
  switch (v.kind) {
    case 'working': {
      if (!v.job) return ''
      const stage = v.job.attempt > 1 ? t('hint.stageAttempt', { stage: t(`stage.${v.job.stage}`), n: v.job.attempt }) : t(`stage.${v.job.stage}`)
      // Этап пишется со строчной после артиста; без артиста (первые доли секунды) — с прописной
      return v.job.artist ? `${v.job.artist} · ${stage}` : stage.charAt(0).toLocaleUpperCase() + stage.slice(1)
    }
    case 'idle': return t('hint.idle', { host: host.value })
    case 'paused': return t('hint.paused')
    case 'offline': return v.retryAt ? t('hint.offlineRetry', { when: when(v.retryAt) }) : t('hint.offline')
    case 'resting': return t('hint.resting', { reason: v.error?.message ?? t('hint.restingReason') })
    case 'starting': return t('hint.starting')
    case 'stopping': return t('hint.stopping')
    case 'restarting': return t('hint.restarting', { when: when(v.retryAt) })
    case 'token': return t('hint.token')
    case 'broken': return t('hint.broken', { reason: v.error?.message ?? '' })
    case 'disabled': return pc ? t('hint.disabledPc') : t('hint.disabledMac')
    case 'signed-out': return t('hint.signedOut')
    default: return ''
  }
})

/** Пауза имеет смысл, пока загрузчик работает или вот-вот начнёт */
const canPause = computed(() => enabled.value && ['working', 'idle', 'paused', 'offline', 'resting', 'starting', 'restarting'].includes(view.value.kind))

const pot = computed(() => {
  const p = view.value.pot
  if (!p) return null
  if (p.state === 'ready') return t('pot.ready', { version: p.version ?? '' })
  if (p.state === 'failed') return t('pot.failed')
  return p.state === 'off' ? t('pot.off') : t('pot.starting')
})

const version = computed(() => {
  const s = snap.value
  if (!s?.version) return undefined
  return s.ytDlp ? t('versions', { fetcher: s.version, ytDlp: s.ytDlp }) : s.version
})

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

/** Путь к журналу покороче: домашняя папка — «~», на Windows — %LOCALAPPDATA% */
const logPath = computed(() => {
  const p = snap.value?.logFile
  if (!p) return undefined
  return p.replace(/^\/Users\/[^/]+\//, '~/').replace(/^[A-Z]:\\Users\\[^\\]+\\AppData\\Local\\/i, '%LOCALAPPDATA%\\')
})

// Переход из боковой панели (/settings#downloader): содержимое прокручивается своим контейнером
onMounted(() => {
  if (route.hash === '#downloader') setTimeout(() => document.getElementById('downloader')?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 120)
})
</script>

<template>
  <SettingsGroup id="downloader" class="dls" :title="title" :footnote="dl.available ? (allowed ? (pc ? t('footnotePc') : t('footnoteMac')) : undefined) : t('webText')">
    <!-- Сайт и телефоны: загрузчик живёт в приложении для компьютера -->
    <SettingsRow v-if="!dl.available" :title="t('webTitle')" />

    <template v-else>
      <SettingsRow :title="pc ? t('switchPc') : t('switchMac')" :description="pc ? t('switchHintPc') : t('switchHintMac')">
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
        <!-- Живое состояние: что делает прямо сейчас -->
        <SettingsRow class="live" :class="`live--${view.kind}`" role="status" aria-live="polite">
          <template #lead>
            <ShellDownloaderRing :view="view" :size="32" :pc="pc" />
          </template>
          <template #title>
            <span class="live__state">{{ state }}</span>
          </template>
          <template v-if="hint || job" #description>
            {{ hint }}<template v-if="job">{{ hint ? ' · ' : '' }}<span class="live__pct">{{ percent }}&nbsp;%</span></template>
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
        <SettingsRow :title="t('server')" :description="pot ? t('potLine', { pot }) : undefined">
          <span class="value">{{ host }}</span>
        </SettingsRow>
        <SettingsRow :title="t('name')" :description="version">
          <span class="value value--name">{{ snap?.workerName ?? '—' }}</span>
        </SettingsRow>
        <SettingsRow as="button" :title="t('openLog')" :description="logPath" class="log" @click="run(dl.openLog)" />
      </template>
    </template>
  </SettingsGroup>
</template>

<style scoped>
.dls { scroll-margin-top: calc(var(--topbar-h) + 16px); }

/* Знак загрузчика вместо плитки значка */
.live :deep(.sr__lead) { background: none; border-radius: 50%; overflow: visible; }
.live__state { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.live--working .live__state { color: var(--text-1); }
.live--offline .live__state, .live--resting .live__state { color: var(--warning); }
.live--token .live__state, .live--broken .live__state { color: var(--danger); }
.live :deep(.sr__title) { flex-wrap: nowrap; min-width: 0; }
.live__pct { font-variant-numeric: tabular-nums; color: var(--accent); font-weight: 600; }

/* Ход задания — подстрока под состоянием, цветом настроения */
.bar { position: relative; flex: 1; height: 4px; margin: 0 4px 4px 0; overflow: hidden; border-radius: var(--r-pill); background: var(--fill-3); }
.bar span { position: absolute; inset: 0 auto 0 0; border-radius: inherit; background: var(--accent-gradient, var(--accent)); box-shadow: 0 0 10px color-mix(in oklab, var(--accent) 50%, transparent); transition: width var(--dur-3) var(--ease-out); }

.value { font-size: var(--fs-small); color: var(--text-2); text-align: right; }
.value--name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tnum { font-variant-numeric: tabular-nums; }
.log :deep(.sr__desc) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

<i18n lang="json">
{
  "ru": {
    "titleMac": "Загрузчик на этом Mac",
    "titlePc": "Загрузчик на этом ПК",
    "titleWeb": "Загрузчик музыки",
    "webTitle": "Работает в приложении для Mac и ПК",
    "webText": "YouTube не отдаёт записи серверу — их качает компьютер, пока на нём открыто приложение DalyWave, даже свёрнутым в строку меню или трей",
    "switchMac": "Скачивать на этом Mac",
    "switchPc": "Скачивать на этом ПК",
    "switchHintMac": "Пока DalyWave открыт, даже в строке меню",
    "switchHintPc": "Пока DalyWave открыт, даже в трее",
    "footnoteMac": "YouTube не отдаёт записи серверу — их качает этот Mac. Закроете приложение — загрузка остановится",
    "footnotePc": "YouTube не отдаёт записи серверу — их качает этот ПК. Закроете приложение — загрузка остановится",
    "forbidden": "Загрузчик доступен администратору",
    "forbiddenHint": "Токен загрузчика выдаёт сервер только администратору",
    "state": {
      "working": "Качает «{track}»",
      "workingUnknown": "Качает трек",
      "finishing": "Доделывает «{track}», потом пауза",
      "idle": "Ждёт заданий",
      "paused": "На паузе",
      "offline": "Нет связи с сервером",
      "resting": "Отдыхает",
      "restingUntil": "Отдыхает — продолжит {time}",
      "starting": "Запускается…",
      "stopping": "Останавливается…",
      "restarting": "Перезапускается",
      "token": "Сервер не принял токен",
      "broken": "Установка повреждена",
      "disabled": "Выключен",
      "signed-out": "Нужен вход",
      "forbidden": "Доступен администратору"
    },
    "stage": {
      "matching": "поиск записи",
      "downloading": "скачивание",
      "verifying": "проверка отпечатком",
      "uploading": "отправка на сервер"
    },
    "hint": {
      "stageAttempt": "{stage}, кандидат {n}",
      "idle": "На связи с {host} — возьмёт задание, как только оно появится",
      "paused": "Новых заданий не берёт",
      "offline": "Повторит сам",
      "offlineRetry": "Повторит {when}",
      "resting": "{reason}. Продолжит сам",
      "restingReason": "YouTube попросил передышку",
      "starting": "Поднимает сервис PO-токенов — несколько секунд",
      "stopping": "Возвращает задание серверу",
      "restarting": "Упал — перезапустится {when}",
      "token": "Токен отозвали в админке («Источники») — нужен новый",
      "broken": "{reason}. Переустановите приложение",
      "disabledMac": "Этот Mac не качает музыку для сервера",
      "disabledPc": "Этот ПК не качает музыку для сервера",
      "signedOut": "Войдите в приложение, и загрузчик заработает"
    },
    "soon": "сейчас",
    "inSeconds": "через {n} с",
    "at": "в {time}",
    "trackPending": "трек",
    "progress": "Ход задания",
    "pause": "Пауза",
    "resume": "Продолжить",
    "recreate": "Пересоздать токен",
    "today": "Сегодня",
    "counters": "скачано {done} · не удалось {failed}",
    "server": "Сервер",
    "potLine": "PO-токены: {pot}",
    "pot": {
      "ready": "готовы, bgutil {version}",
      "failed": "не запустились — качает без них",
      "off": "выключены",
      "starting": "запускаются…"
    },
    "name": "Имя в админке",
    "versions": "Загрузчик {fetcher} · yt-dlp {ytDlp}",
    "openLog": "Открыть журнал",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "en": {
    "titleMac": "Downloader on this Mac",
    "titlePc": "Downloader on this PC",
    "titleWeb": "Music downloader",
    "webTitle": "Works in the app for Mac and PC",
    "webText": "YouTube won't hand recordings to the server — a computer downloads them while the DalyWave app is open on it, even tucked into the menu bar or tray",
    "switchMac": "Download on this Mac",
    "switchPc": "Download on this PC",
    "switchHintMac": "While DalyWave is open, even in the menu bar",
    "switchHintPc": "While DalyWave is open, even in the tray",
    "footnoteMac": "YouTube won't hand recordings to the server — this Mac downloads them. Quit the app and downloading stops",
    "footnotePc": "YouTube won't hand recordings to the server — this PC downloads them. Quit the app and downloading stops",
    "forbidden": "The downloader is available to admins",
    "forbiddenHint": "The server issues downloader tokens to admins only",
    "state": {
      "working": "Downloading “{track}”",
      "workingUnknown": "Downloading a track",
      "finishing": "Finishing “{track}”, then pausing",
      "idle": "Waiting for jobs",
      "paused": "Paused",
      "offline": "Can't reach the server",
      "resting": "Resting",
      "restingUntil": "Resting — resumes {time}",
      "starting": "Starting…",
      "stopping": "Stopping…",
      "restarting": "Restarting",
      "token": "Token rejected by the server",
      "broken": "Installation is damaged",
      "disabled": "Off",
      "signed-out": "Sign-in required",
      "forbidden": "Available to admins"
    },
    "stage": {
      "matching": "finding the recording",
      "downloading": "downloading",
      "verifying": "checking the fingerprint",
      "uploading": "sending to the server"
    },
    "hint": {
      "stageAttempt": "{stage}, candidate {n}",
      "idle": "Connected to {host} — takes a job as soon as one appears",
      "paused": "Not taking new jobs",
      "offline": "Will retry on its own",
      "offlineRetry": "Retries {when}",
      "resting": "{reason}. Resumes on its own",
      "restingReason": "YouTube asked for a break",
      "starting": "Starting the PO token service — a few seconds",
      "stopping": "Returning the job to the server",
      "restarting": "Crashed — restarts {when}",
      "token": "The token was revoked in the admin (Sources) — a new one is needed",
      "broken": "{reason}. Reinstall the app",
      "disabledMac": "This Mac doesn't download music for the server",
      "disabledPc": "This PC doesn't download music for the server",
      "signedOut": "Sign in to the app to start the downloader"
    },
    "soon": "now",
    "inSeconds": "in {n} s",
    "at": "at {time}",
    "trackPending": "track",
    "progress": "Job progress",
    "pause": "Pause",
    "resume": "Resume",
    "recreate": "Recreate token",
    "today": "Today",
    "counters": "{done} downloaded · {failed} failed",
    "server": "Server",
    "potLine": "PO tokens: {pot}",
    "pot": {
      "ready": "ready, bgutil {version}",
      "failed": "didn't start — downloading without them",
      "off": "off",
      "starting": "starting…"
    },
    "name": "Name in admin",
    "versions": "Downloader {fetcher} · yt-dlp {ytDlp}",
    "openLog": "Open log",
    "failed": "That didn't work — try again"
  }
}
</i18n>
