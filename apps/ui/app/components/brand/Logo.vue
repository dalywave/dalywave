<script setup lang="ts">
// Знак и слово «DalyWave» набором интерфейса. compact — только знак (узкая панель)
withDefaults(defineProps<{ compact?: boolean, to?: string }>(), { to: '/' })
</script>

<template>
  <NuxtLink :to="to" class="logo" :aria-label="compact ? 'DalyWave' : undefined">
    <BrandGlyph class="logo__mark" />
    <span v-if="!compact" class="logo__word">DalyWave</span>
  </NuxtLink>
</template>

<style scoped>
.logo { display: inline-flex; align-items: center; gap: 10px; color: var(--text-1); border-radius: var(--r-2); }
.logo__mark { width: 28px; height: 28px; flex: none; }
.logo__word { font-size: 17px; font-weight: 700; letter-spacing: -0.03em; }
</style>
