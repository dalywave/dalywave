<script setup lang="ts">
import type { ClipFollow } from '~/utils/clip-follow'

// Зацикленное видео без звука поверх обложки: проявляется, когда пошли кадры, и стоит, пока его
// не видно — вкладка в фоне, элемент за экраном или `active` выключен. Звука у файлов нет вовсе,
// `muted` — ещё и для автозапуска в браузерах. С `follow` (клип целиком, сопоставленный с треком)
// видео не крутится по кругу, а идёт за позицией трека: перемотка, пауза, растяжение клипа
const props = withDefaults(defineProps<{
  src: string | null
  active?: boolean
  /** Номер круга: сменился — видео с начала (карточки ленты: в такт с отрывком звука) */
  cycle?: number
  /** Как кадр ложится в рамку: обрезается по ней (cover) или целиком (contain) */
  fit?: 'cover' | 'contain'
  /** Клип целиком в такт с треком: видео ведётся по его позиции */
  follow?: ClipFollow | null
  /** Не проявлять, даже когда кадры пошли: снаружи ещё смотрят, что в кадре (плеер) */
  hold?: boolean
  /** Кадры читаются снаружи (поиск вшитых полос): видео запрашивается с CORS, как и звук */
  probe?: boolean
}>(), { active: true, cycle: 0, fit: 'cover', follow: null, hold: false, probe: false })
const emit = defineEmits<{ ready: [boolean] }>()

const bridge = useBridge()
const el = ref<HTMLVideoElement | null>(null)
const shown = ref(false)
const onScreen = ref(true)
const visible = useDocumentVisibility()
const url = computed(() => (props.src ? bridge.resolveUrl(props.src) : null))
const run = computed(() => !!url.value && props.active && onScreen.value && visible.value === 'visible')

useIntersectionObserver(el, ([entry]) => {
  onScreen.value = !!entry?.isIntersecting
}, { threshold: 0.01 })

function sync() {
  const v = el.value
  if (!v) return
  if (run.value) {
    v.play().catch(() => {
      // Автозапуск запрещён (энергосбережение iOS): остаётся обложка
      shown.value = false
      emit('ready', false)
    })
  }
  else {
    v.pause()
  }
}

watch(run, sync)

// ---------- В такт с треком ----------
// Видео отстаёт или убежало дальше допуска — перескакивает; скорость — по растяжению клипа.
// Трек там, где в клипе кадров нет (вступление трека до начала клипа, хвост) — видео прячется,
// под ним обложка
const outside = ref(false)
function track() {
  const v = el.value
  const f = props.follow
  if (!v || !f) return
  const at = followTarget(f, v.duration)
  outside.value = at === null
  if (at === null) {
    if (!v.paused) v.pause()
    return
  }
  const rate = 1 / f.rate
  if (Math.abs(v.playbackRate - rate) > 0.001) v.playbackRate = rate
  if (Math.abs(v.currentTime - at) > FOLLOW_TOLERANCE_S) v.currentTime = at
  if (run.value && f.playing() && v.paused) void v.play().catch(() => {})
  if (!f.playing() && !v.paused) v.pause()
}
const follower = useIntervalFn(track, 250, { immediate: false })
watch([() => !!props.follow, run], ([on, going]) => {
  if (on && going) {
    follower.resume()
    track()
  }
  else {
    follower.pause()
  }
}, { immediate: true })
watch(() => props.cycle, () => {
  if (el.value) el.value.currentTime = 0
})
watch(url, () => {
  shown.value = false
  emit('ready', false)
})

function onPlaying() {
  shown.value = true
  emit('ready', true)
}
// Кадр нужен снаружи: подсветка вокруг клипа в плеере берёт из него цвета
defineExpose({ el })

function onError() {
  shown.value = false
  emit('ready', false)
}
</script>

<template>
  <video
    v-if="url"
    ref="el"
    :key="url"
    class="lv"
    :class="{ 'is-shown': shown && !outside && !hold }"
    :style="fit === 'contain' ? { objectFit: 'contain' } : undefined"
    :src="url"
    :crossorigin="probe ? 'anonymous' : undefined"
    muted
    :loop="!follow"
    playsinline
    disablepictureinpicture
    disableremoteplayback
    preload="auto"
    aria-hidden="true"
    tabindex="-1"
    @loadeddata="sync"
    @playing="onPlaying"
    @error="onError"
  />
</template>

<style scoped>
.lv {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: inherit;
  opacity: 0;
  pointer-events: none;
  transition: opacity 700ms var(--ease-out);
}
.lv.is-shown { opacity: 1; }
@media (prefers-reduced-motion: reduce) {
  .lv { transition: none; }
}
</style>
