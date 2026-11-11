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
  <RadioGroupRoot :model-value="model" class="moods" :class="`moods--${variant}`" :aria-label="t('wave.mood.label')" orientation="horizontal" @update:model-value="update">
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
.mood[aria-checked='true'] .mood__orb { outline-color: var(--text-1); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.2), 0 0 18px 1px color-mix(in oklab, var(--o2) 65%, transparent); }
.mood:focus-visible { outline: none; }
.mood:focus-visible .mood__orb { outline-color: var(--focus); }
.mood__label { font-size: var(--fs-small); font-weight: 550; white-space: nowrap; }
.moods--compact .mood__label { display: none; }
.moods--full { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 8px; }
.moods--full .mood {
  flex-direction: column;
  gap: 8px;
  padding: 12px 2px 10px;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
}
.moods--full .mood__orb { width: 30px; height: 30px; }
.moods--full .mood[aria-checked='true'] { background: var(--fill-2); box-shadow: inset 0 0 0 1px var(--line); }
.moods--full .mood__label { max-width: 100%; overflow: hidden; font-size: 11.5px; letter-spacing: -0.01em; text-overflow: ellipsis; }
.moods--full .mood { min-width: 0; padding-left: 0; padding-right: 0; }
</style>
