<script setup lang="ts">
import { encode } from 'uqr'

// QR-код ссылки «Слушать вместе»: тёмные модули на светлом поле — так его читает любая камера
const props = defineProps<{ value: string, label: string }>()

const qr = computed(() => encode(props.value, { ecc: 'M', border: 0 }))
// Одна фигура из квадратиков модулей: чётко при любом масштабе
const path = computed(() => {
  const { data } = qr.value
  let d = ''
  data.forEach((row, y) => row.forEach((on, x) => {
    if (on) d += `M${x} ${y}h1v1h-1z`
  }))
  return d
})
</script>

<template>
  <div class="qr" role="img" :aria-label="label">
    <svg :viewBox="`0 0 ${qr.size} ${qr.size}`" shape-rendering="crispEdges" aria-hidden="true">
      <path :d="path" fill="currentColor" />
    </svg>
  </div>
</template>

<style scoped>
.qr { display: grid; place-items: center; aspect-ratio: 1; padding: 8%; border-radius: 20px; background: #fff; color: #0b0c10; }
.qr svg { display: block; width: 100%; height: 100%; }
</style>
