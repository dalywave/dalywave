<script setup lang="ts">
import type { AdminAcquisition } from '~/api/types'

// История скачивания записи: кандидаты с разбором оценки и попытки с решением сервера.
// В «Загрузках» — только смотреть; в «Спорных» кандидата можно выбрать (selectable)
const props = defineProps<{
  acq: AdminAcquisition
  selectable?: boolean
  /** id видео, которое сейчас выбирается */
  busy?: string | null
  /** Подписи клавиш у кандидатов: 1–9 */
  hotkeys?: boolean
  /** Имена загрузчиков по id — для колонки «кто качал» */
  names?: Record<string, string>
}>()
const emit = defineEmits<{ choose: [videoId: string] }>()
const { t } = useI18n({ useScope: 'local' })
const L = useAdminLabels()
const lang = useLang()

const playing = ref<string | null>(null)
watch(() => props.acq.acquisition.id, () => (playing.value = null))
const candidates = computed(() => props.acq.candidates)
const showAll = ref(false)
const shown = computed(() => (showAll.value ? candidates.value : candidates.value.slice(0, 6)))
defineExpose({ play: (videoId: string | null) => (playing.value = videoId) })
</script>

<template>
  <div class="aq">
    <section class="aq__block" :aria-label="t('candidates')">
      <h3 class="aq__title">{{ t('candidates') }}<span class="ap-muted tnum">{{ candidates.length }}</span></h3>
      <p v-if="!candidates.length" class="aq__empty">{{ t('noCandidates') }}</p>
      <div v-else class="aq__list">
        <AdminCandidate
          v-for="(c, i) in shown"
          :key="c.videoId"
          :candidate="c"
          :index="i"
          :hotkey="hotkeys && i < 9 ? String(i + 1) : undefined"
          :expected-ms="acq.track?.durationMs"
          :selectable="selectable"
          :busy="busy === c.videoId"
          :playing="playing === c.videoId"
          @play="v => (playing = v)"
          @choose="v => emit('choose', v)"
        />
        <div v-if="candidates.length > 6" class="ap-more">
          <UiButton variant="ghost" size="sm" @click="showAll = !showAll">{{ showAll ? t('less') : t('all', { n: candidates.length }) }}</UiButton>
        </div>
      </div>
    </section>
    <section v-if="acq.attemptsLog.length" class="aq__block" :aria-label="t('attempts')">
      <h3 class="aq__title">{{ t('attempts') }}<span class="ap-muted tnum">{{ acq.attemptsLog.length }}</span></h3>
      <div class="ap-table aq__attempts" style="--cols: 112px 132px minmax(0, 1fr) 80px">
        <div v-for="a in acq.attemptsLog" :key="a.id" class="ap-tr">
          <span class="ap-muted tnum">{{ formatDateTime(a.createdAt, lang) }}</span>
          <span :class="a.outcome === 'ready' ? 'ap-ok' : a.outcome === 'disputed' || a.outcome === 'postponed' ? 'ap-warn' : 'ap-bad'">{{ L.outcome(a.outcome) }}</span>
          <span class="ap-cell">
            <b>{{ L.executor(a.executor, names) }}<template v-if="a.videoId"> · {{ a.videoId }}</template></b>
            <small v-if="a.error" :title="a.error">{{ a.error }}</small>
            <small v-else-if="a.ytDlpVersion">yt-dlp {{ a.ytDlpVersion }}</small>
          </span>
          <span class="ap-num ap-muted">{{ a.totalMs != null ? `${(a.totalMs / 1000).toLocaleString(intlLocale(lang, 'en-US'), { maximumFractionDigits: 1 })} ${t('sec')}` : '' }}</span>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.aq { display: flex; flex-direction: column; gap: 24px; min-width: 0; }
.aq__block { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.aq__title { display: flex; align-items: baseline; gap: 8px; margin: 0 0 4px; font-size: 11px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.aq__empty { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
@media (max-width: 699px) {
  .aq__attempts { --cols: 96px minmax(0, 1fr) !important; }
  .aq__attempts .ap-tr > :nth-child(3) { grid-column: 1 / -1; }
  .aq__attempts .ap-tr > :nth-child(4) { display: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "candidates": "Кандидаты",
    "noCandidates": "Поиск ничего не нашёл",
    "attempts": "Попытки",
    "sec": "с",
    "all": "Все {n}",
    "less": "Свернуть"
  },
  "uk": {
    "candidates": "Кандидати",
    "noCandidates": "Пошук нічого не знайшов",
    "attempts": "Спроби",
    "sec": "с",
    "all": "Усі {n}",
    "less": "Згорнути"
  },
  "en": {
    "candidates": "Candidates",
    "noCandidates": "The search found nothing",
    "attempts": "Attempts",
    "sec": "s",
    "all": "All {n}",
    "less": "Collapse"
  }
}
</i18n>
