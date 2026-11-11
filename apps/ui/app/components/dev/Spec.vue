<script setup lang="ts">
// Образец в галерее компонентов: подпись и сам компонент
withDefaults(defineProps<{ label: string, wide?: boolean, col?: boolean, pad?: boolean }>(), { pad: true })
</script>

<template>
  <figure class="spec" :class="{ 'spec--wide': wide }">
    <div class="spec__box" :class="{ 'spec__box--col': col, 'spec__box--pad': pad }"><slot /></div>
    <figcaption class="spec__label">{{ label }}</figcaption>
  </figure>
</template>

<style scoped>
.spec { display: flex; flex-direction: column; gap: 8px; min-width: 0; margin: 0; }
.spec--wide { grid-column: 1 / -1; }
.spec__box { position: relative; display: flex; flex-wrap: wrap; align-items: center; gap: 12px; min-width: 0; min-height: 64px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.spec__box--pad { padding: 16px; }
.spec__box--col { flex-direction: column; flex-wrap: nowrap; align-items: stretch; }
.spec__box--col > :deep(*) { min-width: 0; }
.spec__label { font-size: var(--fs-caption); color: var(--text-3); }
</style>
