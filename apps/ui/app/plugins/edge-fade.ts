import type { Directive } from 'vue'

// v-edge-fade — горизонтальная лента гаснет к краю, за которым есть ещё карточки:
// обрезанная карточка читается как «листайте дальше», а не как сломанная вёрстка.
// Директива только ставит классы edge-fade / can-left / can-right; сама маска — .edge-fade в main.css.
// v-edge-fade="false" — выключить (например, когда лента на этом размере не прокручивается)
type FadeEl = HTMLElement & { __edgeFade?: () => void, __edgeOn?: boolean }

const CLASSES = ['edge-fade', 'can-left', 'can-right']

function update(el: FadeEl) {
  if (!el.__edgeOn) {
    el.classList.remove(...CLASSES)
    return
  }
  // Vue перезаписывает className при смене :class — ставим свои классы заново
  el.classList.add('edge-fade')
  const max = el.scrollWidth - el.clientWidth
  el.classList.toggle('can-left', max > 2 && el.scrollLeft > 2)
  el.classList.toggle('can-right', max > 2 && el.scrollLeft < max - 2)
}

const edgeFade: Directive<FadeEl, boolean | undefined> = {
  mounted(el, binding) {
    el.__edgeOn = binding.value !== false
    let raf = 0
    const onScroll = () => {
      if (raf) return
      raf = requestAnimationFrame(() => {
        raf = 0
        update(el)
      })
    }
    const ro = new ResizeObserver(onScroll)
    ro.observe(el)
    // Карточки приходят позже ленты: следим и за содержимым
    const mo = new MutationObserver(onScroll)
    mo.observe(el, { childList: true })
    el.addEventListener('scroll', onScroll, { passive: true })
    update(el)
    el.__edgeFade = () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      mo.disconnect()
      el.removeEventListener('scroll', onScroll)
    }
  },
  updated(el, binding) {
    el.__edgeOn = binding.value !== false
    update(el)
  },
  unmounted(el) {
    el.__edgeFade?.()
  },
}

export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.vueApp.directive('edge-fade', edgeFade)
})

declare module 'vue' {
  interface GlobalDirectives {
    vEdgeFade: typeof edgeFade
  }
}
