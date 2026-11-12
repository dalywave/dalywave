import { defineStore } from 'pinia'
import type { SleepState } from '~/bridge/types'
import { EQ_BANDS_HZ, EQ_MAX_DB, FLAT_GAINS, type EqConfig, type EqPresetId } from '~/utils/eq'

/** Последние секунды таймера сна звук затухает — как в движке приложений */
const SLEEP_FADE_MS = 15_000
/** Сколько ползунок «Своего» ждёт, прежде чем отдать полосы движку (приложение пишет их на диск) */
const EQ_SEND_MS = 80

export const SLEEP_MINUTES = [5, 10, 15, 30, 45, 60] as const

export type SleepMode = 'minutes' | 'track'

interface SleepLocal {
  mode: SleepMode
  /** Когда пауза (мс от эпохи); у конца трека — null, остаток считается по позиции */
  endsAt: number | null
  fading: boolean
  /** Сайт, «в конце трека»: трек, конец которого ждём */
  uid: string | null
  /** Что выбрали в меню: минуты или конец трека (у таймера из движка после запуска — неизвестно) */
  choice: number | 'track' | null
}

// Звук: эквалайзер этого устройства и таймер сна. Приложение — всё в движке (оболочка помнит
// эквалайзер, таймер работает и в фоне); сайт — эквалайзер в WebAudio, таймер ведётся здесь
export const useSoundStore = defineStore('sound', () => {
  const bridge = useBridge()
  const engine = bridge.audio
  const native = engine.native
  const player = usePlayerStore()

  // ---------- Эквалайзер ----------

  const stored = useLocalStorage<EqConfig>('dw.eq', { preset: 'off', gains: FLAT_GAINS() }, { mergeDefaults: true })
  const eq = ref<EqConfig>(normalized(stored.value))
  const eqSupported = engine.eqSupported

  function normalized(c: EqConfig): EqConfig {
    const gains = EQ_BANDS_HZ.map((_, i) => Math.max(-EQ_MAX_DB, Math.min(EQ_MAX_DB, Number(c.gains?.[i]) || 0)))
    return { preset: c.preset ?? 'off', gains }
  }

  let sendTimer = 0
  function send(now = false) {
    clearTimeout(sendTimer)
    const go = () => {
      stored.value = { preset: eq.value.preset, gains: [...eq.value.gains] }
      engine.setEq({ preset: eq.value.preset, gains: [...eq.value.gains] })
    }
    if (now) go()
    else sendTimer = window.setTimeout(go, EQ_SEND_MS)
  }

  function setPreset(id: EqPresetId) {
    eq.value = { ...eq.value, preset: id }
    send(true)
  }

  function setGain(band: number, db: number) {
    const gains = [...eq.value.gains]
    gains[band] = Math.round(Math.max(-EQ_MAX_DB, Math.min(EQ_MAX_DB, db)) * 2) / 2
    eq.value = { preset: 'custom', gains }
    send()
  }

  function resetCustom() {
    eq.value = { preset: 'custom', gains: FLAT_GAINS() }
    send(true)
  }

  // ---------- Таймер сна ----------

  const sleep = ref<SleepLocal | null>(null)
  const now = ref(Date.now())
  let clock = 0

  const sleepActive = computed(() => !!sleep.value)
  /** Сколько осталось, мс; у конца трека — по позиции */
  const sleepRemainingMs = computed<number | null>(() => {
    const s = sleep.value
    if (!s) return null
    if (s.mode === 'minutes') return s.endsAt ? Math.max(0, s.endsAt - now.value) : null
    const d = player.durationMs || player.current?.track.durationMs || 0
    return d ? Math.max(0, d - player.positionMs) : null
  })

  function runClock(on: boolean) {
    if (on && !clock) {
      clock = window.setInterval(() => {
        now.value = Date.now()
        if (!native) webTick()
      }, 250)
    }
    else if (!on && clock) {
      clearInterval(clock)
      clock = 0
    }
  }

  function setSleep(timer: number | 'track' | null) {
    if (timer === null) {
      if (!native && sleep.value?.fading) engine.fadeOut(null)
      sleep.value = null
      player.stopAtTrackEnd = false
      if (native) engine.setSleep(null)
      runClock(false)
      return
    }
    if (!native && sleep.value?.fading) engine.fadeOut(null)
    sleep.value = timer === 'track'
      ? { mode: 'track', endsAt: null, fading: false, uid: player.current?.uid ?? null, choice: 'track' }
      : { mode: 'minutes', endsAt: Date.now() + timer * 60_000, fading: false, uid: null, choice: timer }
    now.value = Date.now()
    if (native) engine.setSleep(timer === 'track' ? { mode: 'track' } : { mode: 'minutes', ms: timer * 60_000 })
    else player.stopAtTrackEnd = timer === 'track'
    runClock(true)
  }

  /** Приложение: состояние таймера из движка */
  function fromEngine(s: SleepState) {
    if (!s.timer) {
      sleep.value = null
      runClock(false)
      return
    }
    const choice = sleep.value?.mode === s.timer ? sleep.value.choice : s.timer === 'track' ? 'track' : null
    sleep.value = { mode: s.timer, endsAt: s.timer === 'minutes' ? s.endsAtMs : null, fading: s.fading, uid: null, choice }
    runClock(true)
  }

  /** Сайт: затухание за 15 с до срока (или до конца трека), затем пауза */
  function webTick() {
    const s = sleep.value
    if (!s) return
    if (s.mode === 'minutes') {
      const left = (s.endsAt ?? 0) - Date.now()
      if (left <= 0) return finishWeb()
      if (!s.fading && player.playing && left <= SLEEP_FADE_MS) {
        engine.fadeOut(left)
        s.fading = true
      }
      return
    }
    // Человек сам перешёл к другому треку — ждём уже его конца
    if (player.current?.uid !== s.uid) {
      if (s.fading) engine.fadeOut(null)
      s.uid = player.current?.uid ?? null
      s.fading = false
    }
    const left = sleepRemainingMs.value
    if (left === null) return
    if (s.fading && left > SLEEP_FADE_MS + 500) {
      // Перемотали назад — затухание заново, ближе к концу
      engine.fadeOut(null)
      s.fading = false
    }
    else if (!s.fading && player.playing && left > 0 && left <= SLEEP_FADE_MS) {
      engine.fadeOut(left)
      s.fading = true
    }
  }

  function finishWeb() {
    sleep.value = null
    player.stopAtTrackEnd = false
    runClock(false)
    player.pause()
    // Громкость обратно, когда звук уже на паузе
    window.setTimeout(() => engine.fadeOut(null), 400)
  }

  if (import.meta.client) {
    if (native) {
      // Эквалайзер помнит оболочка: он включён с запуска движка, интерфейс только показывает
      void engine.savedEq().then((saved) => {
        if (saved) {
          eq.value = normalized(saved)
          stored.value = { ...eq.value }
        }
      })
      void engine.sleepState().then(s => s && fromEngine(s))
      engine.on('sleep', fromEngine)
    }
    else {
      engine.setEq(eq.value)
      // Сайт: «в конце трека» сработал — плеер встал на паузу у начала следующего
      watch(() => player.stopAtTrackEnd, (on, was) => {
        if (was && !on && sleep.value?.mode === 'track') {
          sleep.value = null
          runClock(false)
          window.setTimeout(() => engine.fadeOut(null), 400)
        }
      })
      // Пауза посреди затухания: оно расписано по часам WebAudio и дошло бы до тишины без звука —
      // громкость обратно, затухание начнётся снова при игре
      watch(() => player.playing, (on) => {
        if (!on && sleep.value?.fading) {
          engine.fadeOut(null)
          sleep.value.fading = false
        }
      })
    }
  }

  return { eq, eqSupported, setPreset, setGain, resetCustom, sleep, sleepActive, sleepRemainingMs, setSleep }
})
