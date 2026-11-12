import { defineStore } from 'pinia'
import type { Track } from '~/api/types'

export interface ConfirmRequest {
  title: string
  text?: string
  action: string
  danger?: boolean
}

// Общие окна работы с плейлистами: выбор плейлиста для треков, подтверждение, удаляемое прямо
// сейчас (скрыто, пока у «Отменить» не кончилось время) и треки, которые тянут мышью
export const usePlaylistUi = defineStore('playlistUi', () => {
  /** Треки, которые добавляют в плейлист; null — окно выбора закрыто */
  const pickTracks = ref<Track[] | null>(null)
  const pickOpen = ref(false)

  const confirmReq = ref<ConfirmRequest | null>(null)
  const confirmOpen = ref(false)
  let resolveConfirm: ((ok: boolean) => void) | null = null

  /** Удалённые, но ещё не стёртые на сервере плейлисты: списки их не показывают */
  const hidden = ref(new Set<string>())

  /** Треки, которые сейчас тянут на плейлист в боковой панели */
  const dragTracks = ref<Track[] | null>(null)

  function pick(tracks: Track[]) {
    if (!tracks.length) return
    pickTracks.value = tracks
    pickOpen.value = true
  }

  function confirm(req: ConfirmRequest): Promise<boolean> {
    resolveConfirm?.(false)
    confirmReq.value = req
    confirmOpen.value = true
    return new Promise((resolve) => {
      resolveConfirm = resolve
    })
  }

  function answer(ok: boolean) {
    confirmOpen.value = false
    resolveConfirm?.(ok)
    resolveConfirm = null
  }

  watch(confirmOpen, (open) => {
    if (!open) answer(false)
  })

  return { pickTracks, pickOpen, confirmReq, confirmOpen, hidden, dragTracks, pick, confirm, answer }
})
