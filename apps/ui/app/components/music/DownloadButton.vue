<script setup lang="ts">
import type { DownloadKind } from '~/bridge'

// «Скачать на устройство» в шапке альбома, плейлиста, «Любимых» и артиста. Только в приложении:
// на сайте кнопки нет. Закреплено — кольцо хода загрузки, всё на устройстве — галочка
const props = withDefaults(defineProps<{
  kind: DownloadKind
  refId?: string | null
  title?: string
  size?: 'md' | 'lg'
}>(), { refId: null, size: 'lg' })

const offline = useOfflineStore()
const { t } = useI18n()

onMounted(() => offline.start())

const pin = computed(() => offline.pinOf(props.kind, props.refId))
const share = computed(() => {
  const p = pin.value
  if (!p || !p.tracksTotal) return 0
  return p.tracksReady / p.tracksTotal
})
const done = computed(() => !!pin.value && pin.value.tracksTotal > 0 && pin.value.tracksReady >= pin.value.tracksTotal)
const label = computed(() => {
  const p = pin.value
  if (!p) return t('offline.downloadAll')
  if (done.value) return `${t('offline.downloaded')} · ${t('offline.remove')}`
  return `${t('offline.downloading', { done: p.tracksReady, total: p.tracksTotal })} · ${t('offline.remove')}`
})
</script>

<template>
  <UiTooltip v-if="offline.available" :text="label">
    <button
      type="button"
      class="dlb"
      :class="[`dlb--${size}`, { 'is-on': !!pin, 'is-done': done }]"
      :aria-label="label"
      :aria-pressed="!!pin"
      @click="offline.toggle(kind, refId, title)"
    >
      <UiProgressRing v-if="pin && !done" :value="share" :state="share > 0 ? 'progress' : 'queued'" :size="size === 'lg' ? 26 : 22" :label="label" />
      <Icon v-else :name="done ? 'ph:arrow-circle-down-fill' : 'ph:arrow-circle-down'" class="dlb__icon" />
    </button>
  </UiTooltip>
</template>

<style scoped>
.dlb {
  display: inline-grid;
  place-items: center;
  flex: none;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-2);
  transition: background-color var(--dur-1), color var(--dur-1), transform var(--dur-1);
}
.dlb--lg { width: 48px; height: 48px; }
.dlb--md { width: 40px; height: 40px; }
.dlb__icon { font-size: 30px; }
.dlb--md .dlb__icon { font-size: 24px; }
.dlb:hover { background: var(--fill-1); color: var(--text-1); }
.dlb:active { transform: scale(0.94); }
.dlb:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.dlb.is-on { color: var(--accent); }
</style>
