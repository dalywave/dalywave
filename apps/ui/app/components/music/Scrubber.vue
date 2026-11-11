<script setup lang="ts">
import type { ScrubberStyle } from '~/stores/prefs'

// Позиция в треке — в одном из двух видов (выбирается в настройках): «Волна» — форма волны
// с пиками, «Линия» — классическая тонкая полоса. Ромб — где начнётся переход к следующему
// треку: в волне после него полосы окрашены акцентом, на линии — хвост полосы.
// Поведение у обоих одно: нажатие и перетаскивание, клавиши, подсказка времени под курсором
const props = withDefaults(defineProps<{
  peaks: number[]
  positionMs: number
  durationMs: number
  /** До куда трек уже загружен; null — движок не сообщает, полоса буфера не рисуется */
  bufferedMs?: number | null
  /** Где начнётся переход; null — без маркера */
  transitionMs?: number | null
  /** «Переход в 2:26 · Умный» — подсказка у маркера */
  transitionLabel?: string
  /** Вид полосы; без него — как выбрано в настройках */
  variant?: ScrubberStyle
  size?: 'md' | 'lg'
  /** Время по краям (панель) или под полосой (телефон) */
  times?: 'sides' | 'below' | 'none'
  disabled?: boolean
  label?: string
  forceHover?: boolean
}>(), { bufferedMs: null, transitionMs: null, size: 'md', times: 'sides' })

const emit = defineEmits<{ seek: [ms: number] }>()

const prefs = usePrefsStore()
const { t } = useI18n({ useScope: 'local' })
const kind = computed<ScrubberStyle>(() => props.variant ?? prefs.scrubber)

const root = ref<HTMLElement | null>(null)
const drag = ref<number | null>(null)
const hover = ref<number | null>(null)

const clamp01 = (v: number) => Math.min(1, Math.max(0, v))
const dur = computed(() => Math.max(1, props.durationMs))
const p = computed(() => drag.value ?? clamp01(props.positionMs / dur.value))
const x = computed(() => markerRatio(props.transitionMs, props.durationMs))
const b = computed(() => ratioOf(props.bufferedMs, props.durationMs))
const d = computed(() => (kind.value === 'wave' ? barsPath(props.peaks) : ''))

const shownMs = computed(() => (drag.value != null ? drag.value * dur.value : props.positionMs))
const leftMs = computed(() => Math.max(0, props.durationMs - shownMs.value))
const transitionText = computed(() => t('transitionAt', { time: formatDuration(props.transitionMs ?? 0) }))
const nearMarker = computed(() => x.value != null && hover.value != null && Math.abs(hover.value - x.value) < 0.02)
const bubble = computed(() => {
  if (hover.value == null) return null
  if (nearMarker.value) return props.transitionLabel ?? transitionText.value
  return formatDuration(hover.value * dur.value)
})
const bubbleAt = computed(() => (nearMarker.value ? x.value! : hover.value ?? 0))

function ratioAt(e: PointerEvent) {
  const r = root.value!.getBoundingClientRect()
  return clamp01((e.clientX - r.left) / r.width)
}

function onDown(e: PointerEvent) {
  if (props.disabled || e.button !== 0) return
  root.value?.setPointerCapture(e.pointerId)
  drag.value = ratioAt(e)
}

function onMove(e: PointerEvent) {
  const k = ratioAt(e)
  if (e.pointerType === 'mouse') hover.value = k
  if (drag.value != null) drag.value = k
}

function onUp() {
  if (drag.value == null) return
  emit('seek', drag.value * dur.value)
  drag.value = null
}

function onKey(e: KeyboardEvent) {
  if (props.disabled) return
  const step = e.shiftKey ? 15000 : 5000
  const map: Record<string, number> = {
    ArrowLeft: props.positionMs - step,
    ArrowDown: props.positionMs - step,
    ArrowRight: props.positionMs + step,
    ArrowUp: props.positionMs + step,
    PageDown: props.positionMs - dur.value * 0.1,
    PageUp: props.positionMs + dur.value * 0.1,
    Home: 0,
    End: dur.value - 1000,
  }
  const to = map[e.key]
  if (to == null) return
  e.preventDefault()
  emit('seek', Math.max(0, Math.min(dur.value, to)))
}
</script>

