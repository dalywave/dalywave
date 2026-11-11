<script setup lang="ts">
import type { Section } from '~/bridge/types'

// Только разработка: честная пометка, что раздел отдаёт мок из design/fixtures
const props = defineProps<{ sections: Section[], what?: string }>()
const bridge = useBridge()
const ready = ref(false)
onMounted(async () => {
  await bridge.ready()
  ready.value = true
})
const mocked = computed(() => (import.meta.dev && ready.value ? props.sections.filter(s => bridge.mocked(s)) : []))
</script>

<template>
  <p v-if="mocked.length" class="devnote">
    <Icon name="ph:flask" />
    Макет: {{ what ?? 'раздел' }} из design/fixtures — сервер пока не отдаёт {{ mocked.join(', ') }}
  </p>
</template>

<style scoped>
.devnote { display: inline-flex; align-items: center; gap: 6px; margin: 0; padding: 4px 10px; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); font-size: var(--fs-micro); color: var(--text-3); }
</style>
