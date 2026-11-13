<script setup lang="ts">
import type { ChartSummary } from '~/api/types'

// «Чарты» на главной поиска: Россия и мир крупными плитками, «Все» — страница всех чартов
const bridge = useBridge()
const { t } = useI18n()
const items = ref<ChartSummary[]>([])
const failed = ref(false)

onMounted(async () => {
  try {
    items.value = (await bridge.catalog.charts()).items.filter(c => c.kind !== 'genre')
  }
  catch {
    failed.value = true
  }
})
</script>

<template>
  <section v-if="!failed" class="sch" :aria-label="t('charts.title')">
    <header class="sch__head">
      <h2 class="sch__title">{{ t('charts.title') }}</h2>
      <UiButton to="/charts" variant="ghost" size="sm">{{ t('common.all') }}</UiButton>
    </header>
    <div class="sch__grid">
      <template v-if="!items.length">
        <UiSkeleton v-for="i in 2" :key="i" class="sch__skel" />
      </template>
      <MusicChartTile v-for="c in items" :key="c.id" :chart="c" large />
    </div>
  </section>
</template>

<style scoped>
.sch { display: flex; flex-direction: column; gap: 12px; }
.sch__head { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.sch__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.sch__grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.sch__skel { height: 148px; border-radius: var(--r-3); }
@media (max-width: 699px) {
  .sch__title { font-size: 20px; }
  .sch__grid { gap: 10px; }
  .sch__skel { height: 120px; }
}
</style>
