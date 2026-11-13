import { defineStore } from 'pinia'
import type { ConnectHandoff, ConnectOutput, ConnectSignalKind } from '~/api/types'
import type { AudioOutput, AudioOutputs } from '~/bridge/types'

// «Подхват на другом устройстве» (сервер — crates/server/src/handoff.rs). Приложение сообщает серверу,
// что видит само: пропали или появились наушники, экран компьютера заблокирован, приложение открыли.
// Сервер сводит сигналы всех устройств и присылает `connect.handoff`: музыка уже перешла (режим «Сразу»)
// или предложение «Продолжить здесь?». Вкладка сайта своих наушников не знает — только приложения.
//
// Наушники ушли с компьютера, пока играла музыка, — пауза, как в Apple Music: звук не должен вдруг
// пойти из динамиков (iPhone ставит паузу сам, движок — по правилам Apple).

/** Сколько ждёт предложение «Продолжить здесь?» */
const SUGGEST_MS = 60_000
/** Музыка играла здесь — если перестала только что (движок уже поставил паузу из-за наушников) */
const PLAYED_RECENTLY_MS = 4000

type Payload = { signal: ConnectSignalKind, output?: ConnectOutput | null, playing?: boolean }

const personal = (o: AudioOutput | null | undefined): o is AudioOutput => !!o && (o.kind === 'headphones' || o.kind === 'bluetooth')
const same = (a: AudioOutput, b: AudioOutput) => a.id === b.id && a.name === b.name

export const useHandoffStore = defineStore('handoff', () => {
  const bridge = useBridge()
  const player = usePlayerStore()
  const connect = useConnectStore()
  const account = useAccountSettings()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t

  const app = bridge.kind === 'tauri'
  const mobile = bridge.platform === 'ios' || bridge.platform === 'ipados'
  const enabled = computed(() => account.settings.handoff !== 'off')

  let started = false
  let prev: AudioOutputs | null = null
  let lastPlayingAt = 0
  /** Предложение «Продолжить здесь?»: до какого времени ждёт и его тост */
  let pending: { until: number, toast: number } | null = null

  function send(p: Payload) {
    if (!enabled.value) return
    bridge.live.send({ type: 'connect.signal', signal: p.signal, output: p.output ?? null, playing: !!p.playing })
  }

  const asOutput = (o: AudioOutput): ConnectOutput => ({ name: o.name, kind: o.kind })
  const currentOf = (s: AudioOutputs | null) => s?.outputs.find(o => o.current) ?? null
  const playingHere = () => connect.isActive && !player.mirrored && (player.playing || Date.now() - lastPlayingAt < PLAYED_RECENTLY_MS)

  /** Список выходов сменился: что пропало и что появилось */
  function onOutputs(next: AudioOutputs) {
    const before = prev
    prev = next
    if (!before) return
    const was = currentOf(before)
    const now = currentOf(next)
    if (personal(was) && !next.outputs.some(o => same(o, was))) {
      const playing = playingHere()
      send({ signal: 'output_lost', output: asOutput(was), playing })
      // Компьютер: поток сам переехал на динамики — останавливаем, пока никто не услышал
      if (!mobile && enabled.value && playing && player.playing) void player.pause()
    }
    if (personal(now) && !before.outputs.some(o => same(o, now))) send({ signal: 'output_added', output: asOutput(now) })
  }

  /** Приложение открыли: с какими наушниками */
  async function foreground() {
    if (!bridge.outputs) return
    const s = await bridge.outputs.get().catch(() => null)
    if (s) prev = s
    const now = currentOf(s)
    send({ signal: 'foreground', output: personal(now) ? asOutput(now) : null })
  }

  function nameOf(id: string) {
    return connect.devices.find(d => d.id === id)?.name ?? ''
  }

  function onHandoff(e: ConnectHandoff) {
    const here = connect.deviceId
    if (e.transferred) {
      if (e.to === here) toast.show({ text: e.output ? t('handoff.movedHereWith', { output: e.output }) : t('handoff.movedHere'), icon: 'ph:headphones' })
      else if (e.from === here) toast.show({ text: t('handoff.movedTo', { name: nameOf(e.to) }), icon: 'ph:speaker-hifi' })
      return
    }
    if (e.to !== here) return
    if (pending) toast.dismiss(pending.toast)
    const text = e.reason === 'headphones' && e.output
      ? t('handoff.suggestHeadphones', { output: e.output })
      : t('handoff.suggestAway', { name: nameOf(e.from) })
    const id = toast.show({
      text,
      icon: e.reason === 'headphones' ? 'ph:headphones' : 'ph:arrows-left-right',
      duration: SUGGEST_MS,
      action: { label: t('handoff.continue'), onClick: () => accept() },
    })
    pending = { until: Date.now() + SUGGEST_MS, toast: id }
  }

  function accept() {
    if (pending) toast.dismiss(pending.toast)
    pending = null
    void connect.continueHere(true)
  }

  /** «Играть» с наушников или экрана блокировки, пока ждёт предложение: это «да» */
  function takePending(): boolean {
    if (!pending || Date.now() > pending.until) return false
    accept()
    return true
  }

  function start() {
    if (started || !import.meta.client) return
    started = true
    bridge.live.on('connect.handoff', onHandoff)
    if (!app) return
    watch(() => player.playing && !player.mirrored, (on, was) => {
      if (on || was) lastPlayingAt = Date.now()
    })
    if (bridge.outputs) {
      void bridge.outputs.get().then((s) => {
        prev ??= s
      }).catch(() => {})
      bridge.outputs.onChange(onOutputs)
    }
    if (mobile) {
      // Открыли приложение (и при запуске — тоже открытие), когда связь уже есть
      bridge.live.on('hello', () => {
        if (document.visibilityState === 'visible') setTimeout(() => void foreground(), 500)
      })
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') void foreground()
      })
      return
    }
    // Компьютер: экран заблокирован — человек отошёл (оболочка, presence.rs)
    void Promise.all([import('@tauri-apps/api/event')]).then(([event]) =>
      event.listen<{ locked: boolean }>('system:lock', e => send({ signal: e.payload.locked ? 'locked' : 'unlocked' })),
    ).catch(() => {})
  }

  return { start, takePending }
})
