import { dragScrollTop, pageDelta, thumbGeometry, wheelPixels } from '~/utils/scrollbars'

// Классические полосы прокрутки занимают место (Windows и WebView2, Linux, Mac с «Показывать полосы
// прокрутки: всегда»), накладные — нет (macOS по умолчанию, iOS, телефоны). Для классических на корень
// ставится data-scrollbars="classic": main.css прячет системную вертикальную полосу (место под неё не
// занимается), а вместо неё здесь рисуется накладная, как на Mac и в Spotify, — тонкий ползунок поверх
// содержимого у правого края. Он виден, пока область прокручивают или водят над ней указателем, и гаснет
// через секунду покоя; за него можно взяться и тащить, щелчок по пустой дорожке — прокрутка на страницу.
// Накладные системные полосы (Apple) не трогаем.
//
// Механизм один на всё приложение: области не размечаются — полоса появляется у ближайшего вертикально
// прокручиваемого предка (главная колонка, боковая и правая панели, админка, окна, поповеры, меню).
// Полоса — отдельный узел в body (position: fixed), поэтому разметку областей не трогает: Vue, виртуальные
// списки и ссылки на прокручиваемый элемент работают как раньше. Без полосы остаются области, где
// прокрутка спрятана нарочно (scrollbar-width: none, .no-scrollbar), и data-overlay-scroll="off".
// Отступы дорожки от краёв области — свойства --scrollbar-inset-top / -bottom (main.css, @property).
//
// Производительность: размеры области кэшируются и обновляются по ResizeObserver (сама область и её
// прямые потомки — в них растёт содержимое), а при прокрутке раз в кадр читается только scrollTop и
// ползунку меняется transform.

const HIDE_AFTER = 1000 // мс покоя — и полоса гаснет
const INPUT_WINDOW = 800 // прокрутка без колеса/клавиш/касания (программная, смена страницы) полосу не зажигает
const PAD = 2 // дорожка короче области на столько с каждого конца
const WIDTH = 12 // ширина полосы (зона под указатель), px — как правый отступ боковой панели

interface Bar {
  el: HTMLElement
  node: HTMLDivElement
  thumb: HTMLDivElement
  ro: ResizeObserver
  mo: MutationObserver
  /** Кэш размеров (measure) */
  scrollHeight: number
  clientHeight: number
  track: number
  top: number
  /** Последнее положение ползунка */
  size: number
  offset: number
  colors: string
  shown: boolean
  over: boolean
  lastActive: number
  timer: ReturnType<typeof setTimeout> | 0
  drag: { y: number, top: number, id: number } | null
}

interface Measured {
  bar: Bar
  left: number
  top: number
  track: number
  scrollHeight: number
  clientHeight: number
  colors: string
}

