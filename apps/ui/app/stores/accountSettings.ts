import { defineStore } from 'pinia'
import type { Track, UserSettings, UserSettingsPatch } from '~/api/types'

const STORAGE_KEY = 'dw.accountSettings'
const DEFAULTS: UserSettings = {
  hideExplicit: false,
  qualityWifi: 'high',
  qualityCellular: 'normal',
  qualityDownload: 'high',
  playlistSort: 'recent',
  handoff: 'suggest',
  updatedAt: null,
}

function cached(): UserSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? { ...DEFAULTS, ...JSON.parse(raw) as Partial<UserSettings> } : { ...DEFAULTS }
  }
  catch {
    return { ...DEFAULTS }
  }
}

function remember(s: UserSettings) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s))
  }
  catch {
    // Без хранилища настройки придут с сервера при следующем запуске
  }
}

/**
 * Настройки слушателя, общие для всех его устройств (на сервере): «Без нецензурного», качество звука,
 * порядок плейлистов. Правка применяется сразу и откатывается при ошибке; правки с других устройств
 * приходят живым событием. Копия в localStorage — чтобы первая отрисовка и приложение без сети
 * знали фильтр сразу
 */
export const useAccountSettings = defineStore('accountSettings', () => {
  const bridge = useBridge()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t
  const settings = ref<UserSettings>(import.meta.client ? cached() : { ...DEFAULTS })
  const loaded = ref(false)

  async function load() {
    try {
      settings.value = await bridge.settings.get()
      loaded.value = true
      remember(settings.value)
    }
    catch {
      // Без сети — последние известные
    }
  }

  async function update(patch: UserSettingsPatch) {
    const was = settings.value
    const clean = Object.fromEntries(Object.entries(patch).filter(([, v]) => v != null)) as Partial<UserSettings>
    settings.value = { ...was, ...clean }
    try {
      settings.value = await bridge.settings.update(patch)
      remember(settings.value)
    }
    catch {
      settings.value = was
      toast.error(t('settingsSync.failed'))
    }
  }

  const hideExplicit = computed(() => settings.value.hideExplicit)

  /** Трек скрыт фильтром: explicit, а чистой версии в каталоге нет */
  function hides(track: Pick<Track, 'explicit' | 'cleanId'>): boolean {
    return hideExplicit.value && track.explicit && !track.cleanId
  }

  /**
   * Очередь без нецензурного: explicit с чистой версией заменяется ею, без неё — выпадает.
   * `null` — нажатый трек скрыт фильтром (и показано «Скрыто фильтром»)
   */
  async function clean(tracks: Track[], index: number, opts: { strict?: boolean } = {}): Promise<{ tracks: Track[], index: number } | null> {
    if (!hideExplicit.value || !tracks.some(tr => tr.explicit)) return { tracks, index }
    const want = [...new Set(tracks.filter(tr => tr.explicit && tr.cleanId).map(tr => tr.cleanId!))]
    const found = want.length ? await bridge.catalog.lookup(want).catch(() => [] as Track[]) : []
    const byId = new Map(found.map(tr => [tr.id, tr]))
    const out: Track[] = []
    let at = -1
    tracks.forEach((tr, i) => {
      const use = !tr.explicit ? tr : tr.cleanId ? byId.get(tr.cleanId) : undefined
      if (i === index) at = use ? out.length : -1
      if (use) out.push(use)
    })
    if (at < 0 && opts.strict !== false) {
      toast.show({ text: t('settingsSync.hidden'), icon: 'ph:eye-slash' })
      return null
    }
    if (!out.length) {
      toast.show({ text: t('settingsSync.hidden'), icon: 'ph:eye-slash' })
      return null
    }
    return { tracks: out, index: Math.max(0, at) }
  }

  if (import.meta.client) {
    bridge.live.on('library.changed', (e) => {
      if (e.entities.includes('settings')) void load()
    })
    bridge.live.onReconnect(() => void load())
  }

  return { settings, loaded, hideExplicit, load, update, hides, clean }
})
