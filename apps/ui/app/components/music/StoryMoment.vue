<script setup lang="ts">
import type { SegmentOption } from '~/components/ui/SegmentedControl.vue'
import { STORY_LENGTHS, type StoryLength, clampStart, dragStart, keyStart } from '~/lib/story/moment'
import { barsPath } from '~/utils/scrubber'

// Какой момент трека попадёт в историю: форма волны и окно фиксированной длины поверх. Окно тянут
// пальцем или мышью (взять можно и за пределами — окно встанет под палец), двигают стрелками
// (Shift — шагом 5 с); «Начало» и «Припев» — быстрые места. Кнопка слева проигрывает выбранное
const props = defineProps<{
  /** Форма волны, 0…1; пустая — ещё грузится */
  peaks: number[]
  /** Длина всего, что можно выбрать: трек или превью */
  totalMs: number
  /** Где припев; null — неизвестно */
  chorusMs: number | null
  /** Звучит выбранное; `playheadMs` — сколько уже сыграно от начала окна */
  playing: boolean
  playheadMs: number | null
  /** Строка текста в начале выбранного; undefined — у трека нет текста со временем, строки нет совсем */
  lyric?: string | null
  disabled?: boolean
}>()

const emit = defineEmits<{ toggle: [], moved: [] }>()
const start = defineModel<number>('start', { required: true })
const length = defineModel<StoryLength>('length', { required: true })

const { t } = useScopedI18n('components.music.StoryMoment')
const bridge = useBridge()

const lengthOptions = computed<SegmentOption<string>[]>(() =>
  STORY_LENGTHS.map(l => ({ value: String(l), label: t('sec', { n: l / 1000 }), disabled: l > props.totalMs })))
const lengthModel = computed({
  get: () => String(length.value),
  set: (v: string) => {
    length.value = Number(v) as StoryLength
  },
})
// Окно длиннее звука (превью 30 с и окно 30 с) — двигать нечего
const fixed = computed(() => props.totalMs <= length.value)
watch([length, () => props.totalMs], () => {
  start.value = clampStart(start.value, length.value, props.totalMs)
})

const d = computed(() => barsPath(props.peaks))
const total = computed(() => Math.max(1, props.totalMs))
const x0 = computed(() => start.value / total.value)
const w = computed(() => Math.min(1, length.value / total.value))
const chorusX = computed(() => (props.chorusMs != null && props.chorusMs < props.totalMs ? props.chorusMs / total.value : null))
const head = computed(() => (props.playheadMs != null ? Math.min(1, props.playheadMs / length.value) : null))
const atChorus = computed(() => props.chorusMs != null && start.value === clampStart(props.chorusMs, length.value, props.totalMs))
const range = computed(() => `${formatDuration(start.value)} – ${formatDuration(start.value + Math.min(length.value, props.totalMs))}`)

const wave = ref<HTMLElement | null>(null)
let drag: { id: number, grab: number } | null = null
const dragging = ref(false)

function ratioAt(e: PointerEvent): number {
  const r = wave.value!.getBoundingClientRect()
  return Math.min(1, Math.max(0, (e.clientX - r.left) / Math.max(1, r.width)))
}

function onDown(e: PointerEvent) {
  if (props.disabled || fixed.value || !e.isPrimary || (e.pointerType === 'mouse' && e.button !== 0)) return
  const at = ratioAt(e)
  const inside = at >= x0.value && at <= x0.value + w.value
  // За окно — тянем с того места, где взяли; мимо — окно встаёт серединой под палец
  drag = { id: e.pointerId, grab: inside ? (at - x0.value) / w.value : 0.5 }
  dragging.value = true
  wave.value?.setPointerCapture(e.pointerId)
  wave.value?.focus({ preventScroll: true })
  move(e)
}
function move(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return
  const next = dragStart(ratioAt(e), drag.grab, length.value, props.totalMs)
  if (next !== start.value) start.value = next
}
function onUp(e: PointerEvent) {
  if (!drag || e.pointerId !== drag.id) return
  drag = null
  dragging.value = false
  bridge.haptic('selection')
  emit('moved')
}

function onKey(e: KeyboardEvent) {
  if (props.disabled || fixed.value) return
  const next = keyStart(e.key, e.shiftKey, start.value, length.value, props.totalMs)
  if (next == null) return
  e.preventDefault()
  if (next !== start.value) {
    start.value = next
    emit('moved')
  }
}

function jump(ms: number) {
  start.value = clampStart(ms, length.value, props.totalMs)
  bridge.haptic('selection')
  emit('moved')
}
</script>

