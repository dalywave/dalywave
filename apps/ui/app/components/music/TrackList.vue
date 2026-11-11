<script setup lang="ts">
import type { Track } from '~/api/types'
import type { PlayContext } from '~/stores/player'
import type { MenuEntry } from '~/types/ui'

// Список треков. Длинные списки (медиатека на тысячи строк) рисуются окном: в DOM только видимое
// с запасом. Правый клик по любой строке — одно общее меню на список
const props = withDefaults(defineProps<{
  tracks: Track[]
  context: PlayContext
  variant?: 'default' | 'compact' | 'queue' | 'album' | 'wide'
  reasons?: Record<string, string>
  /** Номер первой строки */
  start?: number
  /** С какого размера включать окно */
  virtualFrom?: number
  label?: string
}>(), { variant: 'default', start: 1, virtualFrom: 120 })

const player = usePlayerStore()
const ui = useUiStore()
const { menuFor } = useTrackActions()

const root = ref<HTMLElement | null>(null)
const rowH = computed(() => (ui.isPhone ? 60 : props.variant === 'album' ? 52 : 56))
const virtual = computed(() => props.tracks.length >= props.virtualFrom)
const range = ref({ from: 0, to: 60 })
const OVERSCAN = 8

let scroller: HTMLElement | Window | null = null

function findScroller(el: HTMLElement | null): HTMLElement | Window {
  let p = el?.parentElement ?? null
  while (p) {
    const oy = getComputedStyle(p).overflowY
    if (oy === 'auto' || oy === 'scroll') return p
    p = p.parentElement
  }
  return window
}

function measure() {
  if (!virtual.value || !root.value || !scroller) return
  const listTop = root.value.getBoundingClientRect().top
  const viewTop = scroller instanceof Window ? 0 : scroller.getBoundingClientRect().top
  const viewH = scroller instanceof Window ? window.innerHeight : scroller.clientHeight
  const offset = viewTop - listTop
  const from = Math.max(0, Math.floor(offset / rowH.value) - OVERSCAN)
  const to = Math.min(props.tracks.length, Math.ceil((offset + viewH) / rowH.value) + OVERSCAN)
  if (from !== range.value.from || to !== range.value.to) range.value = { from, to }
}

let raf = 0
const onScroll = () => {
  if (raf) return
  raf = requestAnimationFrame(() => {
    raf = 0
    measure()
  })
}

onMounted(() => {
  scroller = findScroller(root.value)
  scroller.addEventListener('scroll', onScroll, { passive: true })
  window.addEventListener('resize', onScroll)
  measure()
})
onBeforeUnmount(() => {
  scroller?.removeEventListener('scroll', onScroll)
  window.removeEventListener('resize', onScroll)
  cancelAnimationFrame(raf)
})
watch(() => props.tracks.length, () => nextTick(measure))

const visible = computed(() => {
  if (!virtual.value) return props.tracks.map((track, i) => ({ track, i }))
  return props.tracks.slice(range.value.from, range.value.to).map((track, k) => ({ track, i: range.value.from + k }))
})

function play(i: number) {
  const tr = props.tracks[i]
  if (!tr) return
  if (player.current?.track.id === tr.id) void player.toggle()
  else void player.playTracks(props.tracks, i, props.context)
}

// Общее контекстное меню: пункты собираются для строки, по которой кликнули
const menuItems = ref<MenuEntry[]>([])
function onContext(target: EventTarget | null) {
  const row = (target as HTMLElement | null)?.closest<HTMLElement>('[data-i]')
  const tr = row ? props.tracks[Number(row.dataset.i)] : undefined
  menuItems.value = tr ? menuFor(tr) : []
}
</script>

<template>
  <UiContextMenu :items="menuItems" @open="onContext">
    <div
      ref="root"
      class="tl"
      role="table"
      :aria-label="label ?? context.title"
      :aria-rowcount="tracks.length"
      :style="virtual ? { height: `${tracks.length * rowH}px` } : undefined"
      :class="{ 'tl--virtual': virtual }"
    >
      <div
        v-for="{ track, i } in visible"
        :key="track.id + ':' + i"
        :data-i="i"
        class="tl__row"
        :style="virtual ? { transform: `translateY(${i * rowH}px)`, height: `${rowH}px` } : undefined"
      >
        <MusicTrackRow
          :track="track"
          :index="variant === 'album' ? (track.trackPosition ?? i + start) : i + start"
          :variant="variant"
          :reason="reasons?.[track.id]"
          @play="play(i)"
        />
      </div>
    </div>
  </UiContextMenu>
</template>

<style scoped>
.tl { position: relative; }
.tl--virtual .tl__row { position: absolute; left: 0; right: 0; top: 0; }
</style>