<template>
  <div class="scrub" :class="[`scrub--${times}`, `scrub--${size}`, `scrub--${kind}`]">
    <span v-if="times === 'sides'" class="scrub__time tnum">{{ formatDuration(shownMs) }}</span>
    <div
      ref="root"
      class="scrub__bar"
      :class="{ 'is-hover': forceHover || hover != null, 'is-drag': drag != null, 'is-near': nearMarker, 'is-disabled': disabled }"
      :style="{ '--p': p, '--x': x ?? 1, '--b': b ?? 0 }"
      role="slider"
      tabindex="0"
      data-scrub
      :aria-label="label ?? t('seek')"
      :aria-valuemin="0"
      :aria-valuemax="Math.round(durationMs / 1000)"
      :aria-valuenow="Math.round(shownMs / 1000)"
      :aria-valuetext="t('valueText', { pos: formatDuration(shownMs), dur: formatDuration(durationMs) })"
      :aria-disabled="disabled || undefined"
      @pointerdown="onDown"
      @pointermove="onMove"
      @pointerup="onUp"
      @pointercancel="drag = null"
      @pointerleave="hover = null"
      @keydown="onKey"
    >
      <template v-if="kind === 'wave'">
        <svg class="wf__layer wf__base" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
        <svg v-if="x != null" class="wf__layer wf__zone" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
        <svg class="wf__layer wf__played" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
        <svg v-if="x != null" class="wf__layer wf__played-zone" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
        <span v-if="x != null" class="wf__marker" aria-hidden="true" />
      </template>
      <template v-else>
        <span class="ln__track" aria-hidden="true">
          <span v-if="b != null" class="ln__buffer" />
          <span v-if="x != null" class="ln__zone" />
          <span class="ln__fill" />
        </span>
        <span v-if="x != null" class="ln__marker" aria-hidden="true" />
      </template>
      <span class="scrub__head" aria-hidden="true" />
      <span v-if="bubble" class="scrub__bubble tnum" :class="{ 'is-marker': nearMarker }" :style="{ left: `${bubbleAt * 100}%` }">{{ bubble }}</span>
    </div>
    <span v-if="times === 'sides'" class="scrub__time tnum">{{ formatDuration(durationMs) }}</span>
    <div v-if="times === 'below'" class="scrub__below tnum">
      <span>{{ formatDuration(shownMs) }}</span>
      <span v-if="x != null" class="scrub__mid"><Icon name="ph:vinyl-record" />{{ transitionText }}</span>
      <span>−{{ formatDuration(leftMs) }}</span>
    </div>
  </div>
</template>

<style scoped>
.scrub { display: grid; grid-template-columns: 44px minmax(0, 1fr) 44px; align-items: center; gap: 12px; min-width: 0; }
.scrub--below, .scrub--none { display: block; }
.scrub__time { font-size: var(--fs-caption); color: var(--text-3); }
.scrub__time:first-child { text-align: right; }
.scrub__below { display: flex; justify-content: space-between; align-items: center; margin-top: 6px; font-size: var(--fs-caption); color: var(--text-3); }
.scrub__mid { display: inline-flex; align-items: center; gap: 4px; color: var(--accent); font-weight: 600; }

/* ---------- Общее: область нажатия, бегунок, подсказка ---------- */
.scrub__bar {
  --p: 0;
  --x: 1;
  --b: 0;
  position: relative;
  height: 32px;
  cursor: pointer;
  touch-action: none;
  outline: none;
  border-radius: 4px;
}
.scrub__bar.is-disabled { cursor: default; opacity: 0.5; pointer-events: none; }
.scrub__head {
  position: absolute;
  top: 50%;
  left: calc(var(--p) * 100%);
  width: 12px;
  height: 12px;
  margin: -6px 0 0 -6px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 0 0 4px rgba(255, 255, 255, 0.12), 0 2px 6px rgba(0, 0, 0, 0.6);
  opacity: 0;
  transform: scale(0.6);
  transition: opacity var(--dur-1), transform var(--dur-1) var(--ease-out), box-shadow var(--dur-1);
  pointer-events: none;
}
.scrub__bar:hover .scrub__head, .scrub__bar.is-hover .scrub__head, .scrub__bar:focus-visible .scrub__head, .scrub__bar.is-drag .scrub__head { opacity: 1; transform: none; }
.scrub__bubble {
  position: absolute;
  bottom: calc(100% + 8px);
  transform: translateX(-50%);
  padding: 4px 8px;
  border-radius: var(--r-2);
  background: var(--ink-4);
  box-shadow: var(--shadow-pop);
  font-size: var(--fs-caption);
  font-weight: 600;
  white-space: nowrap;
  pointer-events: none;
  animation: fade-in var(--dur-1);
}
.scrub__bubble.is-marker { color: var(--accent); }

