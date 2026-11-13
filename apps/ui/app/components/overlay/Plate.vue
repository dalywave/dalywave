<script setup lang="ts">
// Плашка «Сейчас играет» для стрима (страница /overlay для OBS и предпросмотр в настройках).
// Две формы: строка (обложка, «название — исполнитель», тонкая полоска внизу) и карточка (обложка
// крупнее, название и исполнитель в две строки, полоска под ними). Смена трека — мягкая: новая
// строка проявляется с коротким сдвигом; пауза — плашка тихо уходит.
// Плашка живёт поверх чужого видео, поэтому палитра своя, а не тема интерфейса: тёмное стекло и
// белый текст читаются на любой картинке
export interface PlateTrack { id: string, title: string, artist: string, cover: string | null }
export type PlateStyle = 'line' | 'card'
export type PlateSize = 's' | 'm' | 'l'
export type PlateSide = 'left' | 'right'

const props = withDefaults(defineProps<{
  track: PlateTrack | null
  visible: boolean
  playing: boolean
  /** Позиция на момент `at` (мс, время этой страницы) */
  positionMs: number
  durationMs: number
  at: number
  look?: PlateStyle
  size?: PlateSize
  side?: PlateSide
}>(), { look: 'card', size: 'm', side: 'left' })

const progress = ref(0)
let raf = 0

function frame() {
  const elapsed = props.playing ? Date.now() - props.at : 0
  const pos = props.positionMs + Math.max(0, elapsed)
  progress.value = props.durationMs > 0 ? Math.min(1, pos / props.durationMs) : 0
  raf = requestAnimationFrame(frame)
}

onMounted(() => (raf = requestAnimationFrame(frame)))
onBeforeUnmount(() => cancelAnimationFrame(raf))

const scale = computed(() => ({ s: 0.8, m: 1, l: 1.35 })[props.size])
</script>

<template>
  <div class="op" :class="[`op--${look}`, `op--${side}`]" :style="{ '--u': scale }">
    <Transition name="op-show">
      <div v-if="visible && track" class="op__plate">
        <Transition name="op-swap" mode="out-in">
          <div :key="track.id" class="op__row">
            <span class="op__cover">
              <img v-if="track.cover" :src="track.cover" alt="">
              <Icon v-else name="ph:music-note-simple-fill" aria-hidden="true" />
            </span>
            <span v-if="look === 'line'" class="op__line">
              <b>{{ track.title }}</b><span class="op__dash"> — </span><span>{{ track.artist }}</span>
            </span>
            <span v-else class="op__text">
              <b class="op__title">{{ track.title }}</b>
              <span class="op__artist">{{ track.artist }}</span>
            </span>
          </div>
        </Transition>
        <span class="op__bar" aria-hidden="true"><i :style="{ transform: `scaleX(${progress})` }" /></span>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.op {
  /* Своя палитра плашки: поверх игры или камеры, вне темы интерфейса */
  --op-glass: rgb(14 14 20 / 0.74);
  --op-edge: rgb(255 255 255 / 0.08);
  --op-text: #fff;
  --op-dim: rgb(255 255 255 / 0.64);
  --op-track: rgb(255 255 255 / 0.16);
  --op-fill: rgb(255 255 255 / 0.92);
  display: flex;
  font-family: var(--font-sans, 'Onest Variable', system-ui, sans-serif);
  color: var(--op-text);
}
.op--right { justify-content: flex-end; }

.op__plate {
  position: relative;
  max-width: 100%;
  overflow: hidden;
  border: 1px solid var(--op-edge);
  background: var(--op-glass);
  -webkit-backdrop-filter: blur(20px) saturate(1.4);
  backdrop-filter: blur(20px) saturate(1.4);
}
.op__row { display: flex; align-items: center; min-width: 0; }
.op__cover { display: grid; flex: none; place-items: center; overflow: hidden; background: rgb(255 255 255 / 0.08); color: var(--op-dim); }
.op__cover img { width: 100%; height: 100%; object-fit: cover; }

/* Строка */
.op--line .op__plate { padding: calc(6px * var(--u)) calc(16px * var(--u)) calc(8px * var(--u)) calc(6px * var(--u)); border-radius: calc(14px * var(--u)); }
.op--line .op__row { gap: calc(10px * var(--u)); }
.op--line .op__cover { width: calc(30px * var(--u)); height: calc(30px * var(--u)); border-radius: calc(7px * var(--u)); font-size: calc(14px * var(--u)); }
.op__line { overflow: hidden; max-width: calc(440px * var(--u)); font-size: calc(15px * var(--u)); line-height: 1.25; white-space: nowrap; text-overflow: ellipsis; }
.op__line b { font-weight: 650; }
.op__line > span:last-child { color: var(--op-dim); }
.op__dash { color: var(--op-dim); }

/* Карточка */
.op--card .op__plate { padding: calc(10px * var(--u)) calc(18px * var(--u)) calc(14px * var(--u)) calc(10px * var(--u)); border-radius: calc(18px * var(--u)); }
.op--card .op__row { gap: calc(14px * var(--u)); }
.op--card .op__cover { width: calc(60px * var(--u)); height: calc(60px * var(--u)); border-radius: calc(10px * var(--u)); font-size: calc(24px * var(--u)); }
.op__text { display: flex; flex-direction: column; gap: calc(3px * var(--u)); min-width: 0; max-width: calc(320px * var(--u)); }
.op__title, .op__artist { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.op__title { font-size: calc(17px * var(--u)); font-weight: 650; line-height: 1.2; }
.op__artist { font-size: calc(14px * var(--u)); line-height: 1.25; color: var(--op-dim); }

/* Полоска: у строки — по нижнему краю, у карточки — тоже, внутри полей */
.op__bar { position: absolute; right: calc(14px * var(--u)); bottom: calc(5px * var(--u)); left: calc(14px * var(--u)); height: calc(2px * var(--u)); overflow: hidden; border-radius: 2px; background: var(--op-track); }
.op--line .op__bar { right: calc(12px * var(--u)); bottom: calc(3px * var(--u)); left: calc(12px * var(--u)); }
.op__bar i { display: block; width: 100%; height: 100%; border-radius: inherit; background: var(--op-fill); transform-origin: left center; }

/* Появление и уход — без рывков; смена трека — короткий сдвиг и проявление */
.op-show-enter-active, .op-show-leave-active { transition: opacity 0.5s ease, transform 0.5s cubic-bezier(0.2, 0.8, 0.2, 1); }
.op-show-enter-from, .op-show-leave-to { opacity: 0; transform: translateY(8px); }
.op-swap-enter-active { transition: opacity 0.36s ease, transform 0.36s cubic-bezier(0.2, 0.8, 0.2, 1); }
.op-swap-leave-active { transition: opacity 0.2s ease; }
.op-swap-enter-from { opacity: 0; transform: translateX(10px); }
.op--right .op-swap-enter-from { transform: translateX(-10px); }
.op-swap-leave-to { opacity: 0; }

@media (prefers-reduced-motion: reduce) {
  .op-show-enter-active, .op-show-leave-active, .op-swap-enter-active, .op-swap-leave-active { transition: opacity 0.2s ease; }
  .op-show-enter-from, .op-show-leave-to, .op-swap-enter-from { transform: none; }
}
</style>
