<script setup lang="ts">
import type { ImageSet } from '~/api/types'
import type { PlayerClip } from '~/composables/usePlayerClip'

// Клип в полном плеере (см. usePlayerClip). Два места: `art` — внутри блока обложки (квадрат
// и «Широкий»), `screen` — слоем на весь плеер под интерфейсом («Фон» и «Кино»). Рисуется только
// то место, которое нужно выбранному виду. Пока кадры не пошли, видна обложка: в «Широком» — по
// рамке клипа, на весь плеер — размытой; клип проявляется поверх.
//
// Подсветка «Широкого» (как ambient mode у YouTube): крошечный холст, в который 5 раз в секунду
// вписывается кадр с наложением на прошлый — цвета меняются плавно; растягивает и размывает его
// уже CSS. Второе видео не декодируется, кадр 24×14 — микросекунды. Поля «Кино» — чёрные: во
// многих клипах свои чёрные полосы, цветные поля спорили бы с ними
const props = withDefaults(defineProps<{
  clip: PlayerClip
  place: 'art' | 'screen'
  /** Обложка альбома — пока клип не пошёл */
  image?: ImageSet | null
  active?: boolean
  /** Телефон и iPad: «Кино» — тот же фон во весь экран, видео обрезается по нему */
  touch?: boolean
}>(), { image: null, active: true, touch: false })

const bridge = useBridge()
const visible = useDocumentVisibility()

const here = computed(() => {
  const l = props.clip.layout
  if (!l) return null
  if (props.place === 'art') return l === 'square' || l === 'wide' ? l : null
  return l === 'backdrop' || l === 'cinema' ? l : null
})
const run = computed(() => props.active && props.clip.playing)
const cover = computed(() => (props.image?.large ? bridge.resolveUrl(props.image.large) : null))
/** Вертикальный клип в «Широком» — по высоте квадрата, по бокам подсветка */
const frameStyle = computed(() => (here.value === 'wide' && props.clip.ratio < 1 ? { width: `${props.clip.ratio * 100}%` } : undefined))
const contain = computed(() => here.value === 'cinema' && !props.touch)

const loop = ref<{ el: HTMLVideoElement | null } | null>(null)
const coverEl = ref<HTMLImageElement | null>(null)
const glow = ref<HTMLCanvasElement | null>(null)
const glowOn = computed(() => here.value === 'wide')

function onMeta(e: Event) {
  const v = e.target as HTMLVideoElement
  if (v.videoWidth && v.videoHeight) props.clip.setRatio(v.videoWidth / v.videoHeight)
}

let fresh = true
function paint() {
  const c = glow.value
  const ctx = c?.getContext('2d')
  if (!c || !ctx) return
  const v = loop.value?.el
  const img = coverEl.value
  const from = props.clip.shown && v && v.readyState >= 2 ? v : img?.complete && img.naturalWidth ? img : null
  if (!from) return
  // Наложение на прошлый кадр: смена сцены в клипе перетекает за полсекунды, а не мигает
  ctx.globalAlpha = fresh ? 1 : 0.3
  try {
    ctx.drawImage(from, 0, 0, c.width, c.height)
    fresh = false
  }
  catch { /* кадр ещё не готов */ }
}
const ticker = useIntervalFn(paint, 200, { immediate: false })
watch([glowOn, run, visible, () => props.clip.shown], () => {
  // Кадр стоит (пауза, вкладка в фоне) — холст тоже: одна отрисовка, без таймера
  void nextTick(paint)
  if (glowOn.value && run.value && visible.value === 'visible' && props.clip.shown) ticker.resume()
  else ticker.pause()
}, { immediate: true })
watch(() => props.clip.src, () => {
  fresh = true
  void nextTick(paint)
})
</script>

