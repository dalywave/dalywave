<script setup lang="ts">
// Загрузчик на этом компьютере в подвале боковой панели, рядом с «Загрузкой подписок»: что он
// делает прямо сейчас. Ведёт в раздел настроек. Только приложение для компьютера и только
// администратору — остальным загрузчик недоступен. rail — один значок с подсказкой.
withDefaults(defineProps<{ rail?: boolean }>(), {})

const bridge = useBridge()
const auth = useAuthStore()
const dl = useDownloader()
const { t } = useI18n({ useScope: 'local' })

const pc = bridge.platform === 'windows'
const view = computed(() => dl.view.value)
const shown = computed(() => dl.available && auth.isAdmin && view.value.kind !== 'forbidden')
const job = computed(() => (view.value.kind === 'working' ? view.value.job : null))
const percent = computed(() => Math.floor((job.value?.progress ?? 0) * 100))

/** Подпись под заголовком: без артиста — места мало, процент — отдельно */
const meta = computed(() => {
  const v = view.value
  if (v.kind !== 'working') return t(`meta.${v.kind}`)
  if (!v.job?.track) return t('meta.workingUnknown')
  return v.finishing ? t('meta.finishing', { track: v.job.track }) : t('meta.working', { track: v.job.track })
})

/** Полная строка — как в меню трея: «Загрузчик: качает «Трек» — Артист · 45 %» */
const line = computed(() => {
  const j = job.value
  let state = meta.value
  if (j?.track && j.artist && !view.value.finishing) state = t('meta.workingFull', { track: j.track, artist: j.artist })
  const text = t('line', { state: state.charAt(0).toLocaleLowerCase() + state.slice(1) })
  return j && !view.value.finishing ? `${text} · ${percent.value}\u00A0%` : text
})
</script>

<template>
  <UiTooltip v-if="shown" :text="line" side="right">
    <NuxtLink to="/settings#downloader" class="dli" :class="[`dli--${view.kind}`, { 'dli--rail': rail }]" :aria-label="line">
      <ShellDownloaderRing :view="view" :size="28" :pc="pc" />
      <span v-if="!rail" class="dli__text">
        <span class="dli__title">{{ pc ? t('titlePc') : t('titleMac') }}</span>
        <span class="dli__meta">
          <span class="dli__what">{{ meta }}</span>
          <span v-if="job && !view.finishing" class="dli__pct">· {{ percent }}&nbsp;%</span>
        </span>
      </span>
    </NuxtLink>
  </UiTooltip>
</template>

<style scoped>
/* Как строка «Загрузка подписок» рядом */
.dli { display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px 10px; border-radius: var(--r-3); text-align: left; transition: background-color var(--dur-1); }
.dli:hover { background: var(--fill-1); }
.dli:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.dli__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.dli__title { overflow: hidden; font-size: var(--fs-small); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.dli__meta { display: flex; gap: 4px; min-width: 0; font-size: var(--fs-caption); color: var(--text-3); white-space: nowrap; }
.dli__what { overflow: hidden; text-overflow: ellipsis; }
.dli__pct { flex: none; font-variant-numeric: tabular-nums; color: var(--accent); }
.dli--working .dli__what { color: var(--text-2); }
.dli--offline .dli__meta, .dli--resting .dli__meta { color: var(--warning); }
.dli--token .dli__meta, .dli--broken .dli__meta { color: var(--danger); }
.dli--rail { justify-content: center; padding: 8px 0; }
</style>

<i18n lang="json">
{
  "ru": {
    "titleMac": "Загрузчик на этом Mac",
    "titlePc": "Загрузчик на этом ПК",
    "line": "Загрузчик: {state}",
    "meta": {
      "working": "Качает «{track}»",
      "workingFull": "Качает «{track}» — {artist}",
      "workingUnknown": "Качает трек",
      "finishing": "Доделывает «{track}», потом пауза",
      "idle": "Ждёт заданий",
      "paused": "На паузе",
      "offline": "Нет связи с сервером",
      "resting": "Отдыхает после проверки YouTube",
      "starting": "Запускается…",
      "stopping": "Останавливается…",
      "restarting": "Перезапускается…",
      "token": "Сервер не принял токен",
      "broken": "Установка повреждена",
      "disabled": "Выключен",
      "signed-out": "Нужен вход",
      "forbidden": "Доступен администратору"
    }
  },
  "en": {
    "titleMac": "Downloader on this Mac",
    "titlePc": "Downloader on this PC",
    "line": "Downloader: {state}",
    "meta": {
      "working": "Downloading “{track}”",
      "workingFull": "Downloading “{track}” — {artist}",
      "workingUnknown": "Downloading a track",
      "finishing": "Finishing “{track}”, then pausing",
      "idle": "Waiting for jobs",
      "paused": "Paused",
      "offline": "Can't reach the server",
      "resting": "Resting after a YouTube check",
      "starting": "Starting…",
      "stopping": "Stopping…",
      "restarting": "Restarting…",
      "token": "Token rejected by the server",
      "broken": "Installation is damaged",
      "disabled": "Off",
      "signed-out": "Sign-in required",
      "forbidden": "Available to admins"
    }
  }
}
</i18n>
