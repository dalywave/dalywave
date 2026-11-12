<script lang="ts">
import { DialogContent, DialogDescription, DialogOverlay, DialogPortal, DialogRoot, DialogTitle, DialogTrigger } from 'reka-ui'

// Открытые листы друг над другом (меню трека из очереди): каждый следующий — слоем выше
let depth = 0
</script>

<script setup lang="ts">
// Лист снизу — один на всё: меню «⋯», режим переходов, устройства, текст, очередь. Как в iOS:
// ручка, лист едет за пальцем, фон темнеет и светлеет вместе с ним; отпустили — ближайшая ступень
// с учётом скорости (резкий жест вниз закрывает). `fit` — по высоте содержимого; `expand` —
// сначала до половины экрана (или по содержимому, если его меньше), вверх — во весь экран.
// Список внутри прокручивается, только когда лист раскрыт полностью или список уже не в начале —
// иначе палец двигает сам лист. Двигается только transform: раскладка не пересчитывается
const props = withDefaults(defineProps<{
  title: string
  description?: string
  /** Заголовок виден; иначе только для экранного диктора (своя шапка — в слоте `head`) */
  showTitle?: boolean
  detents?: 'fit' | 'expand'
  /** Карточкой с отступами от краёв, как меню «⋯»; подвал — отдельной карточкой ниже */
  inset?: boolean
}>(), { showTitle: true, detents: 'fit', inset: false })

const open = defineModel<boolean>('open', { default: false })
const ui = useUiStore()
const bridge = useBridge()

const frame = ref<HTMLElement | null>(null)
const body = ref<HTMLElement | null>(null)
const inner = ref<HTMLElement | null>(null)
/** Лист в дереве (Dialog открыт); закрытие сначала доигрывает анимацию */
const shown = ref(false)
/** Сдвиг вниз от раскрытого положения, px */
const y = ref(0)
const animating = ref(false)
const dragging = ref(false)
/** Индекс ступени в `stops` (0 — нижняя) */
const level = ref(0)
const z = ref(150)
/** Ступени: сдвиг вниз, от нижней к верхней (0 — раскрыт) */
const stops = shallowRef<number[]>([0])
let closedY = 0

const content = () => frame.value?.parentElement ?? null
const expanded = computed(() => level.value === stops.value.length - 1)

const sheetStyle = computed(() => ({ transform: `translate3d(0, ${Math.round(y.value * 10) / 10}px, 0)`, zIndex: z.value + 1 }))
// Фон темнеет по мере подъёма листа до нижней ступени
const overlayStyle = computed(() => {
  const low = stops.value[0] ?? 0
  const span = Math.max(1, closedY - low)
  return { opacity: String(Math.min(1, Math.max(0, (closedY - y.value) / span))), zIndex: z.value }
})

// ---------- Ступени ----------
function measure() {
  const c = content()
  if (!c) return
  const top = c.getBoundingClientRect().top - y.value
  closedY = Math.ceil(window.innerHeight - top)
  if (props.detents === 'fit' || !body.value || !inner.value) {
    stops.value = [0]
    return
  }
  const full = c.offsetHeight
  const natural = full - body.value.clientHeight + inner.value.offsetHeight
  const low = Math.min(natural, Math.round(full * 0.56))
  stops.value = low >= full - 24 ? [0] : [full - low, 0]
}

// ---------- Открыть и закрыть ----------
function waitTransition(el: HTMLElement, then: () => void) {
  let fired = false
  const finish = (e?: TransitionEvent) => {
    if (fired || (e && (e.target !== el || e.propertyName !== 'transform'))) return
    fired = true
    el.removeEventListener('transitionend', finish)
    then()
  }
  el.addEventListener('transitionend', finish)
  setTimeout(finish, 520)
}

function settle(to: number, then?: () => void) {
  const c = content()
  if (ui.reducedMotion || !c || Math.abs(to - y.value) < 1) {
    animating.value = false
    y.value = to
    then?.()
    return
  }
  animating.value = true
  y.value = to
  waitTransition(c, () => {
    animating.value = false
    then?.()
  })
}

