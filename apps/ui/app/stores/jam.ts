import { defineStore } from 'pinia'
import type { Jam, JamPatch } from '~/api/types'

// «Слушать вместе», как Jam у Spotify — сторона хозяина (сервер — crates/server/src/jam.rs).
// Сессия с QR-кодом и ссылкой: гости заходят без аккаунта, добавляют треки в очередь активного
// устройства (сразу или после одобрения), ставят ❤. Здесь — список гостей, их треки и настройки;
// живые изменения — `jam.changed` и `jam.reaction` по общему WebSocket.
export const useJamStore = defineStore('jam', () => {
  const bridge = useBridge()
  const toast = useToast()
  const ui = useUiStore()
  const t = useNuxtApp().$i18n.t

  const jam = ref<Jam | null>(null)
  const loading = ref(false)
  const busy = ref(false)
  let started = false

  const active = computed(() => !!jam.value)
  const pending = computed(() => jam.value?.requests.filter(r => r.status === 'pending') ?? [])
  const guests = computed(() => jam.value?.guests ?? [])
  /** Кто добавил трек: id трека → имя гостя */
  const addedBy = computed(() => {
    const map = new Map<string, string>()
    // Новые первыми: имя — у последнего добавления
    for (const r of jam.value?.requests ?? []) {
      if (r.status === 'added' && !map.has(r.track.id)) map.set(r.track.id, r.guestName)
    }
    return map
  })

  /** Подпись строки очереди: «добавил Миша» */
  function addedLabel(trackId: string): string | undefined {
    const name = addedBy.value.get(trackId)
    return name ? t('jam.addedBy', { name }) : undefined
  }

  async function load() {
    loading.value = true
    try {
      jam.value = await bridge.jam.get()
    }
    catch {
      // Нет связи — оставляем показанное
    }
    finally {
      loading.value = false
    }
  }

  /** Подписка на живые события и первая загрузка (оболочка зовёт при запуске) */
  function init() {
    if (started || !import.meta.client) return
    started = true
    bridge.live.on('jam.changed', (e) => {
      if (e.ended && jam.value?.id === e.jamId) jam.value = null
      else void load()
    })
    bridge.live.on('jam.reaction', (e) => {
      if (jam.value?.id !== e.jamId) return
      toast.show({ text: t('jam.reacted', { name: e.guestName }), icon: 'ph:heart-fill' })
    })
    bridge.live.onReconnect(() => void load())
    void load()
  }

  async function run(fn: () => Promise<Jam | undefined>, failed: string) {
    if (busy.value) return
    busy.value = true
    try {
      const next = await fn()
      if (next) jam.value = next
    }
    catch (e) {
      const code = (e as { code?: string }).code
      toast.error(code === 'nothing_playing' ? t('jam.nothingPlaying') : failed)
    }
    finally {
      busy.value = false
    }
  }

  /** «Слушать вместе»: открыть панель (и сессию, если её нет) */
  async function open() {
    init()
    ui.openPanel('jam')
    if (!jam.value) await run(() => bridge.jam.start(), t('jam.startFailed'))
  }

  const start = () => run(() => bridge.jam.start(), t('jam.startFailed'))
  const update = (patch: JamPatch) => run(() => bridge.jam.update(patch), t('jam.failed'))
  const approve = (id: string) => run(() => bridge.jam.approve(id), t('jam.failed'))
  const decline = (id: string) => run(() => bridge.jam.decline(id), t('jam.failed'))

  async function removeGuest(id: string) {
    await run(async () => {
      await bridge.jam.removeGuest(id)
      await load()
      return undefined
    }, t('jam.failed'))
  }

  async function end() {
    await run(async () => {
      await bridge.jam.end()
      jam.value = null
      return undefined
    }, t('jam.failed'))
  }

  return { jam, loading, busy, active, pending, guests, addedBy, addedLabel, init, load, open, start, update, approve, decline, removeGuest, end }
})
