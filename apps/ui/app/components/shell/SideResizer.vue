<script setup lang="ts">
import { SIDE_MAX, SIDE_MIN, SIDE_SNAP } from '~/stores/ui'

// Край боковой панели компьютера, как у Spotify: тянуть — ширина от SIDE_MIN до SIDE_MAX, отпустить
// уже SIDE_SNAP — панель схлопывается в рейку (из рейки так же вытягивается), двойной щелчок —
// ширина по умолчанию. С клавиатуры: фокус на крае, стрелки — ширина (с ⇧ — крупнее), Enter — свернуть.
// Стоит у левого края колонки содержимого — за ней и ходит, пока ширина анимируется
const ui = useUiStore()
const { t } = useI18n()
const el = ref<HTMLElement | null>(null)

/** Ширина панели сейчас — по левому краю колонки содержимого */
const edge = () => Math.round((el.value?.offsetParent as HTMLElement | null)?.getBoundingClientRect().left ?? SIDE_MIN)
const width = computed(() => ui.sideWidth ?? SIDE_MIN)

let startX = 0
let startW = 0
let keepW: number | null = null
let moved = false
function down(e: PointerEvent) {
  if (e.button !== 0) return
  e.preventDefault()
  el.value?.setPointerCapture(e.pointerId)
  startX = e.clientX
  startW = edge()
  keepW = ui.sideWidth
  moved = false
  ui.sideResizing = true
  document.documentElement.classList.add('is-col-resize')
}
function move(e: PointerEvent) {
  if (!ui.sideResizing) return
  // Щелчок и двойной щелчок — без сдвига: дрожь руки ширину не трогает
  if (!moved && Math.abs(e.clientX - startX) < 3) return
  moved = true
  const w = startW + e.clientX - startX
  // Уже порога — рейка; развернуть её потом — на ширину, что была до перетаскивания
  if (w < SIDE_SNAP) {
    ui.sideCollapsed = true
    ui.sideWidth = keepW
  }
  else {
    ui.sideCollapsed = false
    ui.setSideWidth(w)
  }
}
function up(e: PointerEvent) {
  if (!ui.sideResizing) return
  if (el.value?.hasPointerCapture(e.pointerId)) el.value.releasePointerCapture(e.pointerId)
  ui.sideResizing = false
  document.documentElement.classList.remove('is-col-resize')
}
onBeforeUnmount(() => {
  ui.sideResizing = false
  document.documentElement.classList.remove('is-col-resize')
})

function reset() {
  ui.sideCollapsed = false
  ui.setSideWidth(null)
}

function key(e: KeyboardEvent) {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault()
    ui.toggleSidebar()
    return
  }
  const dir = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0
  if (!dir) return
  e.preventDefault()
  if (ui.isRail) {
    if (dir > 0) ui.sideCollapsed = false
    return
  }
  const w = ui.sideWidth ?? edge()
  if (dir < 0 && w <= SIDE_MIN) ui.sideCollapsed = true
  else ui.setSideWidth(w + dir * (e.shiftKey ? 64 : 16))
}
</script>

<template>
  <div
    ref="el"
    class="rsz"
    :class="{ 'is-drag': ui.sideResizing }"
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
    :aria-label="t('nav.resize')"
    :aria-valuemin="SIDE_MIN"
    :aria-valuemax="SIDE_MAX"
    :aria-valuenow="ui.isRail ? SIDE_MIN : width"
    :aria-valuetext="ui.isRail ? t('nav.collapsed') : undefined"
    @pointerdown="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="up"
    @lostpointercapture="up"
    @dblclick="reset"
    @keydown="key"
  />
</template>

<style scoped>
/* Полоса захвата 9 px по краю панели: линия появляется при наведении (с задержкой — не мигает,
   когда мышь просто проходит мимо), при перетаскивании и фокусе с клавиатуры */
.rsz { position: absolute; top: 0; bottom: 0; left: -4px; z-index: 6; width: 9px; cursor: col-resize; touch-action: none; outline: none; }
.rsz::before { content: ''; position: absolute; top: 0; bottom: 0; left: 3px; width: 2px; background: var(--text-3); opacity: 0; transition: opacity var(--dur-1); }
.rsz:hover::before { opacity: 0.6; transition-delay: 150ms; }
.rsz.is-drag::before { opacity: 0.8; transition-delay: 0s; }
.rsz:focus-visible::before { background: var(--focus); opacity: 1; transition-delay: 0s; }
</style>

<style>
/* Пока тянут край: курсор везде «ширина», текст не выделяется, ссылки и кнопки не подсвечиваются */
html.is-col-resize, html.is-col-resize * { cursor: col-resize !important; user-select: none !important; }
html.is-col-resize body { pointer-events: none; }
html.is-col-resize .rsz { pointer-events: auto; }
</style>
