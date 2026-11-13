import type { Track } from '~/api/types'
import type { HookOrigin } from '~/stores/hookPreview'

/** Палец держится столько — «послушать припев» */
const TOUCH_MS = 420
/** Кнопка ▶ под мышью удерживается столько */
const MOUSE_MS = 380
/** Палец ушёл дальше — это прокрутка, а не нажатие */
const SLOP_PX = 10

/**
 * «Послушать припев» по удержанию: долгое нажатие пальцем на строку или карточку (телефон, iPad) или
 * удержание кнопки ▶ мышью (компьютер). Короткое нажатие работает как обычно. Пальцем долгое
 * нажатие здесь — отрывок, а не меню: меню трека — кнопкой «⋯»
 */
export function useHookPress(track: () => Track | null, origin?: () => HookOrigin | null) {
  const hook = useHookPreviewStore()
  let timer = 0
  let x = 0
  let y = 0
  /** Отрывок включён этим нажатием: следующий щелчок не должен включить трек */
  let fired = false
  let touch = false

  function clear() {
    if (timer) window.clearTimeout(timer)
    timer = 0
  }

  /** `mouse` — элемент, который мышью удерживают (кнопка ▶); остальное — только пальцем */
  function down(e: PointerEvent, mouse = false) {
    if (e.button !== 0) return
    touch = e.pointerType !== 'mouse'
    if (!touch && !mouse) return
    // Пальцем: долгое нажатие не должно открыть меню строки (его ведёт область выше)
    if (touch) e.stopPropagation()
    fired = false
    x = e.clientX
    y = e.clientY
    clear()
    timer = window.setTimeout(() => {
      timer = 0
      const tr = track()
      if (!tr) return
      fired = true
      void hook.start(tr, origin?.() ?? null)
    }, touch ? TOUCH_MS : MOUSE_MS)
  }

  function move(e: PointerEvent) {
    if (timer && Math.hypot(e.clientX - x, e.clientY - y) > SLOP_PX) clear()
  }

  function up() {
    clear()
  }

  /** Щелчок после удержания — не включение */
  function click(e: MouseEvent) {
    if (!fired) return
    fired = false
    e.preventDefault()
    e.stopImmediatePropagation()
  }

  /** Android присылает contextmenu на долгое нажатие, iOS — показывает выноску ссылки */
  function context(e: Event) {
    if (!touch || (!timer && !fired)) return
    e.preventDefault()
    e.stopPropagation()
  }

  /** Было ли только что удержание (и забыть о нём) */
  function consume() {
    const was = fired
    fired = false
    return was
  }

  onBeforeUnmount(clear)

  return { down, move, up, click, context, consume }
}
