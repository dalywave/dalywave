import { defineStore } from 'pinia'
import type { WaveActivity, WaveCharacter, WaveLanguage, WaveMood, WaveSettings } from '~/bridge/types'

/** Сочетание волны из фильтра фокуса iOS (apps/shell/src/focus.rs) */
interface FocusWave { id: string, name: string, mood?: WaveMood, activity?: WaveActivity, character?: WaveCharacter, language?: WaveLanguage }

const KEY = 'dw.focus'

function read(): { enabled: boolean, before: WaveSettings | null, applied: string | null } {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Record<string, unknown>
    return { enabled: raw.enabled === true, before: (raw.before as WaveSettings | null) ?? null, applied: typeof raw.applied === 'string' ? raw.applied : null }
  }
  catch {
    return { enabled: false, before: null, applied: null }
  }
}

/**
 * «Фокусирование» (iPhone, iPad): в настройках iOS у фокуса «Работа» выбирают сочетание волны DalyWave —
 * пока фокус включён, «Моя волна» идёт под него: играющая волна перестраивается, а не играющая
 * начнётся с ним. Фокус выключился — настройки волны возвращаются к прежним (если волна не играет:
 * идущую не дёргаем). Включается в настройках приложения, по умолчанию выключено; своё для устройства
 */
export const useFocusStore = defineStore('focus', () => {
  const bridge = useBridge()
  const player = usePlayerStore()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  const available = bridge.kind === 'tauri' && (bridge.platform === 'ios' || bridge.platform === 'ipados')
  const saved = import.meta.client ? read() : { enabled: false, before: null, applied: null }
  const enabled = ref(saved.enabled)
  /** Настройки волны до фокуса и какое сочетание уже применено */
  let before = saved.before
  let applied = saved.applied
  let started = false

  function save() {
    try {
      localStorage.setItem(KEY, JSON.stringify({ enabled: enabled.value, before, applied }))
    }
    catch {
      // Без хранилища выбор живёт до перезапуска
    }
  }

  function settingsOf(w: FocusWave): Partial<WaveSettings> {
    const s: Partial<WaveSettings> = {}
    if (w.mood) s.mood = w.mood
    if (w.activity) s.activity = w.activity
    if (w.character) s.character = w.character
    if (w.language) s.language = w.language
    return s
  }

  function apply(w: FocusWave | null) {
    if (!enabled.value) return
    if (w) {
      if (applied === w.id) return
      if (!applied) before = { ...player.wave }
      applied = w.id
      save()
      // Сбросить к «как обычно» и наложить сочетание: у сочетания заданы не все параметры
      player.setWave({ mood: 'any', activity: 'any', character: 'mix', language: 'any', ...settingsOf(w) })
      if (!player.waveActive) toast.show({ text: t('focus.applied', { name: w.name }), icon: 'ph:moon-stars' })
      return
    }
    if (!applied) return
    applied = null
    if (before && !player.waveActive) player.setWave(before)
    before = null
    save()
  }

  async function check() {
    if (!enabled.value) return
    const { invoke } = await import('@tauri-apps/api/core')
    apply(await invoke<FocusWave | null>('focus_wave').catch(() => null))
  }

  function setEnabled(on: boolean) {
    enabled.value = on
    if (!on) {
      applied = null
      before = null
    }
    save()
    if (on) void check()
  }

  function start() {
    if (started || !available || !import.meta.client) return
    started = true
    void import('@tauri-apps/api/event').then(event =>
      event.listen<FocusWave | null>('focus:wave', e => apply(e.payload)),
    ).catch(() => {})
    // Фокус могли включить, пока приложение спало: свериться при запуске и при каждом открытии
    void check()
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible') void check()
    })
  }

  return { available, enabled, setEnabled, start }
})
