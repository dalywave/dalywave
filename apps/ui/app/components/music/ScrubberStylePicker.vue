<script setup lang="ts">
import { ToggleGroupItem, ToggleGroupRoot } from 'reka-ui'
import type { ScrubberStyle } from '~/stores/prefs'

// Выбор вида полосы прокрутки. У каждого варианта — живой образец: позиция бежит, видны
// бегунок, загруженная часть и ромб перехода — сразу понятно, как это будет выглядеть в плеере
defineProps<{ label: string }>()

const model = defineModel<ScrubberStyle>({ required: true })
const { t } = useI18n({ useScope: 'local' })

const OPTIONS: ScrubberStyle[] = ['wave', 'line']
// Огибающая песни для образца: вступление, куплет, припев, спад, второй припев, концовка
const PEAKS = [0.28, 0.4, 0.34, 0.52, 0.6, 0.48, 0.68, 0.86, 0.72, 0.92, 0.78, 0.6, 0.46, 0.56, 0.74, 0.92, 1, 0.82, 0.94, 0.76, 0.6, 0.7, 0.5, 0.38, 0.26, 0.18]

function update(v: unknown) {
  // Повторное нажатие на выбранный вариант не снимает выбор
  if (v === 'wave' || v === 'line') model.value = v
}
</script>

<template>
  <ToggleGroupRoot type="single" :model-value="model" :aria-label="label" class="ssp" @update:model-value="update">
    <ToggleGroupItem v-for="o in OPTIONS" :key="o" :value="o" class="ssp__item">
      <span class="ssp__stage">
        <MusicScrubberMini class="ssp__art" :variant="o" :peaks="PEAKS" :transition="0.8" demo />
      </span>
      <span class="ssp__label">{{ t(o) }}</span>
    </ToggleGroupItem>
  </ToggleGroupRoot>
</template>

<style scoped>
.ssp {
  display: inline-grid;
  grid-template-columns: repeat(2, 136px);
  gap: 4px;
  max-width: 100%;
  padding: 4px;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
}
.ssp__item {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
  min-width: 0;
  padding: 16px 16px 10px;
  border: 0;
  border-radius: var(--r-3);
  background: transparent;
  font-size: var(--fs-small);
  font-weight: 600;
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-2), box-shadow var(--dur-2);
}
.ssp__stage { display: flex; align-items: center; height: 24px; padding: 0 2px; opacity: 0.5; transition: opacity var(--dur-2); }
.ssp__item .ssp__art { flex: 1; height: 20px; --h: 4px; --stroke: 2px; --played: var(--text-1); }
.ssp__label { text-align: center; }
.ssp__item:hover { color: var(--text-1); }
.ssp__item:hover .ssp__stage { opacity: 0.8; }
.ssp__item[data-state='on'] { color: var(--text-1); background: var(--ink-4); box-shadow: 0 2px 8px -2px rgba(0, 0, 0, 0.6), inset 0 0 0 1px var(--line); }
.ssp__item[data-state='on'] .ssp__stage { opacity: 1; }

@media (max-width: 699px) {
  .ssp { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); width: 100%; }
}
</style>

<i18n lang="json">
{
  "ru": { "wave": "Волна", "line": "Линия" },
  "en": { "wave": "Wave", "line": "Line" }
}
</i18n>
