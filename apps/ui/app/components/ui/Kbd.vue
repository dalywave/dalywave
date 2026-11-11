<script setup lang="ts">
defineProps<{ keys: string[] }>()
</script>

<template>
  <span class="kbd" :aria-label="keys.join(' + ')">
    <kbd v-for="k in keys" :key="k">{{ k }}</kbd>
  </span>
</template>

<style scoped>
.kbd { display: inline-flex; gap: 3px; align-items: center; }
/* Под пальцем и на телефоне клавиатурные подсказки не нужны */
@media (pointer: coarse), (max-width: 699px) {
  .kbd { display: none; }
}
kbd {
  min-width: 20px;
  height: 20px;
  padding: 0 5px;
  display: inline-grid;
  place-items: center;
  border-radius: 5px;
  font-family: var(--font);
  font-size: var(--fs-micro);
  font-weight: 600;
  color: var(--text-2);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line), inset 0 -1px 0 var(--line);
}
</style>