/* ---------- Волна ---------- */
.scrub--wave .scrub__bar { --stroke: 2px; }
.scrub--wave.scrub--lg .scrub__bar { height: 40px; --stroke: 3px; }
.scrub--wave .scrub__bar:focus-visible { box-shadow: 0 0 0 2px var(--focus); }
.wf__layer { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; }
.wf__layer path { fill: none; stroke: currentColor; stroke-width: var(--stroke); stroke-linecap: round; vector-effect: non-scaling-stroke; }
.wf__base { color: rgba(232, 236, 244, 0.22); }
.wf__zone { color: var(--accent); opacity: 0.5; clip-path: inset(0 0 0 calc(var(--x) * 100%)); }
.wf__played { color: var(--text-1); clip-path: inset(0 calc((1 - var(--p)) * 100%) 0 0); }
.wf__played-zone { color: color-mix(in oklab, var(--accent) 60%, white); clip-path: inset(0 calc((1 - var(--p)) * 100%) 0 calc(var(--x) * 100%)); }
.wf__marker { position: absolute; top: -5px; bottom: -5px; left: calc(var(--x) * 100%); width: 0; pointer-events: none; }
.wf__marker::before { content: ''; position: absolute; left: -0.75px; top: 5px; bottom: 5px; width: 1.5px; border-radius: 2px; background: var(--accent); }
.wf__marker::after { content: ''; position: absolute; left: -4px; top: 0; width: 8px; height: 8px; border-radius: 2px; background: var(--accent); transform: rotate(45deg); box-shadow: 0 0 10px var(--accent); }

/* ---------- Линия ---------- */
.scrub--line .scrub__bar { --h: 4px; height: 24px; }
.scrub--line.scrub--lg .scrub__bar { height: 20px; }
.scrub--line .scrub__below { margin-top: 2px; }
/* Невидимая область нажатия шире полосы: 32 px под мышь, 44 pt под палец */
.scrub--line .scrub__bar::before { content: ''; position: absolute; inset: -4px 0; }
.scrub--line .scrub__bar:hover, .scrub--line .scrub__bar.is-hover, .scrub--line .scrub__bar.is-drag { --h: 6px; }
.ln__track {
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  height: var(--h);
  border-radius: var(--h);
  background: rgba(232, 236, 244, 0.16);
  overflow: hidden;
  transform: translateY(-50%);
  transition: height var(--dur-1) var(--ease-out);
}
.ln__buffer { position: absolute; top: 0; bottom: 0; left: 0; width: calc(var(--b) * 100%); background: rgba(232, 236, 244, 0.16); }
.ln__zone { position: absolute; top: 0; bottom: 0; left: calc(var(--x) * 100%); right: 0; background: color-mix(in oklab, var(--accent) 55%, transparent); }
.ln__fill { position: absolute; top: 0; bottom: 0; left: 0; width: calc(var(--p) * 100%); border-radius: inherit; background: var(--text-1); transition: background-color var(--dur-1); }
.scrub--line .scrub__bar:hover .ln__fill, .scrub--line .scrub__bar.is-hover .ln__fill, .scrub--line .scrub__bar.is-drag .ln__fill { background: var(--accent); }
.ln__marker {
  position: absolute;
  top: 50%;
  left: calc(var(--x) * 100%);
  width: 8px;
  height: 8px;
  margin: -4px 0 0 -4px;
  border-radius: 2px;
  background: var(--accent);
  box-shadow: 0 0 0 2px var(--ink-1), 0 0 10px var(--accent);
  transform: rotate(45deg);
  transition: transform var(--dur-1) var(--ease-out);
  pointer-events: none;
}
.scrub__bar.is-near .ln__marker { transform: rotate(45deg) scale(1.3); }
.scrub--line .scrub__head { box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6); }
.scrub--line.scrub--lg .scrub__head { width: 14px; height: 14px; margin: -7px 0 0 -7px; }
.scrub--line .scrub__bar:focus-visible .scrub__head { box-shadow: 0 0 0 3px var(--focus), 0 1px 4px rgba(0, 0, 0, 0.6); }
.scrub--line .scrub__bubble { bottom: calc(50% + 14px); }

/* Пальцем — 44 pt. В панели над полосой кнопки управления: область растёт вниз, не заходя на них */
@media (pointer: coarse) {
  .scrub--line .scrub__bar::before { inset: -4px 0 -16px; }
  .scrub--line.scrub--lg .scrub__bar::before { inset: -12px 0; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "seek": "Позиция в треке",
    "transitionAt": "Переход в {time}",
    "valueText": "{pos} из {dur}"
  },
  "en": {
    "seek": "Track position",
    "transitionAt": "Transition at {time}",
    "valueText": "{pos} of {dur}"
  }
}
</i18n>
