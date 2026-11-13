import { defineStore } from 'pinia'
import type { ServerConfig } from '~/api/types'

// Что включено на сервере: каталог Deezer, загрузчик, своя музыка. У тех, кто ставит DalyWave себе,
// по умолчанию только своя папка — интерфейс прячет то, чего на сервере нет. Ответ запоминается: без
// сети приложение показывает то же, что в прошлый раз
const CACHE_KEY = 'dw.server.config'

function cached(): ServerConfig | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY)
    return raw ? JSON.parse(raw) as ServerConfig : null
  }
  catch {
    return null
  }
}

export const useServerStore = defineStore('server', () => {
  const bridge = useBridge()
  const config = ref<ServerConfig | null>(import.meta.client ? cached() : null)
  let loading: Promise<void> | null = null
  // Спрашиваем раз за запуск: настройки сервера меняются только его перезапуском
  let fresh = false

  // Пока сервер не ответил ни разу — всё включено (как на wave.daly.rest)
  const deezer = computed(() => config.value?.deezer ?? true)
  const downloader = computed(() => config.value?.downloader ?? true)
  const library = computed(() => config.value?.library ?? false)

  function load(): Promise<void> {
    if (fresh) return Promise.resolve()
    loading ??= bridge.serverConfig()
      .then((next) => {
        fresh = true
        config.value = next
        try {
          localStorage.setItem(CACHE_KEY, JSON.stringify(next))
        }
        catch {
          // Без хранилища спросим при следующем запуске
        }
      })
      .catch(() => {
        // Нет связи — остаётся запомненное
      })
      .finally(() => {
        loading = null
      })
    return loading
  }

  return { config, deezer, downloader, library, load }
})
