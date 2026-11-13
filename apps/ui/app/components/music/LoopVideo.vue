<script setup lang="ts">
// Зацикленное видео без звука поверх обложки: проявляется, когда пошли кадры, и стоит, пока его
// не видно — вкладка в фоне, элемент за экраном или `active` выключен. Звука у файлов нет вовсе,
// `muted` — ещё и для автозапуска в браузерах
const props = withDefaults(defineProps<{
  src: string | null
  active?: boolean
  /** Номер круга: сменился — видео с начала (карточки ленты: в такт с отрывком звука) */
  cycle?: number
  /** Как кадр ложится в рамку: обрезается по ней (cover) или целиком (contain) */
  fit?: 'cover' | 'contain'
}>(), { active: true, cycle: 0, fit: 'cover' })
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
    :class="{ 'is-shown': shown }"
    :style="fit === 'contain' ? { objectFit: 'contain' } : undefined"
    :src="url"
    muted
    loop
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
