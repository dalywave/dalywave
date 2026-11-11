<script setup lang="ts">
import type { PendingState } from '~/stores/player'

// Трек выбран, а файла на сервере ещё нет: «скачивается» (с местом в очереди) или «ждём загрузчик на Mac»
const props = withDefaults(defineProps<{
  pending: Pick<PendingState, 'status' | 'queuePosition'>
  long?: boolean
}>(), {})
const { t } = useI18n()

const text = computed(() => {
  const p = props.pending
  if (p.status === 'waiting_for_downloader') return props.long ? t('player.waitingDownloaderLong') : t('player.waitingDownloader')
  if (props.long) return t('player.acquiringLong')
  return p.queuePosition ? t('player.acquiringQueue', { n: p.queuePosition }) : t('player.acquiring')
})
</script>

<template>
  <span class="pend" :class="{ 'pend--wait': pending.status === 'waiting_for_downloader', 'pend--long': long }" role="status">
    <span v-if="pending.status === 'acquiring'" class="pend__spin" aria-hidden="true" />
    <Icon v-else name="ph:laptop" class="pend__icon" />
    <span class="pend__text">{{ text }}</span>
  </span>
</template>

<style scoped>
.pend { display: inline-flex; align-items: center; gap: 6px; min-width: 0; color: var(--accent); font-size: var(--fs-caption); font-weight: 550; }
.pend--wait { color: var(--warning); }
.pend__text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.pend--long .pend__text { white-space: normal; text-wrap: balance; }
.pend__icon { flex: none; font-size: 14px; }
.pend__spin { width: 11px; height: 11px; flex: none; border-radius: 50%; border: 1.5px solid color-mix(in oklab, currentColor 30%, transparent); border-top-color: currentColor; animation: spin 0.9s linear infinite; }
</style>
