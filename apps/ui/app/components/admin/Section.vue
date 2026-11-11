<script setup lang="ts">
// Раздел страницы админки: заголовок, короткая подсказка, инструменты справа (фильтры, кнопки)
defineProps<{
  title: string
  hint?: string
}>()
</script>

<template>
  <section class="ap-sec" :aria-label="title">
    <header class="ap-sec__head">
      <div class="ap-sec__titles">
        <h2 class="ap-h2">{{ title }}<slot name="badge" /></h2>
        <p v-if="hint" class="ap-hint">{{ hint }}</p>
      </div>
      <div v-if="$slots.tools" class="ap-tools"><slot name="tools" /></div>
    </header>
    <slot />
  </section>
</template>
