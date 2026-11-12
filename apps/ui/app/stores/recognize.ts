import { defineStore } from 'pinia'
import type { Recognition } from '~/api/types'
import type { RecognizeErrorCode } from '~/bridge/types'

/** Экран распознавания: слушаем, нашли и ищем трек, не узнали, сбой */
export type RecognizePhase = 'idle' | 'listening' | 'resolving' | 'nomatch' | 'error'
export type RecognizeFailure = Exclude<RecognizeErrorCode, 'no_match' | 'cancelled'> | 'network'

// «Распознать»: оболочка слушает микрофон (ShazamKit), сервер находит трек каталога и пишет его в
// «Распознанное». Кнопка — в поиске, в трее macOS и в быстром действии на значке iPhone
export const useRecognizeStore = defineStore('recognize', () => {
  const bridge = useBridge()
  const available = computed(() => !!bridge.recognizer)

  const phase = ref<RecognizePhase>('idle')
  const failure = ref<RecognizeFailure | null>(null)
  /** Что узнал Shazam, пока сервер ищет трек */
  const heard = ref<{ title: string, artist: string } | null>(null)

  /** Лист результата: найденное сейчас или выбранное в истории */
  const result = shallowRef<Recognition | null>(null)
  const sheetOpen = ref(false)

  const history = shallowRef<Recognition[]>([])
  const historyLoaded = ref(false)

  // Отменили посреди поиска трека — ответ сервера уже не нужен
  let seq = 0

  async function start() {
    const rec = bridge.recognizer
    if (!rec || phase.value === 'listening' || phase.value === 'resolving') return
    const my = ++seq
    sheetOpen.value = false
    failure.value = null
    heard.value = null
    phase.value = 'listening'
    bridge.haptic('medium')
    let match
    try {
      match = await rec.listen()
    }
    catch (e) {
      if (my !== seq) return
      const code = (e as { code?: string } | null)?.code
      if (code === 'cancelled') {
        phase.value = 'idle'
        return
      }
      if (code === 'no_match') phase.value = 'nomatch'
      else {
        failure.value = code === 'permission' || code === 'no_microphone' || code === 'shazam' || code === 'unsupported' ? code : 'shazam'
        phase.value = 'error'
      }
      bridge.haptic('warning')
      return
    }
    if (my !== seq) return
    heard.value = { title: match.title, artist: match.artist }
    phase.value = 'resolving'
    bridge.haptic('success')
    try {
      const found = await bridge.recognize.resolve(match)
      if (my !== seq) return
      history.value = [found, ...history.value.filter(r => r.id !== found.id)]
      show(found)
      phase.value = 'idle'
    }
    catch {
      if (my !== seq) return
      failure.value = 'network'
      phase.value = 'error'
    }
  }

  /** «Отменить» и «Закрыть»: микрофон выключается, музыка (если играла) продолжается */
  function cancel() {
    seq++
    if (phase.value === 'listening') void bridge.recognizer?.cancel().catch(() => {})
    phase.value = 'idle'
  }

  function show(r: Recognition) {
    result.value = r
    sheetOpen.value = true
  }

  async function loadHistory(force = false) {
    if (historyLoaded.value && !force) return
    const page = await bridge.recognize.history(null, 20)
    history.value = page.items
    historyLoaded.value = true
  }

  async function remove(id: string) {
    const before = history.value
    history.value = before.filter(r => r.id !== id)
    try {
      await bridge.recognize.remove(id)
    }
    catch (e) {
      history.value = before
      throw e
    }
  }

  return { available, phase, failure, heard, result, sheetOpen, history, historyLoaded, start, cancel, show, loadHistory, remove }
})
