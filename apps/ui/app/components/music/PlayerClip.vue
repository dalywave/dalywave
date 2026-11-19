<script setup lang="ts">
import type { ImageSet } from '~/api/types'
import type { PlayerClip } from '~/composables/usePlayerClip'
import type { Box, ProbeVerdict } from '~/utils/clip-frame'
import { clipGeometry, FULL_CROP, probeVideo, STILL_AFTER, STILL_AFTER_FULL } from '~/utils/clip-frame'

// Клип в полном плеере (см. usePlayerClip). Два места: `art` — внутри блока обложки (квадрат
// и «Широкий»), `screen` — слоем на весь плеер под интерфейсом («Фон» и «Кино»). Рисуется только
// то место, которое нужно выбранному виду.
//
// Пока кадры не пошли, видна обычная чёткая обложка; клип проявляется поверх, когда первые кадры
// разобраны (utils/clip-frame): вшитые в ролик чёрные полосы не показываются — видео лежит
// в «окне» по полезной части кадра. Квадрат и «Фон» заполняются полезной частью, «Широкий» берёт
// её пропорции, «Кино» показывает её целиком, а вокруг — размытые цвета того же кадра.
//
// Подсветка (как ambient mode у YouTube): крошечный холст, в который 5 раз в секунду вписывается
// кадр с наложением на прошлый — цвета меняются плавно; растягивает и размывает его уже CSS.
// Второе видео не декодируется, кадр 32×18 — микросекунды
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
const frameStyle = computed(() => (here.value === 'wide' && props.clip.visibleRatio < 1 ? { width: `${props.clip.visibleRatio * 100}%` } : undefined))
const contain = computed(() => here.value === 'cinema' && !props.touch)

const loop = ref<{ el: HTMLVideoElement | null } | null>(null)
const coverEl = ref<HTMLImageElement | null>(null)
const fill = ref<HTMLCanvasElement | null>(null)

// ---------- Где видео: окно по полезной части кадра в рамке ----------
const stage = ref<HTMLElement | null>(null)
const { width: boxW, height: boxH } = useElementSize(stage)
const size = shallowRef<{ w: number, h: number } | null>(null)
const px = (b: Box) => ({ left: `${b.left}px`, top: `${b.top}px`, width: `${b.width}px`, height: `${b.height}px` })
const geo = computed(() => {
  const s = size.value
  if (!s || !boxW.value || !boxH.value) return null
  const g = clipGeometry({ w: boxW.value, h: boxH.value }, s, props.clip.crop, contain.value ? 'contain' : 'cover')
  return { win: px(g.win), video: { ...px(g.video), maxWidth: 'none', maxHeight: 'none', objectFit: 'fill' as const } }
})

function onMeta(e: Event) {
  const v = e.target as HTMLVideoElement
  if (!v.videoWidth || !v.videoHeight) return
  size.value = { w: v.videoWidth, h: v.videoHeight }
  props.clip.setRatio(v.videoWidth / v.videoHeight)
}

// ---------- Первые кадры: полосы и «клип-картинка» — до того, как клип проявится ----------
let stopProbe: (() => void) | null = null
function endProbe() {
  stopProbe?.()
  stopProbe = null
}
function onVerdict(r: ProbeVerdict & { state: 'show' | 'still' }) {
  const src = props.clip.src
  if (r.state === 'still' && props.clip.isClip && src) {
    props.clip.markStill(src)
    return
  }
  props.clip.setCrop(r.state === 'show' ? r.crop : FULL_CROP)
  props.clip.setShown(true)
}
function onReady(ok: boolean) {
  endProbe()
  const v = loop.value?.el
  if (!ok || !v || !props.clip.src) {
    props.clip.setShown(false)
    return
  }
  // Клип целиком идёт за треком: заставка в начале — не повод счесть его картинкой
  stopProbe = probeVideo(v, props.clip.src, onVerdict, props.clip.follow ? STILL_AFTER_FULL : STILL_AFTER)
}
watch(() => props.clip.src, endProbe)
onBeforeUnmount(endProbe)

// ---------- Подсветка: цвета кадра (или обложки, пока кадров нет) ----------
const lit = computed(() => here.value === 'wide' || contain.value)
let fresh = true
function paint() {
  const c = fill.value
  const ctx = c?.getContext('2d')
  if (!c || !ctx) return
  const v = loop.value?.el
  const img = coverEl.value
  ctx.globalAlpha = fresh ? 1 : 0.3
  try {
    if (props.clip.shown && v && v.readyState >= 2 && v.videoWidth) {
      // Только полезная часть: чёрные полосы не гасят подсветку
      const k = props.clip.crop
      ctx.drawImage(v, k.x * v.videoWidth, k.y * v.videoHeight, k.w * v.videoWidth, k.h * v.videoHeight, 0, 0, c.width, c.height)
    }
    else if (img?.complete && img.naturalWidth) {
      ctx.drawImage(img, 0, 0, c.width, c.height)
    }
    else {
      return
    }
    // Наложение на прошлый кадр: смена сцены в клипе перетекает за полсекунды, а не мигает
    fresh = false
  }
  catch { /* кадр ещё не готов */ }
}
const ticker = useIntervalFn(paint, 200, { immediate: false })
watch([lit, run, visible, () => props.clip.shown], () => {
  // Кадр стоит (пауза, вкладка в фоне) — холст тоже: одна отрисовка, без таймера
  void nextTick(paint)
  if (lit.value && run.value && visible.value === 'visible' && props.clip.shown) ticker.resume()
  else ticker.pause()
}, { immediate: true })
watch([() => props.clip.src, cover], () => {
  fresh = true
  void nextTick(paint)
})
</script>

