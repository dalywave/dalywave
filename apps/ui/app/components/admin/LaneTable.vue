<script setup lang="ts">
import type { Lane } from '~/api/types'

// Полосы очереди: сколько ждёт, идёт, сломалось и сделано за сутки; пауза переключателем
defineProps<{ lanes: Lane[] }>()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

/** «2 потока · пауза 2 с» — сколько задач полоса делает разом и сколько ждёт между ними */
function laneMode(l: Lane) {
  const workers = lang.value === 'en'
    ? `${l.concurrency} ${l.concurrency === 1 ? 'worker' : 'workers'}`
    : `${l.concurrency}\u00a0${plural(l.concurrency, lang.value === 'uk' ? ['потік', 'потоки', 'потоків'] : ['поток', 'потока', 'потоков'])}`
  if (!l.minIntervalMs) return workers
  const sec = (l.minIntervalMs / 1000).toLocaleString(intlLocale(lang.value, 'en-US'), { maximumFractionDigits: 1 })
  return `${workers} · ${t('pause', { s: sec })}`
}
const hhmm = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`
const windowOf = (l: Lane) => (l.activeFrom != null && l.activeUntil != null ? `${hhmm(l.activeFrom)}–${hhmm(l.activeUntil)}` : null)

async function toggle(l: Lane, paused: boolean) {
  const was = l.paused
  l.paused = paused
  try {
    await bridge.admin.updateLane(l.lane, { paused })
    toast.show({ text: paused ? t('paused', { title: l.title }) : t('resumed', { title: l.title }), icon: paused ? 'ph:pause-circle' : 'ph:play-circle', undo: () => void toggle(l, was) })
  }
  catch {
    l.paused = was
    toast.error(t('failed'))
  }
}
</script>

<template>
  <div class="ap-table lt" role="table" :aria-label="t('title')">
    <div class="ap-tr ap-tr--head" role="row">
      <span role="columnheader">{{ t('lane') }}</span>
      <span role="columnheader" class="ap-num">{{ t('queued') }}</span>
      <span role="columnheader" class="ap-num">{{ t('running') }}</span>
      <span role="columnheader" class="ap-num lt__opt">{{ t('failed24') }}</span>
      <span role="columnheader" class="ap-num lt__opt">{{ t('done') }}</span>
      <span role="columnheader" class="lt__mode">{{ t('mode') }}</span>
      <span role="columnheader" class="sr-only">{{ t('state') }}</span>
    </div>
    <div v-for="l in lanes" :key="l.lane" class="ap-tr" :class="{ 'is-paused': l.paused || !l.open }" role="row">
      <span role="cell" class="ap-cell">
        <b class="lt__name">{{ l.title }}<span v-if="l.paused" class="lt__tag">{{ t('pausedLabel') }}</span><span v-else-if="!l.open" class="lt__tag">{{ t('closed') }}</span></b>
        <small class="lt__mode-sm">{{ laneMode(l) }}<template v-if="windowOf(l)"> · {{ windowOf(l) }}</template></small>
      </span>
      <span role="cell" class="ap-num">{{ l.queued ? formatNumber(l.queued, lang) : '—' }}</span>
      <span role="cell" class="ap-num" :class="{ 'ap-on': l.running }">{{ l.running || '—' }}</span>
      <span role="cell" class="ap-num lt__opt" :class="{ 'ap-bad': l.failed }">{{ l.failed || '—' }}</span>
      <span role="cell" class="ap-num lt__opt">{{ l.done24h ? formatNumber(l.done24h, lang) : '—' }}</span>
      <span role="cell" class="lt__mode ap-muted">{{ laneMode(l) }}<template v-if="windowOf(l)"><br>{{ t('window', { w: windowOf(l) }) }}</template></span>
      <span role="cell" class="ap-end">
        <UiSwitch :model-value="!l.paused" :label="t(l.paused ? 'switchPaused' : 'switchRunning', { title: l.title })" hide-label @update:model-value="v => toggle(l, !v)" />
      </span>
    </div>
  </div>
</template>

<style scoped>
.lt { --cols: minmax(150px, 1.6fr) repeat(4, minmax(52px, 0.45fr)) minmax(150px, 1.3fr) 52px; }
.lt__name { display: flex; align-items: baseline; gap: 8px; }
.lt__tag { flex: none; font-size: var(--fs-caption); font-weight: 500; color: var(--text-3); }
.ap-tr.is-paused .lt__name { color: var(--text-2); }
.lt__mode { padding-left: 16px; font-size: var(--fs-caption); line-height: 1.5; }
.lt__mode-sm { display: none; }
@media (max-width: 899px) {
  .lt { --cols: minmax(0, 1fr) 52px 52px 52px; }
  .lt__opt, .lt__mode { display: none; }
  .lt__mode-sm { display: block; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Полосы очереди",
    "lane": "Полоса",
    "queued": "Ждут",
    "running": "Идут",
    "failed24": "Ошибки",
    "done": "За сутки",
    "mode": "Как работает",
    "state": "Включена",
    "pause": "пауза {s} с",
    "window": "только {w}",
    "closed": "вне расписания",
    "switchRunning": "«{title}» работает",
    "switchPaused": "«{title}» на паузе",
    "pausedLabel": "на паузе",
    "paused": "«{title}» на паузе",
    "resumed": "«{title}» снова работает",
    "failed": "Не получилось изменить полосу"
  },
  "uk": {
    "title": "Смуги черги",
    "lane": "Смуга",
    "queued": "Чекають",
    "running": "Ідуть",
    "failed24": "Помилки",
    "done": "За добу",
    "mode": "Як працює",
    "state": "Увімкнено",
    "pause": "пауза {s} с",
    "window": "лише {w}",
    "closed": "поза розкладом",
    "switchRunning": "«{title}» працює",
    "switchPaused": "«{title}» на паузі",
    "pausedLabel": "на паузі",
    "paused": "«{title}» на паузі",
    "resumed": "«{title}» знову працює",
    "failed": "Не вдалося змінити смугу"
  },
  "en": {
    "title": "Queue lanes",
    "lane": "Lane",
    "queued": "Queued",
    "running": "Running",
    "failed24": "Failed",
    "done": "24 h",
    "mode": "How it runs",
    "state": "Enabled",
    "pause": "{s} s pause",
    "window": "only {w}",
    "closed": "off schedule",
    "switchRunning": "“{title}” is running",
    "switchPaused": "“{title}” is paused",
    "pausedLabel": "paused",
    "paused": "“{title}” paused",
    "resumed": "“{title}” running again",
    "failed": "Couldn't change the lane"
  }
}
</i18n>
