// Кто сейчас показывает волну. Холст один; если открыт плеер поверх главной, волна у плеера,
// а когда плеер закрывается — возвращается на главную
import { getWaveEngine } from './engine'
import type { WaveLayout } from './engine'

interface Host {
  el: HTMLElement
  layout: WaveLayout
  box: HTMLElement | null
  /** Тёмная волна при любой теме */
  dark?: boolean
}

const stack: Host[] = []

function mountTop() {
  const top = stack[stack.length - 1]
  if (top) getWaveEngine().mount(top.el, { layout: top.layout, box: top.box, dark: top.dark })
}

export function pushWaveHost(host: Host) {
  const i = stack.findIndex(h => h.el === host.el)
  if (i >= 0) stack.splice(i, 1)
  stack.push(host)
  mountTop()
}

export function updateWaveHost(el: HTMLElement, patch: Partial<Omit<Host, 'el'>>) {
  const host = stack.find(h => h.el === el)
  if (!host) return
  Object.assign(host, patch)
  if (stack[stack.length - 1] === host) mountTop()
}

export function popWaveHost(el: HTMLElement) {
  const i = stack.findIndex(h => h.el === el)
  if (i < 0) return
  const wasTop = i === stack.length - 1
  stack.splice(i, 1)
  if (!wasTop) return
  getWaveEngine().unmount(el)
  mountTop()
}