<template>
  <div class="moment" :class="{ 'is-disabled': disabled }">
    <UiSegmentedControl v-model="lengthModel" :options="lengthOptions" :label="t('length')" block />

    <div
      ref="wave"
      class="moment__wave"
      :class="{ 'is-drag': dragging, 'is-fixed': fixed }"
      role="slider"
      :tabindex="disabled ? -1 : 0"
      :aria-label="t('moment')"
      :aria-valuemin="0"
      :aria-valuemax="Math.max(0, Math.round((totalMs - length) / 1000))"
      :aria-valuenow="Math.round(start / 1000)"
      :aria-valuetext="range"
      :aria-disabled="disabled || fixed || undefined"
      :style="{ '--x': x0, '--w': w }"
      @pointerdown="onDown"
      @pointermove="move"
      @pointerup="onUp"
      @pointercancel="onUp"
      @keydown="onKey"
    >
      <template v-if="peaks.length">
        <svg class="moment__bars moment__bars--base" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
        <svg class="moment__bars moment__bars--on" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true"><path :d="d" /></svg>
      </template>
      <span v-else class="moment__loading" aria-hidden="true" />
      <span class="moment__window" aria-hidden="true">
        <span v-if="head != null" class="moment__head" :style="{ left: `${head * 100}%` }" />
      </span>
      <span v-if="chorusX != null" class="moment__chorus" :style="{ left: `${chorusX * 100}%` }" aria-hidden="true" />
    </div>

    <div class="moment__row">
      <button type="button" class="moment__play" :aria-label="playing ? t('stop') : t('play')" :disabled="disabled" @click="emit('toggle')">
        <Icon :name="playing ? 'ph:stop-fill' : 'ph:play-fill'" />
      </button>
      <span class="moment__time tnum">{{ range }}</span>
      <span class="moment__jumps">
        <UiChip :selected="start === 0" :disabled="disabled || fixed" @click="jump(0)">{{ t('start') }}</UiChip>
        <UiChip v-if="chorusMs != null" :selected="atChorus" :disabled="disabled || fixed" @click="jump(chorusMs)">{{ t('chorus') }}</UiChip>
      </span>
    </div>

    <p v-if="lyric !== undefined" class="moment__lyric">{{ lyric || '\u00A0' }}</p>
  </div>
</template>

<style scoped>
.moment { display: flex; flex-direction: column; gap: 14px; width: 100%; }
.moment.is-disabled { pointer-events: none; }

.moment__wave {
  position: relative;
  height: 76px;
  margin: 0 2px;
  border-radius: var(--r-2, 10px);
  cursor: grab;
  touch-action: none;
  user-select: none;
  -webkit-user-select: none;
  outline: none;
}
.moment__wave.is-drag { cursor: grabbing; }
.moment__wave.is-fixed { cursor: default; }
.moment__wave:focus-visible .moment__window { box-shadow: 0 0 0 2px var(--text-1), 0 0 0 5px var(--focus); }

.moment__bars { position: absolute; inset: 10px 0; width: 100%; height: calc(100% - 20px); overflow: visible; pointer-events: none; }
.moment__bars path { fill: none; stroke: currentColor; stroke-width: 2.5px; stroke-linecap: round; vector-effect: non-scaling-stroke; }
.moment__bars--base { color: rgba(var(--fg-rgb), 0.2); }
.moment__bars--on { color: var(--text-1); clip-path: inset(0 calc((1 - var(--x) - var(--w)) * 100%) 0 calc(var(--x) * 100%)); }
.moment__loading { position: absolute; inset: 37px 0; border-radius: 2px; background: var(--fill-2); animation: moment-pulse 1.2s var(--ease-in-out) infinite alternate; }
@keyframes moment-pulse { to { opacity: 0.4; } }

/* Окно: рамка, как у обрезки видео в «Фото»; ручки — только знак, что окно тянется */
.moment__window {
  position: absolute;
  top: 0;
  bottom: 0;
  left: calc(var(--x) * 100%);
  width: calc(var(--w) * 100%);
  border-radius: var(--r-2, 10px);
  background: var(--fill-1);
  box-shadow: 0 0 0 2px var(--text-1);
  pointer-events: none;
  transition: box-shadow var(--dur-1);
}
.moment__wave.is-fixed .moment__window { box-shadow: 0 0 0 1px var(--line-strong); }
.moment__wave:not(.is-fixed) .moment__window::before,
.moment__wave:not(.is-fixed) .moment__window::after {
  content: '';
  position: absolute;
  top: 50%;
  width: 3px;
  height: 22px;
  margin-top: -11px;
  border-radius: 2px;
  background: var(--text-1);
}
.moment__window::before { left: -5px; }
.moment__window::after { right: -5px; }
.moment__head { position: absolute; top: 6px; bottom: 6px; width: 2px; margin-left: -1px; border-radius: 1px; background: var(--accent); }
.moment__chorus { position: absolute; bottom: -9px; width: 6px; height: 6px; margin-left: -3px; border-radius: 1.5px; background: var(--accent); transform: rotate(45deg); pointer-events: none; }

.moment__row { display: flex; align-items: center; gap: 12px; min-height: 44px; }
.moment__play { display: grid; flex: none; place-items: center; width: 44px; height: 44px; padding: 0; border: 0; border-radius: 50%; background: var(--text-1); color: var(--on-light); font-size: 20px; transition: transform var(--dur-1) var(--ease-out); }
.moment__play:active { transform: scale(0.94); }
.moment__play:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.moment__time { font-size: var(--fs-small); font-weight: 600; color: var(--text-1); white-space: nowrap; }
.moment__jumps { display: flex; gap: 6px; margin-left: auto; }
.moment__lyric { margin: -4px 0 0; overflow: hidden; font-size: var(--fs-small); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
</style>
