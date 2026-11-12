<script setup lang="ts">
import type { PersonRef } from '~/api/types'

// Аватар человека из daly.rest; без картинки — первая буква имени на нейтральной подложке.
// `live` — слушает прямо сейчас: маленькая точка-эквалайзер в углу
const props = withDefaults(defineProps<{
  person: Pick<PersonRef, 'name' | 'avatarUrl'> | null | undefined
  size?: number
  live?: boolean
}>(), { size: 40, live: false })

const failed = ref(false)
watch(() => props.person?.avatarUrl, () => (failed.value = false))
const initial = computed(() => (props.person?.name ?? '?').trim().charAt(0).toUpperCase() || '?')
const style = computed(() => ({ '--av': `${props.size}px` }))
</script>

<template>
  <span class="av" :style="style" aria-hidden="true">
    <img v-if="person?.avatarUrl && !failed" :src="person.avatarUrl" alt="" class="av__img" loading="lazy" @error="failed = true">
    <span v-else class="av__initial">{{ initial }}</span>
    <span v-if="live" class="av__live"><i /><i /><i /></span>
  </span>
</template>

<style scoped>
.av { position: relative; display: inline-grid; place-items: center; width: var(--av); height: var(--av); flex: none; border-radius: 50%; background: var(--fill-3); box-shadow: inset 0 0 0 1px var(--hair); }
.av__img { width: 100%; height: 100%; border-radius: 50%; object-fit: cover; }
.av__initial { font-size: calc(var(--av) * 0.42); font-weight: 700; color: var(--text-1); line-height: 1; }
/* Слушает сейчас: эквалайзер в кружке на краю аватара */
.av__live { position: absolute; right: -2px; bottom: -2px; display: inline-flex; align-items: flex-end; justify-content: center; gap: 1.5px; width: max(16px, calc(var(--av) * 0.38)); height: max(16px, calc(var(--av) * 0.38)); padding: 4px 3px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 2px var(--ink-1); }
.av__live i { flex: 1; height: 100%; border-radius: 1px; background: var(--on-light); transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.av__live i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.av__live i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
@media (prefers-reduced-motion: reduce) {
  .av__live i { animation: none; transform: scaleY(0.6); }
}
</style>
