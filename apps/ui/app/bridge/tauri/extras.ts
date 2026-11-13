import type { BeatHaptics, Platform, ScreensaverBridge } from '../types'

// «Почувствуй бит» (iPhone) и заставка для Mac — команды оболочки (apps/shell/src/beat.rs, screensaver.rs)
type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>

let core: Promise<Invoke> | null = null
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  core ??= import('@tauri-apps/api/core').then(c => c.invoke as Invoke)
  return (await core)<T>(cmd, args)
}

export function createBeat(platform: Platform): BeatHaptics | null {
  // Taptic Engine есть только у iPhone; есть ли он у этого — скажет оболочка (`supported`)
  if (platform !== 'ios') return null
  return {
    state: () => call('beat_state'),
    set: enabled => call('beat_set', { enabled }),
    check: trackId => call('beat_check', { trackId }),
  }
}

export function createScreensaver(platform: Platform): ScreensaverBridge | null {
  if (platform !== 'macos') return null
  return {
    state: () => call('screensaver_state'),
    install: () => call('screensaver_install'),
  }
}
