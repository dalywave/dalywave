<script setup lang="ts">
import type { AcquisitionCandidate } from '~/api/types'

// Кандидат из поиска: название в источнике, оценка с разбором, что вышло при скачивании.
// Плеер YouTube открывается по нажатию — без лишних запросов к YouTube, пока не нужно
const props = defineProps<{
  candidate: AcquisitionCandidate
  index: number
  /** Ожидаемая длительность трека — для разницы «+3 с» */
  expectedMs?: number | null
  /** Клавиша выбора (1–9), если список управляется с клавиатуры */
  hotkey?: string
  selectable?: boolean
  busy?: boolean
  playing?: boolean
}>()
const emit = defineEmits<{ choose: [videoId: string], play: [videoId: string | null] }>()
const { t } = useScopedI18n('components.admin.Candidate')
const lang = useLang()

const c = computed(() => props.candidate)
const isVideo = computed(() => /^[\w-]{11}$/.test(c.value.videoId))
const link = computed(() => c.value.url ?? (isVideo.value ? `https://music.youtube.com/watch?v=${c.value.videoId}` : c.value.videoId))
const score = computed(() => c.value.score ?? 0)
const tone = computed(() => (c.value.rejected ? 'bad' : score.value >= 0.9 ? 'ok' : score.value >= 0.6 ? 'warn' : 'bad'))
const diff = computed(() => {
  if (c.value.durationMs == null || props.expectedMs == null) return null
  const s = Math.round((c.value.durationMs - props.expectedMs) / 1000)
  return s === 0 ? '±0' : `${s > 0 ? '+' : '−'}${Math.abs(s)}`
})

type Part = { score?: number }
const b = computed(() => (c.value.breakdown ?? {}) as Record<string, unknown>)
const weights = computed(() => (b.value.weights ?? {}) as Record<string, number>)
const PARTS = ['duration', 'title', 'artist', 'album', 'source'] as const
const parts = computed(() => PARTS.map((k) => {
  const p = b.value[k] as Part | undefined
  return { key: k, score: typeof p?.score === 'number' ? p.score : null, weight: weights.value[k] ?? null }
}).filter(p => p.score != null))
const penalties = computed(() => {
  const p = b.value.penalties
  if (!Array.isArray(p)) return []
  return p.map(x => (typeof x === 'string' ? x : typeof x === 'object' && x && 'reason' in x ? String((x as { reason: unknown }).reason) : JSON.stringify(x)))
})
const factor = computed(() => (typeof b.value.penaltyFactor === 'number' ? b.value.penaltyFactor : null))
const titleInfo = computed(() => b.value.title as { expected?: string, got?: string, expectedVersion?: string | null, gotVersion?: string | null } | undefined)
const artistInfo = computed(() => b.value.artist as { missing?: string[] } | undefined)

const attempt = computed(() => c.value.attempt)
const pct = (v: number) => `${Math.round(v * 100)}`
const open = ref(false)
</script>

<template>
  <div class="cd" :class="[`is-${tone}`, { 'is-open': open, 'is-playing': playing }]">
    <div class="cd__row">
      <span class="cd__index tnum">{{ hotkey ?? index + 1 }}</span>
      <div class="cd__main">
        <a class="cd__title" :href="link" target="_blank" rel="noopener noreferrer">{{ c.title }}</a>
        <div class="cd__meta">
          <span>{{ c.artists.join(', ') }}</span>
          <span v-if="c.album">{{ c.album }}</span>
          <span v-if="c.kind">{{ t(`kind.${c.kind}`) }}</span>
          <span v-if="c.durationMs != null" class="tnum">{{ formatDuration(c.durationMs) }}<template v-if="diff"> ({{ diff }}&nbsp;{{ t('sec') }})</template></span>
        </div>
        <div v-if="c.rejected" class="cd__note ap-bad">{{ t('rejected', { why: c.rejected }) }}</div>
        <div v-else-if="attempt" class="cd__note" :class="attempt.verified ? 'ap-ok' : attempt.downloaded ? 'ap-warn' : 'ap-bad'">
          <template v-if="!attempt.downloaded">{{ t('notDownloaded') }}{{ attempt.error ? ` · ${attempt.error}` : '' }}</template>
          <template v-else-if="attempt.verified === true">{{ t('verified', { p: pct(attempt.similarity ?? 0) }) }}</template>
          <template v-else-if="attempt.verified === false">{{ t('notVerified', { p: pct(attempt.similarity ?? 0) }) }}</template>
          <template v-else>{{ t('noPreview') }}</template>
        </div>
      </div>
      <button type="button" class="cd__score" :aria-expanded="open" :aria-label="t('breakdown')" @click="open = !open">
        <span class="cd__meter"><i :style="{ width: `${Math.max(4, score * 100)}%` }" /></span>
        <b class="tnum">{{ c.score != null ? pct(score) : '—' }}</b>
        <Icon name="ph:caret-down" class="cd__caret" />
      </button>
      <div class="cd__actions">
        <UiIconButton v-if="isVideo" :icon="playing ? 'ph:stop' : 'ph:play'" :label="playing ? t('stop') : t('play')" size="sm" @click="emit('play', playing ? null : c.videoId)" />
        <UiButton v-if="selectable" size="sm" :loading="busy" @click="emit('choose', c.videoId)">{{ t('choose') }}</UiButton>
      </div>
    </div>
    <div v-if="open" class="cd__detail">
      <dl class="cd__parts">
        <div v-for="p in parts" :key="p.key">
          <dt>{{ t(`part.${p.key}`) }}<small v-if="p.weight != null" class="tnum"> · {{ pct(p.weight) }}%</small></dt>
          <dd><span class="cd__meter cd__meter--sm"><i :style="{ width: `${(p.score ?? 0) * 100}%` }" /></span><b class="tnum">{{ pct(p.score ?? 0) }}</b></dd>
        </div>
        <div v-if="factor != null && factor < 1">
          <dt>{{ t('penalty') }}<small v-if="penalties.length"> · {{ penalties.join(', ') }}</small></dt>
          <dd><b class="tnum">×{{ factor.toLocaleString(intlLocale(lang, 'en-US')) }}</b></dd>
        </div>
      </dl>
      <p v-if="titleInfo?.got && titleInfo.got !== titleInfo.expected" class="cd__why">{{ t('titleGot', { got: titleInfo.got, expected: titleInfo.expected }) }}</p>
      <p v-if="titleInfo?.gotVersion !== titleInfo?.expectedVersion && (titleInfo?.gotVersion || titleInfo?.expectedVersion)" class="cd__why">{{ t('versionGot', { got: titleInfo?.gotVersion ?? '—', expected: titleInfo?.expectedVersion ?? '—' }) }}</p>
      <p v-if="artistInfo?.missing?.length" class="cd__why">{{ t('missing', { names: artistInfo.missing.join(', ') }) }}</p>
    </div>
    <div v-if="playing && isVideo" class="cd__player">
      <iframe :src="`https://www.youtube-nocookie.com/embed/${c.videoId}?autoplay=1&rel=0`" :title="c.title" allow="autoplay; encrypted-media; picture-in-picture" allowfullscreen />
    </div>
  </div>
