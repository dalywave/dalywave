<script setup lang="ts">
import { renderWaveStill } from '~/lib/rewind/wave'
import { waveColors } from '~/utils/rewind-colors'

// Фон карточки «Твои итоги готовы» в стиле «Волна года»: один неподвижный кадр настоящей волны
// в цветах года. Кадр, а не живая волна: на главной уже играет своя, вторая крутилась бы впустую
const props = defineProps<{ palette: string[] | null }>()

const host = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)

function paint() {
  const el = host.value
  const out = canvas.value
  if (!el || !out || !el.clientWidth) return
  const dpr = Math.min(window.devicePixelRatio || 1, 2)
  const w = Math.round(el.clientWidth * dpr)
  const h = Math.round(el.clientHeight * dpr)
  const frame = renderWaveStill(w, h, { tilt: -0.22, offset: -0.12, amp: 0.9, freq: 1.5, energy: 0.6, vig: 0.35 }, waveColors(props.palette))
  if (!frame) return
  out.width = w
  out.height = h
  out.getContext('2d')?.drawImage(frame, 0, 0)
}

let ro: ResizeObserver | null = null
let timer = 0
onMounted(() => {
  paint()
  // Перерисовка — после того как размер устоялся, а не на каждый кадр изменения
  ro = new ResizeObserver(() => {
    window.clearTimeout(timer)
    timer = window.setTimeout(paint, 150)
  })
  if (host.value) ro.observe(host.value)
})
watch(() => props.palette, paint)
onBeforeUnmount(() => {
  ro?.disconnect()
  window.clearTimeout(timer)
})
</script>

<template>
  <span ref="host" class="lw" aria-hidden="true"><canvas ref="canvas" class="lw__canvas" /></span>
</template>

<style scoped>
.lw { display: block; background: #030406; }
.lw__canvas { display: block; width: 100%; height: 100%; }
</style>
