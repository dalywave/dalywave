<script setup lang="ts">
import { RadioGroupItem, RadioGroupRoot } from 'reka-ui'
import type { WaveMood } from '~/bridge/types'
import { MOOD_OPTIONS, MOOD_ORB } from '~/utils/wave-options'

// Выбор настроения: шары показывают, в какие цвета окрасится волна. «Любое» — весь спектр «Сумерек»
withDefaults(defineProps<{
  variant?: 'compact' | 'full'
  /** Только для галереи: подсветить шар как при наведении */
  hover?: WaveMood
}>(), { variant: 'compact' })

const model = defineModel<WaveMood>({ required: true })
const { t } = useI18n()

function update(v: unknown) {
  if (typeof v === 'string') model.value = v as WaveMood
}
</script>

<template>
  <RadioGroupRoot v-edge-fade="variant === 'full'" :model-value="model" class="moods" :class="[`moods--${variant}`, { 'no-scrollbar': variant === 'full' }]" :aria-label="t('wave.mood.label')" orientation="horizontal" @update:model-value="update">
    <UiTooltip v-for="o in MOOD_OPTIONS" :key="o.value" :text="variant === 'compact' ? t(o.key) : undefined">
      <RadioGroupItem :value="o.value" class="mood" :class="{ 'is-hover': hover === o.value }">
        <span
          class="mood__orb"
          :class="{ 'mood__orb--any': o.value === 'any' }"
          :style="{ '--o1': MOOD_ORB[o.value][0], '--o2': MOOD_ORB[o.value][1], '--o3': MOOD_ORB[o.value][2] }"
        />
        <span class="mood__label">{{ t(o.key) }}</span>
      </RadioGroupItem>
    </UiTooltip>
  </RadioGroupRoot>
</template>

<style scoped>
.moods { display: flex; align-items: center; gap: 10px; }
.mood {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--text-2);
  transition: color var(--dur-2);
}
.mood__orb {
  position: relative;
  width: 22px;
  height: 22px;
  flex: none;
  border-radius: 50%;
  background: radial-gradient(110% 110% at 30% 22%, var(--o3) 0%, var(--o2) 42%, var(--o1) 100%);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  outline: 1.5px solid transparent;
  outline-offset: 3px;
  transition: outline-color var(--dur-2), transform var(--dur-2) var(--ease-spring), box-shadow var(--dur-3);
}
.mood__orb--any { background: radial-gradient(90% 90% at 30% 22%, rgba(255, 255, 255, 0.55), transparent 45%), conic-gradient(from 200deg, #4b21ff, #b01cf0, #ff3d7a, #ff9a3c, #4b21ff); }
.mood:hover .mood__orb, .mood.is-hover .mood__orb { transform: scale(1.12); }
.mood:active .mood__orb { transform: scale(0.94); }
.mood[aria-checked='true'] { color: var(--text-1); }
.mood[aria-checked='true'] .mood__orb { outline-color: var(--text-1); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.2), 0 0 18px 1px color-mix(in oklab, var(--o2) var(--orb-glow, 65%), transparent); }
.mood:focus-visible { outline: none; }
.mood:focus-visible .mood__orb { outline-color: var(--focus); }
.mood__label { font-size: var(--fs-small); font-weight: 550; white-space: nowrap; }
.moods--compact .mood__label { display: none; }
/* Планшет под пальцем: шарики крупнее, у каждого — область 44 pt */
@media (pointer: coarse) and (min-width: 700px) {
  .moods--compact .mood { justify-content: center; min-width: 40px; min-height: 44px; }
  .moods--compact .mood__orb { width: 28px; height: 28px; }
}
/* Плитки на телефоне: лента с постоянной шириной плиток; выходит за поля экрана и гаснет к краю */
.moods--full {
  display: flex;
  gap: 8px;
  margin: 0 -16px;
  padding: 4px 16px;
  overflow-x: auto;
  scroll-snap-type: x proximity;
  scroll-padding-inline: 16px;
  overscroll-behavior-x: contain;
}
.moods--full .mood {
  flex: none;
  flex-direction: column;
  justify-content: center;
  gap: 9px;
  width: 78px;
  height: 84px;
  padding: 0 4px;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  scroll-snap-align: start;
}
.moods--full .mood__orb { width: 30px; height: 30px; }
.moods--full .mood[aria-checked='true'] { background: var(--fill-2); box-shadow: inset 0 0 0 1px var(--line); }
.moods--full .mood__label { font-size: var(--fs-caption); letter-spacing: -0.005em; }
</style>