watch(open, async (v) => {
  if (v) {
    if (shown.value) return
    z.value = 150 + Math.min(depth, 4) * 20
    depth++
    animating.value = false
    y.value = window.innerHeight
    level.value = 0
    shown.value = true
    await nextTick()
    requestAnimationFrame(() => {
      measure()
      y.value = closedY
      content()?.getBoundingClientRect()
      settle(stops.value[0]!)
    })
  }
  else if (shown.value) {
    measure()
    settle(closedY, () => {
      shown.value = false
      depth = Math.max(0, depth - 1)
    })
  }
}, { immediate: true })

onBeforeUnmount(() => {
  if (shown.value) depth = Math.max(0, depth - 1)
})

function onRootOpen(v: boolean) {
  open.value = v
}

// Содержимое выросло (очередь догрузилась): нижняя ступень — по новой высоте
let resize: ResizeObserver | null = null
watch(inner, (el) => {
  resize?.disconnect()
  if (!el || props.detents === 'fit') return
  resize = new ResizeObserver(() => {
    if (!open.value || dragging.value || animating.value) return
    const was = stops.value[level.value]
    measure()
    level.value = Math.min(level.value, stops.value.length - 1)
    if (stops.value[level.value] !== was) settle(stops.value[level.value]!)
  })
  resize.observe(el)
})
onBeforeUnmount(() => resize?.disconnect())

// ---------- Жест ----------
let g: { id: number, x: number, y: number, from: number, lastY: number, lastT: number, v: number, axis: 'x' | 'y' | null, scroller: HTMLElement | null, past: boolean } | null = null
let swallowClick = false

function onDown(e: PointerEvent) {
  if (!e.isPrimary || (e.pointerType === 'mouse' && e.button !== 0) || !open.value) return
  const target = e.target as HTMLElement
  if (target.closest('input, textarea, select, [role="slider"], [data-sheet-nodrag]')) return
  // Поймали лист на лету — продолжаем с того места, где он сейчас
  if (animating.value) {
    const c = content()
    if (c) y.value = new DOMMatrixReadOnly(getComputedStyle(c).transform).m42
    animating.value = false
  }
  g = { id: e.pointerId, x: e.clientX, y: e.clientY, from: y.value, lastY: e.clientY, lastT: e.timeStamp, v: 0, axis: null, scroller: target.closest<HTMLElement>('.uis__body'), past: false }
  // Палец быстрее листа: первые движения могут прийти уже над фоном — слушаем окно
  listen(true)
}

function listen(on: boolean) {
  const f = on ? window.addEventListener.bind(window) : window.removeEventListener.bind(window)
  f('pointermove', onMove, true)
  f('pointerup', onUp, true)
  f('pointercancel', onCancel, true)
}
onBeforeUnmount(() => listen(false))

function rubber(over: number) {
  return Math.min(56, Math.sqrt(over) * 3.2)
}

function onMove(e: PointerEvent) {
  const d = g
  if (!d || e.pointerId !== d.id) return
  const dx = e.clientX - d.x
  const dy = e.clientY - d.y
  if (!d.axis) {
    if (Math.hypot(dx, dy) < 6) return
    if (Math.abs(dx) > Math.abs(dy)) {
      g = null
      listen(false)
      return
    }
    d.axis = 'y'
    const sc = d.scroller
    if (sc && sc.scrollHeight > sc.clientHeight + 1) {
      // Раскрыт полностью — список прокручивается; не в начале — сначала докручиваем его
      if ((expanded.value && dy < 0) || sc.scrollTop > 0) {
        g = null
        listen(false)
        return
      }
    }
    dragging.value = true
    try {
      content()?.setPointerCapture(e.pointerId)
    }
    catch { /* касание уже кончилось */ }
  }
  const dt = Math.max(1, e.timeStamp - d.lastT)
  d.v = 0.7 * ((e.clientY - d.lastY) / dt) + 0.3 * d.v
  d.lastY = e.clientY
  d.lastT = e.timeStamp
  const raw = d.from + dy
  y.value = raw >= 0 ? raw : -rubber(-raw)
  // iPhone: щелчок, когда жест перешёл порог закрытия (и когда вернулся обратно)
  const past = y.value > (stops.value[0]! + closedY) / 2
  if (past !== d.past) {
    d.past = past
    bridge.haptic('selection')
  }
}

