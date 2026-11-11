<script setup lang="ts">
// Полоса долей и легенда строками: «записи по состоянию», «место по видам»
export interface MeterPart { key: string, label: string, value: number, color: string, display?: string, sub?: string }
const props = defineProps<{
  parts: MeterPart[]
  label: string
}>()
const lang = useLang()
const visible = computed(() => props.parts.filter(p => p.value > 0))
</script>

<template>
  <div class="mt">
    <div class="mt__bar" role="img" :aria-label="`${label}: ${visible.map(p => `${p.label} ${p.display ?? p.value}`).join(', ')}`">
      <i v-for="p in visible" :key="p.key" :style="{ flexGrow: p.value, background: p.color }" />
    </div>
    <ul class="mt__legend">
      <li v-for="p in parts" :key="p.key" :class="{ 'is-zero': !p.value }">
        <i :style="{ background: p.color }" />
        <span class="mt__name">{{ p.label }}<small v-if="p.sub">{{ p.sub }}</small></span>
        <b class="tnum">{{ p.display ?? formatNumber(p.value, lang) }}</b>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.mt { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
.mt__bar { display: flex; gap: 2px; height: 8px; overflow: hidden; border-radius: 4px; background: var(--fill-2); }
.mt__bar i { display: block; min-width: 3px; height: 100%; }
.mt__legend { display: flex; flex-direction: column; margin: 0; padding: 0; list-style: none; font-size: var(--fs-small); }
.mt__legend li { display: flex; align-items: center; gap: 10px; min-height: 40px; border-top: 1px solid var(--hair); }
.mt__legend li:first-child { border-top: 0; }
.mt__legend li.is-zero { color: var(--text-3); }
.mt__legend i { width: 8px; height: 8px; flex: none; border-radius: 2px; }
.mt__name { display: flex; flex: 1; align-items: baseline; gap: 8px; min-width: 0; overflow: hidden; color: var(--text-2); white-space: nowrap; }
.mt__name small { overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); text-overflow: ellipsis; }
.mt__legend b { font-weight: 600; }
</style>