<template>
  <!-- Квадрат: как живая обложка (MusicMotionCover) — поверх картинки, в той же рамке (.mc) -->
  <div v-if="here === 'square' && clip.src" class="mc pc-square">
    <MusicLoopVideo :src="clip.src" :active="run" @ready="clip.setShown" />
  </div>

  <div v-else-if="here === 'wide'" class="pcw">
    <div class="pcw__frame" :style="frameStyle">
      <canvas ref="glow" class="pc-glow" width="24" height="14" aria-hidden="true" />
      <div class="pcw__card">
        <img v-if="cover" ref="coverEl" :src="cover" alt="" class="pcw__cover" aria-hidden="true" @load="paint">
        <MusicLoopVideo ref="loop" :src="clip.src" :active="run" @ready="clip.setShown" @loadedmetadata="onMeta" />
      </div>
    </div>
  </div>

  <div v-else-if="here" class="pcs" :class="{ 'pcs--contain': contain, 'is-shown': clip.shown }" aria-hidden="true">
    <img v-if="cover" ref="coverEl" :src="cover" alt="" class="pcs__cover" @load="paint">
    <MusicLoopVideo ref="loop" :src="clip.src" :active="run" :fit="contain ? 'contain' : 'cover'" @ready="clip.setShown" @loadedmetadata="onMeta" />
    <i class="pcs__shade" />
  </div>
</template>

<style scoped>
.pc-square {
  position: absolute;
  inset: 0;
  z-index: 1;
  overflow: hidden;
  border-radius: inherit;
  pointer-events: none;
}

/* ---------- «Широкий»: клип целиком, вокруг — его цвета ---------- */
.pcw {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  justify-content: center;
  border-radius: inherit;
  pointer-events: none;
}
.pcw__frame { position: relative; width: 100%; height: 100%; border-radius: inherit; transition: width var(--dur-4) var(--ease-out); }
.pcw__card {
  position: absolute;
  inset: 0;
  overflow: hidden;
  border-radius: inherit;
  background: var(--ink-3);
  /* Без тёмной тени: вокруг светится сам клип */
  box-shadow: 0 0 0 1px rgba(var(--hi-rgb), 0.06), 0 18px 40px -24px rgba(0, 0, 0, 0.6);
}
.pcw__cover { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.pc-glow {
  position: absolute;
  inset: -26% -16%;
  z-index: -1;
  width: 132%;
  height: 152%;
  border-radius: 50%;
  opacity: 0.9;
  /* Тёмные клипы тоже дают цвет: ярче и насыщеннее самого кадра */
  filter: blur(48px) saturate(1.6) brightness(1.3);
  transform: translateZ(0);
}
/* На бумаге тёмный кадр дал бы грязную тень — только светлый оттенок его цветов */
:root[data-theme='light'] .pc-glow { opacity: 0.5; filter: blur(48px) saturate(1.8) brightness(1.9); }

/* ---------- «Фон» и «Кино»: на весь плеер ---------- */
.pcs { position: absolute; inset: 0; z-index: 0; overflow: hidden; background: #000; pointer-events: none; }
/* Пока клип не пошёл — обложка размытым фоном, без мыльных краёв */
.pcs__cover {
  position: absolute;
  inset: -40px;
  width: calc(100% + 80px);
  height: calc(100% + 80px);
  object-fit: cover;
  opacity: 0.7;
  filter: blur(36px) saturate(1.1);
  transition: opacity var(--dur-4) var(--ease-out);
}
.pcs.is-shown .pcs__cover { opacity: 0; }
/* «Кино»: пока клип не пошёл, обложка тише — поля останутся чёрными */
.pcs--contain:not(.is-shown) .pcs__cover { opacity: 0.45; }
/* Затемнение сверху (шапка) и снизу (название, кнопки), лёгкий вигнет — 480p не выглядит мылом */
.pcs__shade {
  position: absolute;
  inset: 0;
  background:
    linear-gradient(180deg, rgba(0, 0, 0, 0.55) 0, rgba(0, 0, 0, 0.28) 9%, rgba(0, 0, 0, 0) 22%),
    linear-gradient(0deg, rgba(0, 0, 0, 0.86) 0, rgba(0, 0, 0, 0.72) 22%, rgba(0, 0, 0, 0.32) 44%, rgba(0, 0, 0, 0) 62%),
    radial-gradient(120% 90% at 50% 45%, transparent 55%, rgba(0, 0, 0, 0.35) 100%);
  transition: opacity 600ms var(--ease-out);
}
</style>
