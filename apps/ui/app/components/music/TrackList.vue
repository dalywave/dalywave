<script setup lang="ts">
import type { PersonRef, Track } from '~/api/types'
import type { PlayContext } from '~/stores/player'
import type { MenuEntry } from '~/types/ui'

// Список треков. Длинные списки (медиатека на тысячи строк) рисуются окном: в DOM только видимое
// с запасом. Правый клик по любой строке — одно общее меню на список.
// На компьютере: Shift/⌘ + щелчок выбирают несколько строк (меню и полоска внизу — для всех
// выбранных), строки можно перетащить на плейлист в боковой панели
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
  /** Строки без состояния на сервере */
  plain?: boolean
  /** Пункты меню строки этого места («Убрать из плейлиста») */
  extraMenu?: (track: Track, index: number) => MenuEntry[]
  /** Убрать выбранные строки (свой плейлист) */
  removeMany?: (indices: number[]) => void
  /** Совместный плейлист: кто добавил строку (по порядку строк) — маленький аватар */
  addedBy?: (PersonRef | null)[]
}>(), { variant: 'default', start: 1, virtualFrom: 120 })

const player = usePlayerStore()
const ui = useUiStore()
const pui = usePlaylistUi()
const { menuFor } = useTrackActions()
const { t } = useI18n()
const coarse = useMediaQuery('(pointer: coarse)')

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

// ---------- Выбор нескольких строк (компьютер) ----------

const selectable = computed(() => !ui.isPhone && !coarse.value)
const selected = ref(new Set<number>())
let anchor = -1
const selectedTracks = computed(() => [...selected.value].sort((a, b) => a - b).map(i => props.tracks[i]).filter(tr => !!tr))

function clearSelection() {
  if (selected.value.size) selected.value = new Set()
  anchor = -1
}
watch(() => props.tracks, clearSelection)

function onRowClick(i: number, e: MouseEvent) {
  if (!selectable.value || (e.target as HTMLElement).closest('button, a')) return
  if (e.shiftKey || e.metaKey || e.ctrlKey) {
    e.stopPropagation()
    e.preventDefault()
    const next = new Set(selected.value)
    if (e.shiftKey && anchor >= 0) {
      const [a, b] = anchor < i ? [anchor, i] : [i, anchor]
      for (let k = a; k <= b; k++) next.add(k)
    }
    else {
      if (next.has(i)) next.delete(i)
      else next.add(i)
      anchor = i
    }
    selected.value = next
    return
  }
  clearSelection()
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && selected.value.size) clearSelection()
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

function bulkMenu(): MenuEntry[] {
  const tracks = selectedTracks.value
  const indices = [...selected.value]
  return [
    { kind: 'heading', label: t('pl.selected', { n: tracks.length }) },
    { label: t('track.enqueue'), icon: 'ph:list-plus', onSelect: () => player.enqueueMany(tracks, t('pl.selected', { n: tracks.length })) },
    { label: t('pl.add'), icon: 'ph:plus-circle', onSelect: () => pui.pick(tracks) },
    ...(props.removeMany ? [{ label: t('pl.removeFrom'), icon: 'ph:minus-circle', danger: true, onSelect: () => removeSelected(indices) }] : []),
    { kind: 'separator' },
    { label: t('pl.clearSelection'), icon: 'ph:x', onSelect: clearSelection },
  ]
}

function removeSelected(indices = [...selected.value]) {
  props.removeMany?.(indices.sort((a, b) => a - b))
  clearSelection()
}

// ---------- Перетаскивание на плейлист в боковой панели ----------