function createOverlayScrollbars(): () => void {
  const doc = document
  const bars = new Map<HTMLElement, Bar>()
  const byNode = new WeakMap<Element, Bar>()
  const webkit = CSS.supports('selector(::-webkit-scrollbar)')
  const reduced = matchMedia('(prefers-reduced-motion: reduce)')

  let raf = 0
  let lastInput = 0
  let hoverEl: HTMLElement | null = null
  let hoverTarget: EventTarget | null | undefined
  let moveReveal = false
  const scrolled = new Set<HTMLElement>()
  const dirty = new Set<Bar>()
  const moved = new Set<Bar>()

  const schedule = () => {
    if (!raf) raf = requestAnimationFrame(frame)
  }
  const rootEl = () => (doc.scrollingElement as HTMLElement | null) ?? doc.documentElement

  // ---------- Какая область прокручивается ----------
  function optedOut(el: HTMLElement, cs: CSSStyleDeclaration) {
    if (el.dataset.overlayScroll === 'off') return true
    // Firefox прячет системную полосу у всех через scrollbar-width: none (main.css) — там признак отказа — класс
    return webkit ? cs.scrollbarWidth === 'none' : el.classList.contains('no-scrollbar')
  }
  function canScroll(el: HTMLElement) {
    const cs = getComputedStyle(el)
    const root = el === rootEl()
    if (root ? cs.overflowY === 'hidden' || cs.overflowY === 'clip' : !/^(?:auto|scroll|overlay)$/.test(cs.overflowY)) return false
    if (optedOut(el, cs)) return false
    return el.scrollHeight > el.clientHeight + 1
  }
  function scrollerOf(target: EventTarget | null): HTMLElement | null {
    const root = rootEl()
    for (let el = target instanceof Element ? target : null; el && el !== doc.body && el !== root; el = el.parentElement) {
      const own = byNode.get(el)
      if (own) return own.el
      if (el instanceof HTMLElement && canScroll(el)) return el
    }
    return canScroll(root) ? root : null
  }

  // ---------- Полоса ----------
  function create(el: HTMLElement): Bar {
    for (const b of bars.values()) if (!b.el.isConnected) destroy(b)
    const node = doc.createElement('div')
    node.className = 'ovs'
    node.setAttribute('aria-hidden', 'true')
    const thumb = doc.createElement('div')
    thumb.className = 'ovs__thumb'
    node.append(thumb)
    const bar: Bar = {
      el, node, thumb,
      ro: new ResizeObserver(() => touch(bar)),
      mo: new MutationObserver(() => {
        watchChildren(bar)
        touch(bar)
      }),
      scrollHeight: 0, clientHeight: 0, track: 0, top: 0, size: -1, offset: -1, colors: '',
      shown: false, over: false, lastActive: 0, timer: 0, drag: null,
    }
    watchChildren(bar)
    bar.mo.observe(el, { childList: true })
    node.addEventListener('pointerenter', () => {
      bar.over = true
      show(bar, performance.now())
    })
    node.addEventListener('pointerleave', () => {
      bar.over = false
      bar.lastActive = performance.now()
    })
    node.addEventListener('pointerdown', e => onDown(bar, e))
    thumb.addEventListener('pointermove', e => onDrag(bar, e))
    thumb.addEventListener('pointerup', () => endDrag(bar))
    thumb.addEventListener('pointercancel', () => endDrag(bar))
    thumb.addEventListener('lostpointercapture', () => endDrag(bar))
    node.addEventListener('wheel', e => onWheel(bar, e), { passive: false })
    doc.body.append(node)
    bars.set(el, bar)
    byNode.set(node, bar)
    byNode.set(thumb, bar)
    return bar
  }
  // Содержимое растёт в прямых потомках области — следим и за ними (новая страница — новый потомок)
  function watchChildren(bar: Bar) {
    bar.ro.disconnect()
    bar.ro.observe(bar.el)
    for (const child of bar.el.children) bar.ro.observe(child)
  }
  function touch(bar: Bar) {
    if (!bar.shown) return
    dirty.add(bar)
    schedule()
  }
  function destroy(bar: Bar) {
    if (bar.timer) clearTimeout(bar.timer)
    bar.ro.disconnect()
    bar.mo.disconnect()
    bar.node.remove()
    bars.delete(bar.el)
    dirty.delete(bar)
    moved.delete(bar)
    if (hoverEl === bar.el) hoverEl = null
  }

  function show(bar: Bar, now: number) {
    bar.lastActive = now
    if (!bar.shown) {
      bar.shown = true
      dirty.add(bar)
      schedule()
    }
    if (!bar.timer) bar.timer = setTimeout(() => check(bar), HIDE_AFTER)
  }
  function hide(bar: Bar) {
    if (bar.timer) clearTimeout(bar.timer)
    bar.timer = 0
    bar.shown = false
    bar.node.classList.remove('is-on')
  }
  function check(bar: Bar) {
    bar.timer = 0
    if (!bar.el.isConnected) return destroy(bar)
    if (!bar.shown) return
    const rest = performance.now() - bar.lastActive
    if (bar.drag || bar.over || rest < HIDE_AFTER) {
      bar.timer = setTimeout(() => check(bar), bar.drag || bar.over ? HIDE_AFTER : HIDE_AFTER - rest)
      return
    }
    hide(bar)
  }

  // ---------- Кадр: сначала все чтения, потом все записи ----------
  function read(bar: Bar): Measured {
    const el = bar.el
    let left = 0
    let top = 0
    if (el !== rootEl()) {
      const r = el.getBoundingClientRect()
      left = r.left + el.clientLeft
      top = r.top + el.clientTop
    }
    const cs = getComputedStyle(el)
    const insetTop = Number.parseFloat(cs.getPropertyValue('--scrollbar-inset-top')) || 0
    const insetBottom = Number.parseFloat(cs.getPropertyValue('--scrollbar-inset-bottom')) || 0
    // Цвет — от самой области: тёмный остров в светлой теме получает свой ползунок
    const colors = `${cs.getPropertyValue('--scroll-overlay').trim()}|${cs.getPropertyValue('--scroll-overlay-hover').trim()}`
    return {
      bar,
      left: left + el.clientWidth - WIDTH,
      top: top + insetTop + PAD,
      track: el.clientHeight - insetTop - insetBottom - 2 * PAD,
      scrollHeight: el.scrollHeight,
      clientHeight: el.clientHeight,
      colors,
    }
  }
  function apply(m: Measured) {
    const bar = m.bar
    const node = bar.node
    bar.scrollHeight = m.scrollHeight
    bar.clientHeight = m.clientHeight
    bar.track = m.track
    bar.top = m.top
    node.style.transform = `translate3d(${Math.round(m.left)}px, ${Math.round(m.top)}px, 0)`
    node.style.height = `${Math.max(0, m.track)}px`
    if (m.colors !== bar.colors) {
      bar.colors = m.colors
      const [thumb, hover] = m.colors.split('|')
      node.style.setProperty('--ovs-thumb', thumb || null)
      node.style.setProperty('--ovs-thumb-hover', hover || null)
    }
  }
  function place(bar: Bar, scrollTop: number) {
    const g = thumbGeometry({ scrollTop, scrollHeight: bar.scrollHeight, clientHeight: bar.clientHeight, track: bar.track })
    if (!g) return hide(bar)
    if (g.size !== bar.size) {
      bar.size = g.size
      bar.thumb.style.height = `${g.size}px`
    }
    if (g.offset !== bar.offset) {
      bar.offset = g.offset
      bar.thumb.style.transform = `translate3d(0, ${g.offset}px, 0)`
    }
    if (bar.shown) bar.node.classList.add('is-on')
  }

  function frame() {
    raf = 0
    const now = performance.now()

    // Указатель перешёл на другой элемент: чья это область
    if (hoverTarget !== undefined) {
      const el = hoverTarget === null ? null : scrollerOf(hoverTarget)
      hoverTarget = undefined
      if (el !== hoverEl) {
        // Ушли из области — её полоса гаснет сразу (кроме той, что тащат)
        for (const b of bars.values()) {
          if (b.shown && !b.drag && !b.over && (!el || (b.el !== el && !b.el.contains(el)))) hide(b)
        }
        hoverEl = el
      }
      if (el) show(bars.get(el) ?? create(el), now)
    }
    if (moveReveal) {
      moveReveal = false
      if (hoverEl?.isConnected && canScroll(hoverEl)) show(bars.get(hoverEl) ?? create(hoverEl), now)
    }

    for (const el of scrolled) {
      // Вложенная область сдвинулась вместе с внешней — переставить её полосу
      for (const b of bars.values()) if (b.shown && b.el !== el && el.contains(b.el)) dirty.add(b)
      const bar = bars.get(el)
      if (bar?.shown) {
        bar.lastActive = now
        moved.add(bar)
        continue
      }
      if (now - lastInput > INPUT_WINDOW && el !== hoverEl) continue
      if (!el.isConnected || !canScroll(el)) continue
      show(bar ?? create(el), now)
    }
    scrolled.clear()

    const measured: Measured[] = []
    for (const b of dirty) {
      if (b.el.isConnected) measured.push(read(b))
      moved.add(b)
    }
    dirty.clear()
    const tops: [Bar, number][] = []
    for (const b of moved) if (b.el.isConnected) tops.push([b, b.el.scrollTop])
    moved.clear()

    for (const m of measured) apply(m)
    for (const [b, top] of tops) place(b, top)
  }

  // ---------- Указатель и ввод ----------
  function onDown(bar: Bar, e: PointerEvent) {
    if (e.button !== 0) return
    e.preventDefault()
    e.stopPropagation()
    if (e.target === bar.thumb) {
      bar.drag = { y: e.clientY, top: bar.el.scrollTop, id: e.pointerId }
      bar.thumb.setPointerCapture(e.pointerId)
      bar.node.classList.add('is-drag')
      return
    }
    const delta = pageDelta(e.clientY - bar.top, bar.offset, bar.size, bar.clientHeight)
    if (delta) bar.el.scrollBy({ top: delta, behavior: reduced.matches ? 'auto' : 'smooth' })
  }
  function onDrag(bar: Bar, e: PointerEvent) {
    if (!bar.drag || e.pointerId !== bar.drag.id) return
    bar.el.scrollTop = dragScrollTop(bar.drag.top, e.clientY - bar.drag.y, bar, bar.size)
  }
  function endDrag(bar: Bar) {
    if (!bar.drag) return
    bar.drag = null
    bar.lastActive = performance.now()
    bar.node.classList.remove('is-drag')
  }
  // Колесо над самой полосой: узел лежит поверх области, а не внутри — прокрутку передаём ей
  function onWheel(bar: Bar, e: WheelEvent) {
    if (e.ctrlKey) return
    e.preventDefault()
    bar.el.scrollBy({ top: wheelPixels(e.deltaY, e.deltaMode, bar.clientHeight), left: wheelPixels(e.deltaX, e.deltaMode, bar.clientHeight) })
  }

  const onScroll = (e: Event) => {
    const el = e.target === doc ? rootEl() : e.target
    if (el instanceof HTMLElement) {
      scrolled.add(el)
      schedule()
    }
  }
  const onInput = () => {
    lastInput = performance.now()
  }
  const onOver = (e: PointerEvent) => {
    if (e.pointerType === 'touch') return
    hoverTarget = e.target
    schedule()
  }
  const onOut = (e: PointerEvent) => {
    // Указатель ушёл из окна
    if (e.pointerType === 'touch' || e.relatedTarget) return
    hoverTarget = null
    schedule()
  }
  const onMove = (e: PointerEvent) => {
    if (e.pointerType === 'touch' || !hoverEl) return
    const bar = bars.get(hoverEl)
    if (bar?.shown) bar.lastActive = performance.now()
    else if (!moveReveal) {
      moveReveal = true
      schedule()
    }
  }
  const onResize = () => {
    for (const b of bars.values()) if (b.shown) dirty.add(b)
    schedule()
  }

  const capture = { capture: true, passive: true } as const
  doc.addEventListener('scroll', onScroll, capture)
  doc.addEventListener('wheel', onInput, capture)
  doc.addEventListener('keydown', onInput, capture)
  doc.addEventListener('touchmove', onInput, capture)
  doc.addEventListener('pointerover', onOver, capture)
  doc.addEventListener('pointerout', onOut, capture)
  doc.addEventListener('pointermove', onMove, capture)
  window.addEventListener('resize', onResize, { passive: true })

  return () => {
    doc.removeEventListener('scroll', onScroll, capture)
    doc.removeEventListener('wheel', onInput, capture)
    doc.removeEventListener('keydown', onInput, capture)
    doc.removeEventListener('touchmove', onInput, capture)
    doc.removeEventListener('pointerover', onOver, capture)
    doc.removeEventListener('pointerout', onOut, capture)
    doc.removeEventListener('pointermove', onMove, capture)
    window.removeEventListener('resize', onResize)
    if (raf) cancelAnimationFrame(raf)
    for (const b of [...bars.values()]) destroy(b)
  }
}

export default defineNuxtPlugin(() => {
  const root = document.documentElement
  let stop: (() => void) | null = null
  function measure() {
    const probe = document.createElement('div')
    // Пробе своя системная полоса: правила main.css для классических полос её не касаются
    probe.dataset.scrollbarProbe = ''
    probe.style.cssText = 'position:absolute;top:-200px;left:0;width:100px;height:100px;overflow:scroll;visibility:hidden'
    document.body.appendChild(probe)
    const classic = probe.offsetWidth - probe.clientWidth > 0
    probe.remove()
    if (classic) {
      root.dataset.scrollbars = 'classic'
      stop ??= createOverlayScrollbars()
    }
    else {
      delete root.dataset.scrollbars
      stop?.()
      stop = null
    }
  }
  measure()
  // На Mac настройку меняют на ходу (или подключают мышь при «Автоматически») — окно получает фокус
  window.addEventListener('focus', measure)
})
