import { defineStore } from 'pinia'
import type { PlaylistSummary, Track } from '~/api/types'

export interface ConfirmRequest {
  title: string
  text?: string
  action: string
  danger?: boolean
}

/** Короткий ввод названия (папка): заголовок, начальное значение, кнопка */
export interface PromptRequest {
  title: string
  value?: string
  placeholder?: string
  action: string
  maxLength?: number
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

  const promptReq = ref<PromptRequest | null>(null)
  const promptOpen = ref(false)
  let resolvePrompt: ((value: string | null) => void) | null = null

  /** Удалённые, но ещё не стёртые на сервере плейлисты: списки их не показывают */
  const hidden = ref(new Set<string>())

  /** Треки, которые сейчас тянут на плейлист в боковой панели */
  const dragTracks = ref<Track[] | null>(null)

  /** «Собрать как сет»: для какого плейлиста открыт лист */
  const setFor = ref<PlaylistSummary | null>(null)
  const setOpen = ref(false)
  /** Порядок плейлиста поменялся сборкой или «Отменить» — открытая страница перечитывает его */
  const reordered = ref<{ id: string, at: number } | null>(null)

  /** «Совместный плейлист»: участники, позвать друзей, ссылка — для какого плейлиста открыт лист */
  const peopleFor = ref<PlaylistSummary | null>(null)
  const peopleOpen = ref(false)
  /** Участники или «совместный» поменялись — открытая страница плейлиста перечитывает его */
  const peopleChanged = ref(0)

  function openPeople(p: PlaylistSummary) {
    peopleFor.value = p
    peopleOpen.value = true
  }

  function openSet(p: PlaylistSummary) {
    setFor.value = p
    setOpen.value = true
  }

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

  /** Спросить название; `null` — передумали */
  function prompt(req: PromptRequest): Promise<string | null> {
    resolvePrompt?.(null)
    promptReq.value = req
    promptOpen.value = true
    return new Promise((resolve) => {
      resolvePrompt = resolve
    })
  }

  function submitPrompt(value: string | null) {
    promptOpen.value = false
    resolvePrompt?.(value)
    resolvePrompt = null
  }

  watch(promptOpen, (open) => {
    if (!open) submitPrompt(null)
  })

  return { pickTracks, pickOpen, confirmReq, confirmOpen, promptReq, promptOpen, hidden, dragTracks, setFor, setOpen, reordered, peopleFor, peopleOpen, peopleChanged, pick, confirm, answer, prompt, submitPrompt, openSet, openPeople }
})
