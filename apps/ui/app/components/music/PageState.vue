<script setup lang="ts">
// Загрузка и ошибка страницы в одном месте: скелет шапки и строк, «не нашли», «не загрузилось»
withDefaults(defineProps<{
  state: 'loading' | 'error' | 'missing'
  round?: boolean
}>(), {})
defineEmits<{ retry: [] }>()
const { t } = useI18n()
</script>

<template>
  <div v-if="state === 'loading'" class="ps" aria-busy="true" :aria-label="t('common.loading')">
    <div class="ps__head">
      <UiSkeleton class="ps__art" :variant="round ? 'round' : 'block'" />
      <div class="ps__lines">
        <UiSkeleton variant="line" width="90px" />
        <UiSkeleton variant="line" width="min(420px, 70%)" height="40px" />
        <UiSkeleton variant="line" width="220px" />
        <div class="ps__btns"><UiSkeleton width="120px" height="40px" /><UiSkeleton width="40px" height="40px" variant="round" /></div>
      </div>
    </div>
    <div class="ps__rows">
      <div v-for="n in 6" :key="n" class="ps__row"><UiSkeleton width="40px" height="40px" /><div class="ps__lines ps__lines--row"><UiSkeleton variant="line" width="40%" /><UiSkeleton variant="line" width="24%" /></div></div>
    </div>
  </div>
  <UiEmptyState v-else-if="state === 'missing'" icon="ph:magnifying-glass" :title="t('common.notFound')" :text="t('common.notFoundText')">
    <UiButton to="/" icon="ph:house">{{ t('nav.home') }}</UiButton>
  </UiEmptyState>
  <UiEmptyState v-else icon="ph:cloud-warning" :title="t('common.loadFailed')" :text="t('common.loadFailedText')" tone="error">
    <UiButton icon="ph:arrow-clockwise" @click="$emit('retry')">{{ t('common.retry') }}</UiButton>
  </UiEmptyState>
</template>

<style scoped>
.ps { display: flex; flex-direction: column; gap: 40px; padding-top: 24px; }
.ps__head { display: flex; align-items: flex-end; gap: 32px; }
.ps__art { width: 232px; height: 232px; flex: none; border-radius: var(--r-4); }
.ps__lines { display: flex; flex: 1; flex-direction: column; gap: 14px; }
.ps__lines--row { gap: 8px; }
.ps__btns { display: flex; gap: 10px; margin-top: 8px; }
.ps__rows { display: flex; flex-direction: column; gap: 4px; }
.ps__row { display: flex; align-items: center; gap: 12px; padding: 8px 10px; }
@media (max-width: 699px) {
  .ps { padding: 12px 16px 0; }
  .ps__head { flex-direction: column; align-items: center; }
  .ps__art { width: 200px; height: 200px; }
  .ps__lines { align-items: center; width: 100%; }
}
</style>