function onDragStart(i: number, e: DragEvent) {
  const tr = props.tracks[i]
  if (!tr || !e.dataTransfer) return
  const tracks = selected.value.has(i) ? selectedTracks.value : [tr]
  pui.dragTracks = tracks
  e.dataTransfer.effectAllowed = 'copy'
  e.dataTransfer.setData('application/x-dalywave-tracks', tracks.map(x => x.id).join(','))
  e.dataTransfer.setData('text/plain', tracks.map(x => `${x.artists[0]?.name ?? ''} — ${x.title}`).join('\n'))
  // Под указателем — компактная плашка с названием или числом, а не снимок всей строки
  const ghost = document.createElement('div')
  ghost.className = 'tl-ghost'
  ghost.textContent = tracks.length === 1 ? tr.title : t('pl.selected', { n: tracks.length })
  document.body.appendChild(ghost)
  e.dataTransfer.setDragImage(ghost, 16, 16)
  requestAnimationFrame(() => ghost.remove())
}

function onDragEnd() {
  pui.dragTracks = null
}

// Общее контекстное меню: пункты собираются для строки, по которой кликнули
const menuItems = ref<MenuEntry[]>([])
function onContext(target: EventTarget | null) {
  const row = (target as HTMLElement | null)?.closest<HTMLElement>('[data-i]')
  const i = row ? Number(row.dataset.i) : -1
  const tr = props.tracks[i]
  if (tr && selected.value.size > 1 && selected.value.has(i)) {
    menuItems.value = bulkMenu()
    return
  }
  if (selected.value.size && !selected.value.has(i)) clearSelection()
  menuItems.value = tr ? menuFor(tr, { extra: props.extraMenu?.(tr, i) }) : []
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
        :draggable="selectable ? 'true' : undefined"
        @click.capture="onRowClick(i, $event)"
        @dragstart="onDragStart(i, $event)"
        @dragend="onDragEnd"
      >
        <MusicTrackRow
          :track="track"
          :index="variant === 'album' ? (track.trackPosition ?? i + start) : i + start"
          :variant="variant"
          :reason="reasons?.[track.id]"
          :plain="plain"
          :selected="selected.has(i)"
          :by="addedBy ? (addedBy[i] ?? null) : undefined"
          :extra="extraMenu ? () => extraMenu!(track, i) : undefined"
          @play="play(i)"
        />
      </div>
      <!-- Выбрано несколько: действия для всех сразу -->
      <Transition name="tl-bar">
        <div v-if="selected.size > 1" class="tl-bar" role="toolbar" :aria-label="t('pl.selected', { n: selected.size })">
          <span class="tl-bar__count tnum">{{ t('pl.selected', { n: selected.size }) }}</span>
          <UiButton size="sm" icon="ph:plus-circle" @click="pui.pick(selectedTracks)">{{ t('pl.add') }}</UiButton>
          <UiButton v-if="removeMany" size="sm" icon="ph:minus-circle" @click="removeSelected()">{{ t('pl.remove') }}</UiButton>
          <UiIconButton icon="ph:x" :label="t('pl.clearSelection')" size="sm" @click="clearSelection" />
        </div>
      </Transition>
    </div>
  </UiContextMenu>
</template>

<style scoped>
.tl { position: relative; }
.tl__row[draggable='true'] { -webkit-user-drag: element; }
.tl-bar {
  position: sticky;
  bottom: 16px;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 8px;
  width: max-content;
  max-width: 100%;
  margin: 12px auto 0;
  padding: 6px 6px 6px 16px;
  border-radius: var(--r-pill);
  background: var(--ink-3);
  box-shadow: 0 0 0 1px var(--line), var(--shadow-pop);
}
.tl-bar__count { margin-right: 4px; font-size: var(--fs-small); font-weight: 600; white-space: nowrap; }
.tl-bar-enter-active, .tl-bar-leave-active { transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.tl-bar-enter-from, .tl-bar-leave-to { opacity: 0; transform: translateY(8px); }
.tl--virtual .tl__row { position: absolute; left: 0; right: 0; top: 0; }
</style>

<style>
.tl-ghost { position: fixed; top: -100px; left: 0; max-width: 280px; padding: 8px 14px; overflow: hidden; border-radius: var(--r-pill); background: var(--ink-3); box-shadow: 0 0 0 1px var(--line); color: var(--text-1); font: 600 13px/1.3 var(--font); text-overflow: ellipsis; white-space: nowrap; pointer-events: none; }
</style>
