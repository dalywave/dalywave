<script setup lang="ts">
import type { ImageSet, PlaylistKind } from '~/api/types'
import { seededRandom } from '~/utils/peaks'

// Обложка плейлиста: своя картинка, мозаика из четырёх обложек или одна обложка, если альбомов
// меньше. Пустой плейлист — спокойная заглушка с нотой. Собранная обложка в цветах настроения
// с линией волны — только у миксов: это знак того, что их собирает сервис
const props = withDefaults(defineProps<{
  id: string
  title: string
  covers?: ImageSet[]
  /** Своя обложка, загруженная человеком */
  image?: ImageSet | null
  kind?: PlaylistKind
  /** Принудительно собранная обложка (миксы) */
  generated?: boolean
  showTitle?: boolean
}>(), { covers: () => [], kind: 'user', showTitle: true })

const PALETTES: [string, string, string][] = [
  ['#0a2a52', '#1d8cbc', '#aef0ff'],
  ['#1c1946', '#5b50c4', '#d3ccff'],
  ['#5c0f5e', '#ff4fa8', '#ffe9a8'],
  ['#7a2208', '#ff6a1a', '#ffd27a'],
  ['#1d1048', '#b01cf0', '#ff9a3c'],
]

const bridge = useBridge()
const gen = computed(() => !props.image && (props.generated || (props.kind === 'mix' && !props.covers.length)))
const useMosaic = computed(() => !props.image && !gen.value && props.covers.length >= 4)
const single = computed(() => props.image ?? (!gen.value && props.covers.length > 0 && props.covers.length < 4 ? props.covers[0]! : null))
const blank = computed(() => !gen.value && !useMosaic.value && !single.value)
const look = computed(() => {
  const rnd = seededRandom(props.id)
  const pal = PALETTES[Math.floor(rnd() * PALETTES.length)]!
  const phase = rnd() * 6
  const amp = 10 + rnd() * 10
  let d = 'M-10 50'
  for (let x = 0; x <= 110; x += 5) d += ` L${x} ${(50 + Math.sin(x / 14 + phase) * amp * Math.sin(x / 40 + 0.4)).toFixed(1)}`
  return { pal, d }
})
const kicker = computed(() => (props.kind === 'mix' ? 'Микс' : props.kind === 'smart' ? 'Подборка' : 'Плейлист'))
</script>

<template>
  <div class="pc" :class="{ 'pc--gen': gen }">
    <div v-if="useMosaic" class="pc__mosaic">
      <img v-for="(c, i) in covers.slice(0, 4)" :key="i" :src="bridge.resolveUrl(c.small)" alt="" loading="lazy" decoding="async">
    </div>
    <img v-else-if="single" :src="bridge.resolveUrl(single.medium)" alt="" class="pc__single" loading="lazy" decoding="async">
    <div v-else-if="blank" class="pc__blank" aria-hidden="true"><Icon name="ph:music-notes-simple" /></div>
    <div v-else class="pc__gen" :style="{ '--g1': look.pal[0], '--g2': look.pal[1], '--g3': look.pal[2] }">
      <svg class="pc__wave" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        <path :d="look.d" fill="none" stroke="rgba(255,255,255,0.75)" stroke-width="1.6" vector-effect="non-scaling-stroke" />
        <path :d="look.d" fill="none" stroke="var(--g3)" stroke-width="8" opacity="0.25" vector-effect="non-scaling-stroke" />
      </svg>
      <span class="pc__kicker"><BrandGlyph class="pc__glyph" />{{ kicker }}</span>
      <span v-if="showTitle" class="pc__title">{{ title }}</span>
    </div>
  </div>
</template>

<style scoped>
.pc { position: relative; aspect-ratio: 1; overflow: hidden; border-radius: var(--r-3); background: var(--ink-3); container-type: inline-size; }
.pc::after { content: ''; position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 0 1px rgba(var(--hi-rgb), 0.06); pointer-events: none; }
.pc__mosaic { display: grid; grid-template-columns: 1fr 1fr; width: 100%; height: 100%; }
.pc__mosaic img, .pc__single { width: 100%; height: 100%; object-fit: cover; }
.pc__blank { display: grid; place-items: center; width: 100%; height: 100%; background: var(--fill-2); color: var(--text-3); font-size: clamp(14px, 26cqw, 56px); }
.pc__gen {
  position: relative;
  width: 100%;
  height: 100%;
  background: radial-gradient(80% 60% at 20% 100%, color-mix(in oklab, var(--g2) 80%, transparent), transparent 70%), radial-gradient(70% 60% at 100% 0%, color-mix(in oklab, var(--g3) 45%, transparent), transparent 65%), var(--g1);
}
.pc__wave { position: absolute; left: 0; top: 30%; width: 100%; height: 40%; overflow: visible; }
.pc__kicker { position: absolute; left: 7%; top: 7%; display: inline-flex; align-items: center; gap: 4px; font-size: clamp(8px, 4.2cqw, 11px); font-weight: 650; letter-spacing: 0.12em; text-transform: uppercase; color: rgba(255, 255, 255, 0.82); }
.pc__glyph { width: 1.4em; height: 1.4em; }
/* Длинное название — не больше трёх строк и всегда по левому краю, где бы обложка ни стояла */
.pc__title { position: absolute; left: 7%; right: 7%; bottom: 7%; display: -webkit-box; overflow: hidden; font-size: clamp(10px, 10cqw, 28px); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.05; text-align: left; text-wrap: balance; color: #fff; -webkit-box-orient: vertical; -webkit-line-clamp: 3; line-clamp: 3; }
</style>
