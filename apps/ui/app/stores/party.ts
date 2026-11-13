import { defineStore } from 'pinia'
import type { Party, PartyPatch, PartySettings } from '~/api/types'

// Режим вечеринки (сервер — crates/server/src/party.rs): сцена для телевизора или проектора на
// странице /party, QR «Слушать вместе» для гостей и настройки, которые меняют волну хозяина —
// «Только энергичное» и «Без повторов артистов». «Диджей» — переходы на устройстве, где играет звук.
// Живые изменения с других устройств — событие `party.changed`.
export const usePartyStore = defineStore('party', () => {
  const bridge = useBridge()
  const toast = useToast()
  const jam = useJamStore()
  const connect = useConnectStore()
  const t = useNuxtApp().$i18n.t

  const party = ref<Party | null>(null)
  const busy = ref(false)
  let started = false

  const active = computed(() => !!party.value?.active)
  const settings = computed<PartySettings>(() => party.value?.settings ?? { energetic: false, noArtistRepeat: true, dj: true })

  async function load() {
    try {
      party.value = await bridge.party.get()
    }
    catch {
      // Нет связи — оставляем показанное
    }
  }

  function init() {
    if (started || !import.meta.client) return
    started = true
    jam.init()
    bridge.live.on('party.changed', (e) => {
      const { type: _type, ...next } = e
      party.value = next
    })
    bridge.live.onReconnect(() => void load())
    void load()
  }

  /** «Диджей» — там, где играет звук, если оно это умеет */
  function applyDj() {
    if (settings.value.dj && connect.djAvailable && connect.transition !== 'dj') connect.transition = 'dj'
  }

  async function update(patch: PartyPatch) {
    if (busy.value) return
    busy.value = true
    try {
      party.value = await bridge.party.update(patch)
      if (patch.active) void jam.load()
      if (patch.active || patch.dj) applyDj()
    }
    catch {
      toast.error(t('party.failed'))
    }
    finally {
      busy.value = false
    }
  }

  const start = () => update({ active: true })
  const stop = () => update({ active: false })

  return { party, busy, active, settings, init, load, update, start, stop }
})
