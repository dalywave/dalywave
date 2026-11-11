import { createTauriBridge, isTauri } from './tauri'
import type { Bridge } from './types'
import { createWebBridge } from './web'

export type * from './types'

let bridge: Bridge | null = null

/** Один мост на приложение: в Tauri — приложения, иначе — сайта */
export function getBridge(): Bridge {
  bridge ??= isTauri() ? createTauriBridge() : createWebBridge()
  return bridge
}