function onUp(e: PointerEvent) {
  const d = g
  if (!d || e.pointerId !== d.id) return
  g = null
  listen(false)
  if (!dragging.value) return
  dragging.value = false
  swallowClick = true
  window.addEventListener('click', onClick, { capture: true, once: true })
  setTimeout(() => {
    swallowClick = false
    window.removeEventListener('click', onClick, true)
  }, 0)
  // Палец замер перед тем, как отпустить, — скорости нет
  const v = e.timeStamp - d.lastT > 80 ? 0 : d.v
  // Куда долетел бы лист по инерции (как в UIKit: ~0,2 с при скорости отпускания)
  const projected = y.value + v * 200
  const targets = [...stops.value, closedY]
  let best = targets[0]!
  for (const t of targets) if (Math.abs(t - projected) < Math.abs(best - projected)) best = t
  // Резкий жест вниз с нижней ступени — закрыть, вверх — на ступень выше
  if (v > 1.1 && y.value > stops.value[0]! - 4) best = closedY
  else if (v < -0.9) best = stops.value.find(s => s < y.value - 4) ?? stops.value.at(-1)!
  if (best === closedY) {
    open.value = false
    return
  }
  const to = stops.value.indexOf(best)
  // Лист встал на другую ступень — щелчок, как у листов iOS
  if (to !== level.value) bridge.haptic('selection')
  level.value = to
  settle(best)
}

function onCancel(e: PointerEvent) {
  if (!g || e.pointerId !== g.id) return
  g = null
  listen(false)
  if (!dragging.value) return
  dragging.value = false
  settle(stops.value[level.value] ?? 0)
}

function onClick(e: MouseEvent) {
  if (!swallowClick) return
  e.stopPropagation()
  e.preventDefault()
}

// iOS: прокрутку тянуть вниз, когда список в начале, — это жест листа, а не отскок списка.
// Решаем по первому же движению пальца, пока система не начала прокрутку сама
let touchY = 0
function onTouchStart(e: TouchEvent) {
  touchY = e.touches[0]?.clientY ?? 0
}
function onTouchMove(e: TouchEvent) {
  if (!e.cancelable) return
  if (dragging.value) {
    e.preventDefault()
    return
  }
  const sc = (e.target as HTMLElement).closest<HTMLElement>('.uis__body')
  if (sc && sc.scrollTop <= 0 && (e.touches[0]?.clientY ?? 0) > touchY) e.preventDefault()
}
watch(frame, (el, old) => {
  const c = old?.parentElement
  c?.removeEventListener('touchstart', onTouchStart)
  c?.removeEventListener('touchmove', onTouchMove)
  const n = el?.parentElement
  n?.addEventListener('touchstart', onTouchStart, { passive: true })
  n?.addEventListener('touchmove', onTouchMove, { passive: false })
})
</script>

<template>
  <DialogRoot :open="shown" @update:open="onRootOpen">
    <DialogTrigger v-if="$slots.trigger" as-child>
      <slot name="trigger" />
    </DialogTrigger>
    <DialogPortal>
      <DialogOverlay class="uis-overlay" :class="{ 'is-anim': animating }" :style="overlayStyle" />
      <DialogContent
        class="uis"
        :class="[`uis--${detents}`, { 'uis--inset': inset, 'is-anim': animating, 'is-dragging': dragging }]"
        :style="sheetStyle"
        :aria-describedby="description ? undefined : undefined"
        @open-auto-focus.prevent
        @pointerdown="onDown"
      >
        <div ref="frame" class="uis__card">
          <div class="uis__grab" aria-hidden="true" />
          <slot name="head">
            <header v-if="showTitle" class="uis__head">
              <DialogTitle class="uis__title">{{ title }}</DialogTitle>
              <slot name="head-actions" />
            </header>
          </slot>
          <DialogTitle v-if="!showTitle" class="sr-only">{{ title }}</DialogTitle>
          <DialogDescription v-if="description" class="sr-only">{{ description }}</DialogDescription>
          <div ref="body" class="uis__body" :class="{ 'is-scroll': expanded }">
            <div ref="inner" class="uis__inner">
              <slot :close="() => (open = false)" />
            </div>
          </div>
          <div v-if="$slots.footer && !inset" class="uis__foot">
            <slot name="footer" :close="() => (open = false)" />
          </div>
        </div>
        <div v-if="$slots.footer && inset" class="uis__foot-card">
          <slot name="footer" :close="() => (open = false)" />
        </div>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

