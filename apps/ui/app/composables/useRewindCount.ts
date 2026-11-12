/**
 * Счётчик историй «Итогов года»: число досчитывает от нуля до значения, когда слайд появляется.
 * При уменьшенном движении — сразу итог. `start(to)` запускает заново (новый слайд)
 */
export function useRewindCount(ms = 1500, delay = 350) {
  const value = ref(0)
  let raf = 0
  const reduced = import.meta.client && window.matchMedia('(prefers-reduced-motion: reduce)').matches

  function start(to: number) {
    cancelAnimationFrame(raf)
    if (reduced) {
      value.value = to
      return
    }
    value.value = 0
    const t0 = performance.now() + delay
    const step = (now: number) => {
      const k = Math.min(1, Math.max(0, (now - t0) / ms))
      value.value = Math.round(to * (k >= 1 ? 1 : 1 - 2 ** (-10 * k)))
      if (k < 1) raf = requestAnimationFrame(step)
    }
    raf = requestAnimationFrame(step)
  }

  onBeforeUnmount(() => cancelAnimationFrame(raf))
  return { value, start }
}
