import { defineStore } from 'pinia'
import type { NewWavePreset, WavePreset, WavePresetPatch } from '~/api/types'

// Сохранённые сочетания настроек волны. Последний список с сервера лежит и в браузере: ряд под волной
// рисуется сразу и без сети. Правки — сразу на экране, при ошибке откатываются
export const useWavePresetsStore = defineStore('wavePresets', () => {
  const bridge = useBridge()
  const items = useLocalStorage<WavePreset[]>('dw.wavePresets', [])
  const loaded = ref(false)
  let loading: Promise<void> | null = null

  function load(force = false): Promise<void> {
    if (loaded.value && !force) return Promise.resolve()
    loading ??= bridge.wave.presets.list()
      .then((list) => {
        items.value = list
        loaded.value = true
      })
      .finally(() => (loading = null))
    return loading
  }

  async function create(body: NewWavePreset): Promise<WavePreset> {
    const p = await bridge.wave.presets.create(body)
    items.value = [...items.value.filter(x => x.id !== p.id), p]
    return p
  }

  async function update(id: string, patch: WavePresetPatch): Promise<WavePreset> {
    const before = items.value
    items.value = items.value.map(p => (p.id === id ? { ...p, ...patch, settings: patch.settings ?? p.settings } as WavePreset : p))
    try {
      const p = await bridge.wave.presets.update(id, patch)
      items.value = items.value.map(x => (x.id === id ? p : x))
      return p
    }
    catch (e) {
      items.value = before
      throw e
    }
  }

  /** Удалить; вернёт «Отменить»: сочетание создаётся заново на прежнем месте */
  async function remove(id: string): Promise<() => Promise<void>> {
    const before = items.value
    const gone = before.find(p => p.id === id)
    items.value = before.filter(p => p.id !== id)
    try {
      await bridge.wave.presets.remove(id)
    }
    catch (e) {
      items.value = before
      throw e
    }
    const order = before.map(p => p.id)
    return async () => {
      if (!gone) return
      const back = await create({ name: gone.name, icon: gone.icon, settings: gone.settings })
      items.value = await bridge.wave.presets.reorder(order.map(x => (x === id ? back.id : x)))
    }
  }

  if (import.meta.client) {
    // Сочетание сохранили или удалили на другом устройстве
    bridge.live.on('library.changed', (e) => {
      if (e.entities.includes('wavePresets')) void load(true).catch(() => {})
    })
    bridge.live.onReconnect(() => void load(true).catch(() => {}))
  }

  return { items, loaded, load, create, update, remove }
})
