<script setup lang="ts">
import type { ChartSummary } from '~/api/types'

// Плитка чарта: ровная заливка, название, «Топ 100» и обложка первого места в углу.
// `large` — Россия и мир в поиске и в начале страницы чартов
const props = defineProps<{ chart: ChartSummary, large?: boolean }>()
const { t, te } = useI18n()
const name = computed(() => (te(`charts.name.${props.chart.id}`) ? t(`charts.name.${props.chart.id}`) : props.chart.title))
</script>

<template>
  <NuxtLink :to="`/charts/${chart.id}`" class="ct" :class="{ 'ct--large': large }" :style="{ '--tile': chartTile(chart.id) }">
    <span class="ct__kicker">{{ t('charts.top') }}</span>
    <span class="ct__name">{{ name }}</span>
    <MusicCoverArt v-if="chart.image" :image="chart.image" size="small" class="ct__art" />
  </NuxtLink>
</template>

<style scoped>
.ct {
  position: relative;
  display: block;
  height: 112px;
  overflow: hidden;
  border-radius: var(--r-3);
  background: var(--tile);
  color: #fff;
  isolation: isolate;
  transition: transform var(--dur-1) var(--ease-out), filter var(--dur-1);
}
.ct::after { content: ''; position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07); pointer-events: none; }
.ct:hover { filter: brightness(1.08); }
.ct:active { transform: scale(0.97); }
.ct:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.ct__kicker { position: absolute; left: 16px; top: 14px; z-index: 2; font-size: var(--fs-micro); font-weight: 700; letter-spacing: var(--tr-caps); text-transform: uppercase; opacity: 0.78; }
.ct__name { position: absolute; left: 16px; top: 32px; right: 40%; z-index: 2; font-size: 17px; font-weight: 700; letter-spacing: -0.01em; line-height: 1.2; text-wrap: balance; }
.ct__art {
  position: absolute;
  right: -12px;
  bottom: -6px;
  width: 72px;
  border-radius: var(--r-1);
  box-shadow: -2px 4px 12px -4px rgba(0, 0, 0, 0.45);
  transform: rotate(24deg);
}
.ct--large { height: 148px; }
.ct--large .ct__name { top: 34px; font-size: 26px; letter-spacing: -0.02em; }
.ct--large .ct__art { width: 104px; right: -14px; bottom: -10px; }
@media (max-width: 699px) {
  .ct { height: 96px; }
  .ct__kicker { left: 12px; top: 12px; }
  .ct__name { left: 12px; top: 28px; font-size: 16px; }
  .ct__art { width: 64px; }
  .ct--large { height: 120px; }
  .ct--large .ct__name { font-size: 22px; }
  .ct--large .ct__art { width: 84px; }
}
</style>
