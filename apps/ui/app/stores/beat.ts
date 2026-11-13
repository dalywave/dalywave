import { defineStore } from 'pinia'
import type { BeatHaptics } from '~/bridge/types'
import type { MenuEntry } from '~/types/ui'

const KEY = 'dw.beat'

// «Почувствуй бит» (iPhone): выключатель в меню «⋯» полного плеера. Вибрацию по сетке долей делает
// оболочка (apps/shell/src/beat.rs) — и только пока приложение на экране. Выбор хранится на устройстве
/** Только разработка: пункт видно на сайте — `?beat=1` (есть сетка) или `?beat=nogrid` (снимки экранов) */
function devBeat(): BeatHaptics | null {
  if (!import.meta.dev || typeof location === 'undefined') return null
  const q = new URLSearchParams(location.search).get('beat')
  if (!q) return null
  let enabled = false
  return {
    state: async () => ({ supported: true, enabled }),
    set: async v => ({ supported: true, enabled: (enabled = v) }),
    check: async () => q !== 'nogrid',
  }
}

export const useBeatStore = defineStore('beat', () => {
  const bridge = { beat: useBridge().beat ?? devBeat() }
  const player = usePlayerStore()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t
  const supported = ref(false)
  const enabled = ref(false)
  /** Есть ли сетка долей: по id трека */
  const grids = ref(new Map<string, boolean>())
  let started = false

  function remembered(): boolean {
    try {
      return localStorage.getItem(KEY) === '1'
    }
    catch {
      return false
    }
  }

  async function init() {
    if (started || !bridge.beat) return
    started = true
    try {
      const s = remembered() ? await bridge.beat.set(true) : await bridge.beat.state()
      supported.value = s.supported
      enabled.value = s.enabled
    }
    catch {
      supported.value = false
    }
  }

  const trackId = computed(() => player.current?.track.id ?? null)
  const hasGrid = computed(() => (trackId.value ? grids.value.get(trackId.value) ?? null : null))

  // Сетку текущего трека узнаём заранее, чтобы пункт меню сразу был доступен или нет
  watch(trackId, (id) => {
    if (!id || !bridge.beat || !supported.value || grids.value.has(id)) return
    void bridge.beat.check(id).then(ok => grids.value.set(id, ok)).catch(() => {})
  })

  async function toggle() {
    if (!bridge.beat) return
    const next = !enabled.value
    enabled.value = next
    try {
      enabled.value = (await bridge.beat.set(next)).enabled
      localStorage.setItem(KEY, enabled.value ? '1' : '0')
    }
    catch {
      enabled.value = !next
    }
    if (enabled.value) toast.show({ text: t('beat.on'), icon: 'ph:vibrate' })
  }

  /** Пункт меню «⋯» плеера; без Taptic Engine — ничего */
  function menuEntries(): MenuEntry[] {
    if (!bridge.beat || !supported.value) return []
    if (trackId.value && !grids.value.has(trackId.value)) void bridge.beat.check(trackId.value).then(ok => grids.value.set(trackId.value!, ok)).catch(() => {})
    const noGrid = hasGrid.value === false
    return [{
      label: t('beat.label'),
      icon: 'ph:vibrate',
      checked: enabled.value,
      disabled: noGrid && !enabled.value,
      hint: noGrid ? t('beat.noGrid') : undefined,
      onSelect: () => void toggle(),
    }]
  }

  return { supported, enabled, hasGrid, init, toggle, menuEntries }
})
