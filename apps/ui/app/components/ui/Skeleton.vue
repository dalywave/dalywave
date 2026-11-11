<script setup lang="ts">
withDefaults(defineProps<{
  variant?: 'block' | 'line' | 'round'
  width?: string
  height?: string
}>(), { variant: 'block' })
</script>

<template>
  <span class="skel" :class="`skel--${variant}`" :style="{ width, height }" aria-hidden="true" />
</template>

<style scoped>
.skel { position: relative; display: block; overflow: hidden; background: var(--fill-1); border-radius: var(--r-2); }
.skel::after {
  content: '';
  position: absolute;
  inset: 0;
  transform: translateX(-100%);
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.05), transparent);
  animation: shimmer 1.6s var(--ease-in-out) infinite;
}
.skel--line { height: 10px; border-radius: 5px; }
.skel--round { border-radius: 50%; aspect-ratio: 1; }
.skel--block { aspect-ratio: auto; }
</style>
