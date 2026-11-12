// Перестановка строк списка за ручку — мышью, пальцем и пером (pointer events). Строка едет за
// указателем, соседи расступаются; у края прокрутки список прокручивается сам. Клавиатура
// (Alt + ↑/↓) — у вызывающего: он просто зовёт onMove(i, i ± 1)

const EDGE = 72
const MAX_SPEED = 18

function findScroller(el: HTMLElement | null): HTMLElement | null {
  let p = el?.parentElement ?? null
  while (p) {
    const oy = getComputedStyle(p).overflowY
    if ((oy === 'auto' || oy === 'scroll') && p.scrollHeight > p.clientHeight) return p
    p = p.parentElement
  }
  return null
}

export function useSortable(opts: {
  /** Контейнер, строки внутри — с атрибутом data-sort-i */
  container: Ref<HTMLElement | null>
  onMove: (from: number, to: number) => void
}) {
  const from = ref<number | null>(null)
  const to = ref<number | null>(null)
  const dy = ref(0)

  let scroller: HTMLElement | null = null
  let tops: number[] = []
  let heights: number[] = []
  let startY = 0
  let startScroll = 0
  let pointerY = 0
  let raf = 0
  let pointerId = -1
  let handle: HTMLElement | null = null

  const scrollTop = () => (scroller ? scroller.scrollTop : window.scrollY)
  const viewport = () => {
    if (scroller) {
      const r = scroller.getBoundingClientRect()
      return { top: r.top, bottom: r.bottom }
    }
    return { top: 0, bottom: window.innerHeight }
  }

  function rows(): HTMLElement[] {
    return [...(opts.container.value?.querySelectorAll<HTMLElement>('[data-sort-i]') ?? [])]
  }

  function update() {
    const f = from.value
    if (f == null) return
    const delta = pointerY + scrollTop() - (startY + startScroll)
    dy.value = delta
    const center = tops[f]! + heights[f]! / 2 + delta
    // Куда встанет строка: сколько других строк выше её середины
    let target = 0
    for (let j = 0; j < tops.length; j++) {
      if (j === f) continue
      if (tops[j]! + heights[j]! / 2 < center) target++
    }
    to.value = target
  }

  function tick() {
    raf = 0
    if (from.value == null) return
    const { top, bottom } = viewport()
    let speed = 0
    if (pointerY < top + EDGE) speed = -MAX_SPEED * Math.min(1, (top + EDGE - pointerY) / EDGE)
    else if (pointerY > bottom - EDGE) speed = MAX_SPEED * Math.min(1, (pointerY - (bottom - EDGE)) / EDGE)
    if (speed) {
      if (scroller) scroller.scrollTop += speed
      else window.scrollBy(0, speed)
      update()
    }
    raf = requestAnimationFrame(tick)
  }

  function onMovePointer(e: PointerEvent) {
    if (e.pointerId !== pointerId) return
    e.preventDefault()
    pointerY = e.clientY
    update()
  }

  function finish(commit: boolean) {
    const f = from.value
    const t = to.value
    window.removeEventListener('pointermove', onMovePointer)
    window.removeEventListener('pointerup', onUp)
    window.removeEventListener('pointercancel', onCancel)
    cancelAnimationFrame(raf)
    raf = 0
    try {
      handle?.releasePointerCapture(pointerId)
    }
    catch {
      // Захват уже снят
    }
    handle = null
    from.value = null
    to.value = null
    dy.value = 0
    if (commit && f != null && t != null && t !== f) opts.onMove(f, t)
  }
  const onUp = (e: PointerEvent) => e.pointerId === pointerId && finish(true)
  const onCancel = (e: PointerEvent) => e.pointerId === pointerId && finish(false)

  /** pointerdown на ручке строки `i` */
  function start(i: number, e: PointerEvent) {
    if (e.button !== 0 || from.value != null) return
    e.preventDefault()
    const list = rows()
    scroller = findScroller(opts.container.value)
    startScroll = scrollTop()
    tops = list.map(r => r.getBoundingClientRect().top + startScroll)
    heights = list.map(r => r.offsetHeight)
    startY = e.clientY
    pointerY = e.clientY
    pointerId = e.pointerId
    handle = e.currentTarget as HTMLElement
    try {
      handle.setPointerCapture(e.pointerId)
    }
    catch {
      // Без захвата тоже работает: слушаем окно
    }
    from.value = i
    to.value = i
    window.addEventListener('pointermove', onMovePointer, { passive: false })
    window.addEventListener('pointerup', onUp)
    window.addEventListener('pointercancel', onCancel)
    raf = requestAnimationFrame(tick)
  }

  /** Сдвиг строки `i`, пока тянут другую (или её саму) */
  function offset(i: number): number {
    const f = from.value
    const t = to.value
    if (f == null || t == null) return 0
    if (i === f) return dy.value
    const h = heights[f] ?? 0
    if (f < t && i > f && i <= t) return -h
    if (t < f && i >= t && i < f) return h
    return 0
  }

  onBeforeUnmount(() => finish(false))

  return { from, to, start, offset, dragging: computed(() => from.value != null) }
}
