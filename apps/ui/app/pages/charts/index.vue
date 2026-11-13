<script setup lang="ts">
// Все чарты: Россия и мир крупно, жанры — плитками. Вход — из поиска
const bridge = useBridge()
const ui = useUiStore()
const { t } = useI18n()
useHead({ title: () => t('charts.title') })

const { data, state, load } = useLoad(() => bridge.catalog.charts())
const main = computed(() => data.value?.items.filter(c => c.kind !== 'genre') ?? [])
const genres = computed(() => data.value?.items.filter(c => c.kind === 'genre') ?? [])

// Телефон: название уехало под верхнюю строку — она показывает его сама
const titleEl = ref<HTMLElement | null>(null)
const condensed = ref(false)
let io: IntersectionObserver | null = null
watch(titleEl, (el) => {
  io?.disconnect()
  if (!el) return
  io = new IntersectionObserver(([e]) => {
    if (e) condensed.value = !e.isIntersecting && e.boundingClientRect.top < 100
  }, { rootMargin: '-100px 0px 0px 0px' })
  io.observe(el)
})
onBeforeUnmount(() => io?.disconnect())
</script>

<template>
  <div class="charts">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="t('charts.title')" :condensed="condensed" fallback="/search" />
    <h1 ref="titleEl" class="charts__title">{{ t('charts.title') }}</h1>
    <UiEmptyState v-if="state === 'error'" icon="ph:wifi-slash" :title="t('charts.failed')" compact>
      <UiButton icon="ph:arrow-clockwise" @click="load">{{ t('common.retry') }}</UiButton>
    </UiEmptyState>
    <template v-else>
      <div class="charts__main">
        <template v-if="state === 'loading'">
          <UiSkeleton v-for="i in 2" :key="i" class="charts__skel charts__skel--large" />
        </template>
        <MusicChartTile v-for="c in main" :key="c.id" :chart="c" large />
      </div>
      <section class="charts__genres" :aria-label="t('charts.genres')">
        <h2 class="charts__h2">{{ t('charts.genres') }}</h2>
        <div class="charts__grid">
          <template v-if="state === 'loading'">
            <UiSkeleton v-for="i in 8" :key="i" class="charts__skel" />
          </template>
          <MusicChartTile v-for="c in genres" :key="c.id" :chart="c" />
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.charts { display: flex; flex-direction: column; gap: 20px; padding-top: 16px; }
.charts__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.charts__main { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.charts__genres { display: flex; flex-direction: column; gap: 12px; margin-top: 12px; }
.charts__h2 { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.charts__grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 16px; }
.charts__skel { height: 112px; border-radius: var(--r-3); }
.charts__skel--large { height: 148px; }
@container main (max-width: 900px) {
  .charts__grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
}
@media (max-width: 699px) {
  .charts { gap: 16px; padding: 0 16px; }
  .charts > .pn { margin: 0 -16px -8px; }
  .charts__title { font-size: 28px; }
  .charts__main { gap: 10px; }
  .charts__grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .charts__h2 { font-size: 20px; }
  .charts__skel { height: 96px; }
  .charts__skel--large { height: 120px; }
}
</style>
