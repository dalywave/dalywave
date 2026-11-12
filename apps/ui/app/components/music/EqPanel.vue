<script setup lang="ts">
import { SliderRoot, SliderThumb, SliderTrack } from 'reka-ui'
import { EQ_BANDS_HZ, EQ_MAX_DB, EQ_PRESETS, type EqPresetId } from '~/utils/eq'

// Содержимое эквалайзера: список пресетов и, для «Своего», десять вертикальных полос.
// И в листе на телефоне, и во всплывающей панели на компьютере
const sound = useSoundStore()
const { t, locale } = useI18n()

const PRESETS: EqPresetId[] = [...EQ_PRESETS.map(p => p.id), 'custom']
const custom = computed(() => sound.eq.preset === 'custom')
const label = (hz: number) => (hz >= 1000 ? `${hz / 1000}${locale.value === 'en' ? 'k' : 'к'}` : String(hz))
const dbText = (v: number) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toLocaleString(locale.value === 'en' ? 'en' : 'ru')} ${t('sound.db')}`

function bandModel(i: number) {
  return computed({
    get: () => [sound.eq.gains[i] ?? 0],
    set: (v: number[] | undefined) => {
      if (v?.length) sound.setGain(i, v[0]!)
    },
  })
}
const bands = EQ_BANDS_HZ.map((_, i) => bandModel(i))
// Заливка — от черты нуля до бегунка
function fill(v: number) {
  const h = `${(Math.abs(v) / (2 * EQ_MAX_DB)) * 100}%`
  return v >= 0 ? { bottom: '50%', height: h } : { top: '50%', height: h }
}
</script>

<template>
  <div class="eq">
    <div role="radiogroup" :aria-label="t('sound.eq')" class="eq__list">
      <button
        v-for="id in PRESETS"
        :key="id"
        type="button"
        role="radio"
        class="eq__preset"
        :class="{ 'is-selected': sound.eq.preset === id }"
        :aria-checked="sound.eq.preset === id"
        @click="sound.setPreset(id)"
      >
        <span class="eq__name">{{ t(`sound.preset.${id}`) }}</span>
        <Icon v-if="sound.eq.preset === id" name="ph:check-bold" class="eq__check" />
      </button>
    </div>

    <div v-if="custom" class="eq__custom" data-sheet-nodrag>
      <div class="eq__bands">
        <div v-for="(hz, i) in EQ_BANDS_HZ" :key="hz" class="eq__band">
          <SliderRoot
            v-model="bands[i]!.value"
            orientation="vertical"
            :min="-EQ_MAX_DB"
            :max="EQ_MAX_DB"
            :step="0.5"
            class="eq__slider"
          >
            <SliderTrack class="eq__track">
              <span class="eq__range" :style="fill(sound.eq.gains[i] ?? 0)" />
            </SliderTrack>
            <SliderThumb class="eq__thumb" :aria-label="`${label(hz)} ${t('sound.hz')}`" :aria-valuetext="dbText(sound.eq.gains[i] ?? 0)" />
          </SliderRoot>
          <span class="eq__hz tnum">{{ label(hz) }}</span>
        </div>
      </div>
      <button type="button" class="eq__reset" :disabled="sound.eq.gains.every(g => g === 0)" @click="sound.resetCustom()">{{ t('sound.reset') }}</button>
    </div>
  </div>
</template>

<style scoped>
.eq { display: flex; flex-direction: column; }
.eq__list { display: grid; grid-template-columns: 1fr 1fr; gap: 2px; }
.eq__preset {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  min-height: 36px;
  padding: 6px 10px;
  border: 0;
  border-radius: 10px;
  background: transparent;
  text-align: left;
  color: var(--text-1);
  transition: background-color var(--dur-1);
}
.eq__preset:hover { background: var(--fill-2); }
.eq__preset:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.eq__preset.is-selected { background: var(--fill-1); }
.eq__name { flex: 1; min-width: 0; font-size: var(--fs-body); }
.eq__preset.is-selected .eq__name { font-weight: 600; }
.eq__check { font-size: 16px; color: var(--accent); flex: none; }

.eq__custom { display: flex; flex-direction: column; align-items: stretch; gap: 8px; margin: 6px 4px 0; padding: 14px 6px 6px; border-top: 1px solid var(--hair); }
.eq__bands { display: grid; grid-template-columns: repeat(10, minmax(0, 1fr)); gap: 2px; }
.eq__band { display: flex; flex-direction: column; align-items: center; gap: 8px; min-width: 0; }
.eq__slider { position: relative; display: flex; justify-content: center; width: 28px; height: 116px; touch-action: none; user-select: none; cursor: pointer; }
/* Полоса и черта нуля посередине: подъём — вверх от неё, провал — вниз */
.eq__track { position: relative; width: 4px; height: 100%; border-radius: 4px; background: rgba(232, 236, 244, 0.14); }
.eq__track::after { content: ''; position: absolute; left: -5px; right: -5px; top: 50%; height: 1px; background: var(--line-strong); }
.eq__range { position: absolute; left: 0; width: 100%; border-radius: 2px; background: var(--text-1); }
.eq__thumb { display: block; width: 16px; height: 16px; border-radius: 50%; background: #fff; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.6); transition: transform var(--dur-1) var(--ease-out); }
.eq__thumb:focus-visible { outline: none; box-shadow: 0 0 0 3px var(--focus); }
.eq__slider:hover .eq__range, .eq__slider[data-dragging] .eq__range { background: var(--accent); }
.eq__slider[data-dragging] .eq__thumb { transform: scale(1.12); }
.eq__hz { font-size: var(--fs-micro); color: var(--text-3); white-space: nowrap; }
.eq__reset { align-self: flex-end; min-height: 32px; padding: 0 10px; border: 0; border-radius: var(--r-pill); background: transparent; font-size: var(--fs-small); font-weight: 600; color: var(--text-2); }
.eq__reset:hover:not(:disabled), .eq__reset:focus-visible { background: var(--fill-2); color: var(--text-1); outline: none; }
.eq__reset:disabled { color: var(--text-4); }
/* Под пальцем — списком на всю ширину, как меню iOS */
@media (pointer: coarse) {
  .eq__list { display: flex; flex-direction: column; }
  .eq__preset { min-height: 48px; }
  .eq__preset.is-selected { background: transparent; }
  .eq__slider { height: 168px; width: 32px; }
  .eq__thumb { width: 22px; height: 22px; }
  .eq__reset { min-height: 44px; }
}
</style>