<style>
.uis-overlay { position: fixed; inset: 0; background: var(--overlay); will-change: opacity; }
.uis-overlay.is-anim { transition: opacity 420ms cubic-bezier(0.32, 0.72, 0, 1); }
.uis {
  position: fixed;
  left: 0;
  right: 0;
  bottom: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: calc(100dvh - var(--safe-top) - 8px);
  outline: none;
  touch-action: none;
  will-change: transform;
  -webkit-tap-highlight-color: transparent;
}
.uis.is-anim { transition: transform 420ms cubic-bezier(0.32, 0.72, 0, 1); }
.uis--expand { height: calc(100dvh - var(--safe-top) - 8px); }
.uis__card {
  position: relative;
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  padding-bottom: var(--safe-bottom);
  border-radius: 20px 20px 0 0;
  background: var(--sheet);
  -webkit-backdrop-filter: blur(40px) saturate(1.6);
  backdrop-filter: blur(40px) saturate(1.6);
  box-shadow: 0 0 0 0.5px rgba(var(--hi-rgb), 0.08), 0 -12px 40px -12px rgba(var(--shade-rgb), calc(0.6 * var(--shade-k)));
}
/* Лист у края при отскоке вверх отрывается от низа экрана — продолжение фона закрывает щель */
.uis:not(.uis--inset)::after { content: ''; position: absolute; left: 0; right: 0; top: 100%; height: 80px; background: var(--sheet-solid); }
.uis--inset { left: 8px; right: 8px; bottom: calc(var(--safe-bottom) + 8px); max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - 40px); }
.uis--inset .uis__card { flex: 0 1 auto; padding-bottom: 0; border-radius: 18px; box-shadow: 0 0 0 0.5px rgba(var(--hi-rgb), 0.08), 0 20px 50px -12px rgba(var(--shade-rgb), calc(0.8 * var(--shade-k))); }
.uis__grab { width: 36px; height: 5px; margin: 6px auto 0; flex: none; border-radius: 3px; background: rgba(var(--hi-rgb), 0.24); }
.uis__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 48px; padding: 4px 20px 6px; flex: none; }
.uis__title { margin: 0; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; }
/* Прокрутка — своя у списка, и touch-action у предков на неё не действует: запрещаем здесь же,
   пока лист не раскрыт полностью */
.uis__body { flex: 1 1 auto; min-height: 0; overflow-y: auto; overscroll-behavior: contain; scrollbar-width: none; touch-action: none; }
.uis--fit .uis__card, .uis--fit .uis__body { flex: 0 1 auto; }
.uis__body.is-scroll { touch-action: pan-y; }
.uis__body::-webkit-scrollbar { display: none; }
.uis__foot { flex: none; }
/* Планшет под пальцем: лист не тянется на всю ширину экрана — колонкой по центру, как листы iPadOS */
@media (pointer: coarse) and (min-width: 700px) {
  .uis, .uis--inset { left: max(16px, calc(50% - 300px)); right: max(16px, calc(50% - 300px)); }
  .uis--expand { height: min(calc(100dvh - var(--safe-top) - 8px), 88dvh); }
  .uis__card { border-radius: 24px 24px 0 0; }
  .uis--inset .uis__card { border-radius: 20px; }
}
.uis__foot-card {
  flex: none;
  overflow: hidden;
  border-radius: 18px;
  background: var(--sheet-2);
  -webkit-backdrop-filter: blur(40px) saturate(1.6);
  backdrop-filter: blur(40px) saturate(1.6);
  box-shadow: 0 0 0 0.5px rgba(var(--hi-rgb), 0.08);
}
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
@media (prefers-reduced-motion: reduce) {
  .uis.is-anim, .uis-overlay.is-anim { transition: none; }
}
</style>
