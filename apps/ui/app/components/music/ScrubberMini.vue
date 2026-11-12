<script setup lang="ts">
import type { ScrubberStyle } from '~/stores/prefs'

// Полоса прокрутки в миниатюре, без управления: прогресс мини-плеера и живые образцы в настройках.
// Вид — тот же, что у большой полосы: форма волны или тонкая линия
const props = withDefaults(defineProps<{
  peaks: number[]
  /** Доля 0…1 */
  progress?: number
  /** Где начнётся переход, доля 0…1 */
  transition?: number | null
  /** До куда загружено, доля 0…1 */
  buffered?: number | null
  /** Вид; без него — как выбрано в настройках */
  variant?: ScrubberStyle
  /** Образец для настроек: позиция бежит сама, бегунок и ромб видны */
  demo?: boolean
}>(), { progress: 0, transition: null, buffered: null })

const prefs = usePrefsStore()
const kind = computed<ScrubberStyle>(() => props.variant ?? prefs.scrubber)
const d = computed(() => (kind.value === 'wave' ? barsPath(props.peaks) : ''))
const style = computed(() => ({
  '--x': props.transition ?? 1,
  ...(props.demo ? {} : { '--p': Math.min(1, Math.max(0, props.progress)), '--b': props.buffered ?? 0 }),
}))
</script>

<template>
  <span class="sm" :class="[`sm--${kind}`, { 'is-demo': demo }]" :style="style" aria-hidden="true">
    <template v-if="kind === 'wave'">
      <svg class="sm__layer sm__base" viewBox="0 0 1000 100" preserveAspectRatio="none"><path :d="d" /></svg>
      <svg v-if="transition != null" class="sm__layer sm__zone" viewBox="0 0 1000 100" preserveAspectRatio="none"><path :d="d" /></svg>
      <svg class="sm__layer sm__played" viewBox="0 0 1000 100" preserveAspectRatio="none"><path :d="d" /></svg>
    </template>
    <template v-else>
      <span class="sm__track">
        <span v-if="buffered != null || demo" class="sm__buffer" />
        <span v-if="transition != null" class="sm__tail" />
        <span class="sm__fill" />
      </span>
      <span v-if="demo" class="sm__thumb" />
    </template>
    <span v-if="demo && transition != null" class="sm__marker" />
  </span>
</template>

<style scoped>
/* Позиция образца бежит сама; при «уменьшить движение» стоит на середине */
@property --sm-demo { syntax: '<number>'; inherits: true; initial-value: 0.56; }
@keyframes sm-demo { from { --sm-demo: 0.06; } to { --sm-demo: 0.94; } }

.sm {
  --p: 0;
  --x: 1;
  --b: 0;
  --h: 3px;
  --stroke: 1.5px;
  --played: var(--accent);
  position: relative;
  display: block;
  height: 6px;
  pointer-events: none;
}
.sm--line { height: var(--h); }
.sm.is-demo { --p: var(--sm-demo); --b: min(1, calc(var(--sm-demo) + 0.24)); animation: sm-demo 10s linear infinite; }

.sm__layer { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; }
.sm__layer path { fill: none; stroke: currentColor; stroke-width: var(--stroke); stroke-linecap: round; vector-effect: non-scaling-stroke; }
.sm__base { color: rgba(232, 236, 244, 0.22); }
.sm__zone { color: var(--accent); opacity: 0.5; clip-path: inset(0 0 0 calc(var(--x) * 100%)); }
.sm__played { color: var(--played); clip-path: inset(0 calc((1 - var(--p)) * 100%) 0 0); }

.sm__track { position: absolute; left: 0; right: 0; top: 50%; height: var(--h); border-radius: var(--h); background: rgba(232, 236, 244, 0.16); overflow: hidden; transform: translateY(-50%); }
.sm__buffer { position: absolute; top: 0; bottom: 0; left: 0; width: calc(var(--b) * 100%); background: rgba(232, 236, 244, 0.16); }
.sm__tail { position: absolute; top: 0; bottom: 0; left: calc(var(--x) * 100%); right: 0; background: color-mix(in oklab, var(--accent) 55%, transparent); }
.sm__fill { position: absolute; top: 0; bottom: 0; left: 0; width: calc(var(--p) * 100%); border-radius: inherit; background: var(--played); }
.sm__thumb { position: absolute; top: 50%; left: calc(var(--p) * 100%); width: 10px; height: 10px; margin: -5px 0 0 -5px; border-radius: 50%; background: #fff; box-shadow: 0 1px 3px rgba(0, 0, 0, 0.6); }

/* Ромб перехода: у волны — над чертой во всю высоту, у линии — на самой полосе с вырезом вокруг */
.sm__marker { position: absolute; top: 0; bottom: 0; left: calc(var(--x) * 100%); width: 0; }
.sm__marker::after { content: ''; position: absolute; left: -3px; top: 50%; width: 6px; height: 6px; margin-top: -3px; border-radius: 1.5px; background: var(--accent); transform: rotate(45deg); }
.sm--line .sm__marker::after { box-shadow: 0 0 0 1.5px var(--ink-2); }
.sm--wave .sm__marker { top: -5px; }
.sm--wave .sm__marker::before { content: ''; position: absolute; left: -0.5px; top: 5px; bottom: 0; width: 1px; background: var(--accent); }
.sm--wave .sm__marker::after { top: 0; margin-top: 0; }
</style>
