<script setup lang="ts">
// Загрузка, ошибка и пустота блока админки — одинаково на всех страницах
withDefaults(defineProps<{
  loading?: boolean
  error?: boolean
  empty?: boolean
  emptyIcon?: string
  emptyTitle?: string
  emptyText?: string
  rows?: number
}>(), { rows: 4, emptyIcon: 'ph:check-circle' })
defineEmits<{ retry: [] }>()
const { t } = useI18n({ useScope: 'global' })
</script>

<template>
  <div v-if="error" class="st">
    <UiEmptyState icon="ph:cloud-warning" :title="t('admin.failed')" tone="error" compact>
      <UiButton icon="ph:arrow-clockwise" @click="$emit('retry')">{{ t('admin.retry') }}</UiButton>
    </UiEmptyState>
  </div>
  <div v-else-if="loading" class="st st--loading" aria-busy="true">
    <UiSkeleton v-for="n in rows" :key="n" variant="line" height="16px" :width="`${92 - (n % 3) * 14}%`" />
  </div>
  <div v-else-if="empty" class="st">
    <UiEmptyState :icon="emptyIcon" :title="emptyTitle ?? ''" :text="emptyText" compact>
      <slot name="empty" />
    </UiEmptyState>
  </div>
  <slot v-else />
</template>

<style scoped>
.st { padding: 8px 0; }
.st--loading { display: flex; flex-direction: column; gap: 20px; padding: 16px 0; }
</style>