<template>
  <!-- Квадрат: как живая обложка (MusicMotionCover) — поверх картинки, в той же рамке (.mc) -->
  <div v-if="here === 'square' && clip.src" ref="stage" class="mc pc-square">
    <div class="pc-win" :style="geo?.win">
      <MusicLoopVideo ref="loop" :src="clip.src" :active="run" :follow="clip.follow" :hold="!clip.shown" probe :style="geo?.video" @ready="onReady" @loadedmetadata="onMeta" />
    </div>
  </div>

  <div v-else-if="here === 'wide'" class="pcw" :class="{ 'is-shown': clip.shown }">
    <div class="pcw__frame" :style="frameStyle">
      <canvas ref="fill" class="pc-glow" width="32" height="18" aria-hidden="true" />
      <div ref="stage" class="pcw__card">
        <div class="pc-win" :style="geo?.win">
          <MusicLoopVideo ref="loop" :src="clip.src" :active="run" :follow="clip.follow" :hold="!clip.shown" probe :style="geo?.video" @ready="onReady" @loadedmetadata="onMeta" />
        </div>
      </div>
    </div>
  </div>

  <div v-else-if="here" class="pcs" :class="{ 'pcs--contain': contain, 'pcs--touch': touch, 'is-shown': clip.shown }" aria-hidden="true">
    <canvas ref="fill" class="pcs__fill" width="32" height="18" />
    <!-- Пока клип не пошёл — обычная чёткая обложка -->
    <div v-if="cover" class="pcs__art">
      <img ref="coverEl" :src="cover" alt="" @load="paint">
    </div>
    <div ref="stage" class="pcs__stage">
      <div class="pc-win" :style="geo?.win">
        <MusicLoopVideo ref="loop" :src="clip.src" :active="run" :follow="clip.follow" :hold="!clip.shown" probe :fit="contain ? 'contain' : 'cover'" :style="geo?.video" @ready="onReady" @loadedmetadata="onMeta" />
      </div>
    </div>
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
/* Окно по полезной части кадра: полосы, вшитые в ролик, остаются за его краем */
.pc-win { position: absolute; inset: 0; overflow: hidden; border-radius: inherit; }

/* ---------- «Широкий»: клип целиком, вокруг — его цвета ---------- */
.pcw {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  justify-content: center;
  border-radius: inherit;
  pointer-events: none;
  /* Пока кадры не пошли — под ним обычная обложка; проявляется плавно, уходит сразу (новый трек) */
  opacity: 0;
}
.pcw.is-shown { opacity: 1; transition: opacity 600ms var(--ease-out); }
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
.pcs__stage { position: absolute; inset: 0; }
/* Цвета кадра, размытые и угасающие к краям: поля «Кино» и фон под обложкой, пока клипа нет */
.pcs__fill {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0.3;
  filter: blur(40px) saturate(1.4) brightness(0.85);
  -webkit-mask-image: radial-gradient(farthest-corner, #000 45%, transparent 95%);
  mask-image: radial-gradient(farthest-corner, #000 45%, transparent 95%);
  transform: translateZ(0);
}
.pcs.is-shown .pcs__fill { transition: opacity 700ms var(--ease-out); }
.pcs--contain.is-shown .pcs__fill { opacity: 1; }
.pcs:not(.pcs--contain).is-shown .pcs__fill { opacity: 0; }

/* Обложка, пока клипа нет: по центру свободного места над названием */
.pcs__art {
  position: absolute;
  inset: var(--pcs-art-inset, 76px 48px 248px);
  display: grid;
  place-items: center;
  container-type: size;
}
.pcs--touch .pcs__art { inset: var(--pcs-art-inset, calc(var(--safe-top, 0px) + 76px) 24px calc(var(--safe-bottom, 0px) + 380px)); }
.pcs__art img {
  width: min(100cqw, 100cqh, 640px);
  aspect-ratio: 1;
  object-fit: cover;
  border-radius: var(--r-4);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06), 0 24px 60px -28px rgba(0, 0, 0, 0.9);
}
/* Клип проявляется поверх — обложка уходит; новый трек — она на месте сразу, без провала в чёрное */
.pcs.is-shown .pcs__art { opacity: 0; transition: opacity 700ms var(--ease-out); }

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
/* Под обложкой затемнять нечего: текст и так на тёмном */
.pcs:not(.is-shown) .pcs__shade { opacity: 0; }

@media (prefers-reduced-motion: reduce) {
  .pcw, .pcs__fill, .pcs__art, .pcs__shade { transition: none !important; }
}
</style>
