<script setup lang="ts">
// Позиция в треке поверх формы волны. Ромб — где начнётся переход к следующему треку:
// после него полосы окрашены акцентом. Перетаскивание, клавиши, подсказка времени под курсором
const props = withDefaults(defineProps<{
  peaks: number[]
  positionMs: number
  durationMs: number
  /** Где начнётся переход; null — без маркера */
  transitionMs?: number | null
  /** «Умный», «Плавный» — для подсказки у маркера */
  transitionLabel?: string
  size?: 'md' | 'lg'
  /** Время по краям (панель) или под формой (телефон) */
  times?: 'sides' | 'below' | 'none'
  disabled?: boolean
  label?: string
  forceHover?: boolean
}>(), { transitionMs: null, size: 'md', times: 'sides', label: 'Позиция в треке' })

const emit = defineEmits<{ seek: [ms: number] }>()

const root = ref<HTMLElement | null>(null)
const drag = ref<number | null>(null)
const hover = ref<number | null>(null)

const clamp01 = (v: number) => Math.min(1, Math.max(0, v))
const dur = computed(() => Math.max(1, props.durationMs))
const p = computed(() => drag.value ?? clamp01(props.positionMs / dur.value))
const x = computed(() => (props.transitionMs != null && props.transitionMs < props.durationMs ? clamp01(props.transitionMs / dur.value) : null))

// Полосы: x растягивается по ширине, толщина штриха — в пикселях (non-scaling-stroke)
const d = computed(() => {
  const n = props.peaks.length || 1
  const w = 1000 / n
  let out = ''
  for (let i = 0; i < n; i++) {
    const v = Math.max(0.14, Math.min(1, props.peaks[i] ?? 0.3))
    const h = v * 86
    const cx = (i * w + w / 2).toFixed(2)
    out += `M${cx} ${(50 - h / 2).toFixed(1)}V${(50 + h / 2).toFixed(1)}`
  }
  return out
})

const shownMs = computed(() => (drag.value != null ? drag.value * dur.value : props.positionMs))
const leftMs = computed(() => Math.max(0, props.durationMs - shownMs.value))
const nearMarker = computed(() => x.value != null && hover.value != null && Math.abs(hover.value - x.value) < 0.02)
const bubble = computed(() => {
  if (hover.value == null) return null
  if (nearMarker.value) return props.transitionLabel ?? `Переход в ${formatDuration(props.transitionMs ?? 0)}`
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
  <div class="scrub" :class="[`scrub--${times}`, `scrub--${size}`]">
    <span v-if="times === 'sides'" class="scrub__time tnum">{{ formatDuration(shownMs) }}</span>
    <div
      ref="root"
      class="wf"
      :class="{ 'is-hover': forceHover, 'is-drag': drag != null, 'is-disabled': disabled }"
      :style="{ '--p': p, '--x': x ?? 1 }"
      role="slider"
      tabindex="0"
      :aria-label="label"
      :aria-valuemin="0"
      :aria-valuemax="Math.round(durationMs / 1000)"
      :aria-valuenow="Math.round(shownMs / 1000)"
      :aria-valuetext="`${formatDuration(shownMs)} из ${formatDuration(durationMs)}`"
      :aria-disabled="disabled || undefined"
      @pointerdown="onDown"
      @pointermove="onMove"
      @pointerup="onUp"
      @pointercancel="drag = null"
      @pointerleave="hover = null"
      @keydown="onKey"
    >
      <svg class="wf__layer wf__base" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
      <svg v-if="x != null" class="wf__layer wf__zone" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
      <svg class="wf__layer wf__played" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
      <svg v-if="x != null" class="wf__layer wf__played-zone" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
      <span v-if="x != null" class="wf__marker" aria-hidden="true" />
      <span class="wf__head" aria-hidden="true" />
      <span v-if="bubble" class="wf__bubble tnum" :class="{ 'is-marker': nearMarker }" :style="{ left: `${bubbleAt * 100}%` }">{{ bubble }}</span>
    </div>
    <span v-if="times === 'sides'" class="scrub__time tnum">{{ formatDuration(durationMs) }}</span>
    <div v-if="times === 'below'" class="scrub__below tnum">
      <span>{{ formatDuration(shownMs) }}</span>
      <span v-if="x != null && transitionMs != null" class="scrub__mid"><Icon name="ph:vinyl-record" />{{ `Переход в ${formatDuration(transitionMs)}` }}</span>
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

.wf {
  --p: 0;
  --x: 1;
  --stroke: 2px;
  position: relative;
  height: 32px;
  cursor: pointer;
  touch-action: none;
  outline: none;
  border-radius: 4px;
}
.scrub--lg .wf { height: 40px; --stroke: 3px; }
.wf.is-disabled { cursor: default; opacity: 0.5; pointer-events: none; }
.wf:focus-visible { box-shadow: 0 0 0 2px var(--focus); }
.wf__layer { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; }
.wf__layer path { fill: none; stroke: currentColor; stroke-width: var(--stroke); stroke-linecap: round; vector-effect: non-scaling-stroke; }
.wf__base { color: rgba(232, 236, 244, 0.22); }
.wf__zone { color: var(--accent); opacity: 0.5; clip-path: inset(0 0 0 calc(var(--x) * 100%)); }
.wf__played { color: var(--text-1); clip-path: inset(0 calc((1 - var(--p)) * 100%) 0 0); }
.wf__played-zone { color: color-mix(in oklab, var(--accent) 60%, white); clip-path: inset(0 calc((1 - var(--p)) * 100%) 0 calc(var(--x) * 100%)); }
.wf__marker { position: absolute; top: -5px; bottom: -5px; left: calc(var(--x) * 100%); width: 0; pointer-events: none; }
.wf__marker::before { content: ''; position: absolute; left: -0.75px; top: 5px; bottom: 5px; width: 1.5px; border-radius: 2px; background: var(--accent); }
.wf__marker::after { content: ''; position: absolute; left: -4px; top: 0; width: 8px; height: 8px; border-radius: 2px; background: var(--accent); transform: rotate(45deg); box-shadow: 0 0 10px var(--accent); }
.wf__head {
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
  transition: opacity var(--dur-1), transform var(--dur-1) var(--ease-out);
  pointer-events: none;
}
.wf:hover .wf__head, .wf.is-hover .wf__head, .wf:focus-visible .wf__head, .wf.is-drag .wf__head { opacity: 1; transform: none; }
.wf__bubble {
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
.wf__bubble.is-marker { color: var(--accent); }
</style>