</template>

<style scoped>
.cd { border-top: 1px solid var(--hair); }
.cd:first-child { border-top: 0; }
.cd__row { display: grid; grid-template-columns: 24px minmax(0, 1fr) 132px auto; align-items: center; gap: 14px; min-height: 60px; padding: 8px 0; }
.cd__index { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 6px; background: var(--fill-1); font-size: 11px; font-weight: 700; color: var(--text-3); }
.cd__main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.cd__title { overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.cd__title:hover { text-decoration: underline; text-underline-offset: 3px; }
.cd__meta { display: flex; flex-wrap: wrap; gap: 2px 12px; font-size: var(--fs-caption); color: var(--text-3); }
.cd__meta span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
.cd__note { font-size: var(--fs-caption); }
.cd__score { display: grid; grid-template-columns: minmax(0, 1fr) 28px 14px; align-items: center; gap: 8px; height: 36px; padding: 0 8px; border: 0; border-radius: var(--r-3); background: none; color: inherit; font: inherit; cursor: pointer; transition: background-color var(--dur-1); }
.cd__score:hover { background: var(--fill-1); }
.cd__score:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.cd__score b { font-size: var(--fs-small); text-align: right; }
.cd__caret { font-size: 14px; color: var(--text-3); transition: transform var(--dur-2) var(--ease-out); }
.cd.is-open .cd__caret { transform: rotate(180deg); }
.cd__meter { display: block; height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-2); }
.cd__meter i { display: block; height: 100%; border-radius: inherit; background: var(--text-3); }
.cd.is-ok .cd__score .cd__meter i { background: var(--success); }
.cd.is-warn .cd__score .cd__meter i { background: var(--warning); }
.cd.is-bad .cd__score .cd__meter i { background: var(--danger); }
.cd__meter--sm { width: 88px; height: 4px; }
.cd__actions { display: flex; align-items: center; justify-content: flex-end; gap: 6px; }
.cd__detail { display: flex; flex-direction: column; gap: 6px; padding: 0 0 14px 38px; }
.cd__parts { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 0 32px; margin: 0; }
.cd__parts > div { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 32px; border-top: 1px solid var(--hair); font-size: var(--fs-caption); }
.cd__parts dt { color: var(--text-2); }
.cd__parts dt small { color: var(--text-3); }
.cd__parts dd { display: flex; align-items: center; gap: 10px; margin: 0; }
.cd__parts dd b { width: 24px; font-weight: 600; text-align: right; }
.cd__why { margin: 0; font-size: var(--fs-caption); color: var(--text-3); }
.cd__player { padding: 0 0 14px 38px; }
.cd__player iframe { display: block; width: min(100%, 480px); aspect-ratio: 16 / 9; border: 0; border-radius: var(--r-3); background: #000; }
@media (max-width: 699px) {
  .cd__row { grid-template-columns: 22px minmax(0, 1fr) auto; align-items: start; row-gap: 6px; }
  .cd__index { margin-top: 1px; }
  .cd__score { grid-column: 2; grid-row: 2; width: 150px; padding: 0; justify-self: start; }
  .cd__actions { grid-column: 3; grid-row: 1 / span 2; }
  .cd__detail, .cd__player { padding-left: 0; }
}
</style>
