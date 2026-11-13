import { defineStore } from 'pinia'
import type { ConnectDevice, ConnectKind, PlaybackState, RemoteCommand, ServerEvent, Track } from '~/api/types'
import type { PlaybackDevice, Platform } from '~/bridge/types'
import type { TransitionMode } from './player'

// Управление между устройствами, как Spotify Connect (сервер — crates/server/src/connect.rs).
// Каждая вкладка и приложение — устройство. Звук играет одно активное: оно владеет очередью и
// публикует снимки (при каждом изменении и раз в 5 с с позицией). Остальные — пульты: плеер
// показывает снимок (позицию досчитываем по времени сервера), действия уходят командами с
// подтверждением; изменение видно сразу и откатывается, если устройство не ответило.
// Активное пропало (15 с без связи) — «Продолжить здесь» с того же места.

const HEARTBEAT_MS = 5000
const ACK_TIMEOUT_MS = 5000
const PUBLISH_DEBOUNCE_MS = 120
/** Позиция ушла от ожидаемой больше чем на столько — была перемотка: снимок сразу */
const SEEK_JUMP_MS = 1500
const TICK_MS = 250

type ServerEventOf<T extends ServerEvent['type']> = Extract<ServerEvent, { type: T }>

function storage(kind: 'local' | 'session'): Storage | null {
  try {
    return kind === 'local' ? localStorage : sessionStorage
  }
  catch {
    return null
  }
}

function newId(prefix: string) {
  return `${prefix}-${crypto.randomUUID()}`
}

/** Приложение — постоянный id установки; вкладка — свой id на время её жизни (переживает перезагрузку) */
function initialId(app: boolean): string {
  const store = storage(app ? 'local' : 'session')
  const key = app ? 'dw.connect.device' : 'dw.connect.tab'
  let id = store?.getItem(key) ?? null
  if (!id) {
    id = newId(app ? 'app' : 'web')
    store?.setItem(key, id)
  }
  return id
}

function kindOf(platform: Platform, app: boolean): ConnectKind {
  if (!app) return 'web'
  if (platform === 'ios' || platform === 'android') return 'phone'
  if (platform === 'ipados') return 'tablet'
  return platform === 'macos' ? 'laptop' : 'desktop'
}

export const useConnectStore = defineStore('connect', () => {
  const bridge = useBridge()
  const player = usePlayerStore()
  const toast = useToast()
  const t = useNuxtApp().$i18n.t
  const app = bridge.kind === 'tauri'

  const started = ref(false)
  /** Сервер уже сказал, кто играет (первый ответ на hello) */
  const ready = ref(false)
  const deviceId = ref(import.meta.client ? initialId(app) : '')
  const list = ref<ConnectDevice[]>([])
  const activeId = ref<string | null>(null)
  /** Снимок, который показывает пульт (с оптимистичными правками) … */
  const shown = ref<PlaybackState | null>(null)
  /** … и последний подтверждённый сервером */
  let confirmed: PlaybackState | null = null
  const pendingCmds = new Map<string, { timer: ReturnType<typeof setTimeout>, success?: string }>()
  /** Время сервера минус время здесь, мс */
  let offset = 0
  let lastPublished: { at: number, pos: number, playing: boolean } | null = null
  let publishTimer: ReturnType<typeof setTimeout> | undefined
  /** Звук заиграл здесь, сервер ещё не подтвердил: прежнее «активно другое» уже не в счёт */
  let claiming = false

  const active = computed(() => list.value.find(d => d.id === activeId.value) ?? null)
  const isActive = computed(() => !!activeId.value && activeId.value === deviceId.value)
  /** Звук играет на другом устройстве, и оно на связи: здесь пульт */
  const remote = computed(() => !!activeId.value && !isActive.value && !!active.value?.online)
  /** Активное устройство пропало, а снимок остался: «Продолжить здесь» */
  const orphaned = computed(() => !!activeId.value && !isActive.value && !active.value?.online && !!shown.value?.current)
  const activeName = computed(() => active.value?.name ?? '')
  const activeKind = computed<ConnectKind>(() => active.value?.kind ?? 'web')

  const devices = computed<PlaybackDevice[]>(() => {
    const own = list.value.some(d => d.id === deviceId.value)
    const rows: PlaybackDevice[] = list.value.map(d => ({
      id: d.id,
      name: d.name,
      kind: d.kind,
      current: d.id === deviceId.value,
      online: d.online,
      lastSeenAt: d.lastSeenAt,
      active: d.id === activeId.value,
      volume: d.capabilities.volume,
      remote: !!d.capabilities.remote,
      platform: d.platform,
    }))
    // Связи с сервером ещё нет — само устройство в списке всё равно есть
    if (!own) rows.unshift({ id: deviceId.value, name: deviceName(), kind: kindOf(bridge.platform, app), current: true, online: true, lastSeenAt: null, active: isActive.value })
    return withoutStale(rows)
  })

  const now = () => Date.now() + offset

  /** Позиция снимка на сейчас: пока играет, время идёт */
  function positionOf(s: PlaybackState | null): number {
    if (!s) return 0
    const base = s.positionMs ?? 0
    if (!s.playing || s.pending || !s.at) return base
    const pos = base + Math.max(0, now() - Date.parse(s.at))
    return s.durationMs ? Math.min(s.durationMs, pos) : pos
  }

  function deviceName(): string {
    if (app) return t('connect.appName')
    const ua = navigator.userAgent
    const browser = /YaBrowser/.test(ua) ? 'Яндекс Браузер' : /Edg\//.test(ua) ? 'Edge' : /Firefox\//.test(ua) ? 'Firefox' : /Chrome\//.test(ua) ? 'Chrome' : /Safari\//.test(ua) ? 'Safari' : t('connect.browser')
    const os = bridge.platform === 'ios' ? 'iPhone' : bridge.platform === 'ipados' ? 'iPad' : bridge.platform === 'android' ? 'Android' : /Mac OS X/.test(ua) ? 'Mac' : /Windows/.test(ua) ? 'Windows' : 'Linux'
    return t('connect.webName', { browser, os })
  }

  // ---------- Связь ----------

  function sendHello() {
    bridge.live.send({
      type: 'connect.hello',
      deviceId: deviceId.value,
      name: deviceName(),
      kind: kindOf(bridge.platform, app),
      platform: bridge.platform,
      appVersion: null,
      // Громкость Safari на iPhone меняют только кнопки телефона; «Диджей» — движок приложения
      capabilities: { volume: !(bridge.kind === 'web' && (bridge.platform === 'ios' || bridge.platform === 'ipados')), dj: app },
    })
  }

  function heartbeat() {
    bridge.live.send({ type: 'connect.heartbeat' })
  }

  function claim() {
    activeId.value = deviceId.value
    claiming = true
    bridge.live.send({ type: 'connect.claim' })
  }

  // ---------- Активное устройство: снимки ----------

  function publish() {
    clearTimeout(publishTimer)
    publishTimer = undefined
    if (!isActive.value || player.mirrored) return
    const state = player.connectSnapshot()
    if (bridge.live.send({ type: 'connect.state', state })) lastPublished = { at: Date.now(), pos: state.positionMs ?? 0, playing: !!state.playing }
  }

  // Не откладываем уже назначенный снимок: позиция меняется каждые 250 мс, и отсрочка по каждому
  // изменению не дала бы снимку уйти вовсе
  function schedulePublish() {
    if (publishTimer) return
    publishTimer = setTimeout(publish, PUBLISH_DEBOUNCE_MS)
  }

  /** Что видно пульту, кроме позиции: смена — повод для снимка */
  const signature = computed(() => {
    if (!isActive.value || player.mirrored) return ''
    return JSON.stringify([
      player.current?.uid, player.playing, player.pending?.status, player.context?.id, player.context?.type,
      player.manual.map(e => e.uid), player.upcoming.slice(0, 100).map(e => e.uid), player.history.length,
      player.volume, player.muted, player.transition, player.shuffle, player.repeat, player.waveSessionId, Math.round(player.durationMs), player.peaks.length,
    ])
  })

  // ---------- Пульт: показ снимка ----------

  function mirror() {
    const s = shown.value
    if (s) player.connectMirror(s, positionOf(s))
  }

  /** Снимок с сервера. Пока команда ждёт ответа, он снят ещё до неё: показ остаётся оптимистичным,
   *  а снимок пригодится для отката. После ответа устройство пришлёт свежий */
  function adopt(state: PlaybackState) {
    confirmed = state
    if (pendingCmds.size) return
    shown.value = state
    mirror()
  }

  /** Оптимистично: пульт сразу показывает результат команды, ответ его подтвердит или откатит */
  function optimistic(cmd: RemoteCommand) {
    const s = shown.value
    if (!s) return
    const at = new Date(now()).toISOString()
    const freeze = { positionMs: Math.round(positionOf(s)), at }
    let next: PlaybackState | null = null
    switch (cmd.action) {
      case 'play': next = { ...s, ...freeze, playing: true }; break
      case 'pause': next = { ...s, ...freeze, playing: false }; break
      case 'toggle': next = { ...s, ...freeze, playing: !s.playing }; break
      case 'seek': next = { ...s, positionMs: cmd.positionMs, at }; break
      case 'setVolume': next = { ...s, volume: cmd.volume }; break
      case 'setMuted': next = { ...s, muted: cmd.muted }; break
      case 'setTransition': next = { ...s, transition: cmd.mode }; break
      case 'setShuffle': next = { ...s, shuffle: cmd.shuffle }; break
      case 'setRepeat': next = { ...s, repeat: cmd.mode }; break
    }
    if (next) {
      shown.value = next
      mirror()
    }
  }

  function fail(text: string) {
    toast.error(text)
    shown.value = confirmed
    mirror()
  }

  function track(id: string, success?: string) {
    const name = activeName.value
    const timer = setTimeout(() => {
      pendingCmds.delete(id)
      fail(t('connect.noAnswer', { name }))
    }, ACK_TIMEOUT_MS)
    pendingCmds.set(id, { timer, success })
  }

  /** Команда активному устройству. true — ушла (или показана ошибка), false — некому */
  function command(cmd: RemoteCommand): boolean {
    const to = activeId.value
    if (!to) return false
    const id = crypto.randomUUID()
    if (!bridge.live.send({ type: 'connect.command', id, to, command: cmd })) {
      toast.error(t('connect.noLink'))
      return true
    }
    optimistic(cmd)
    track(id)
    return true
  }

  // Ползунок громкости шлёт много значений: на устройство уходит не чаще раза в 120 мс и последнее
  let volumeTimer: ReturnType<typeof setTimeout> | undefined
  let volumeQueued: number | null = null
  function sendVolume(v: number) {
    volumeQueued = v
    if (shown.value) {
      shown.value = { ...shown.value, volume: v }
      mirror()
    }
    if (volumeTimer) return
    const flush = () => {
      volumeTimer = undefined
      if (volumeQueued == null) return
      const value = volumeQueued
      volumeQueued = null
      command({ action: 'setVolume', volume: value })
      volumeTimer = setTimeout(flush, 120)
    }
    flush()
  }

  // ---------- Переходы между ролями ----------

  /** Маршрут действий плеера, пока он показывает другое устройство */
  function route(cmd: RemoteCommand, local?: () => unknown): boolean {
    if (!player.mirrored) return false
    // «Играть» (с наушников, экрана блокировки), пока ждёт «Продолжить здесь?» — это «да»
    if ((cmd.action === 'play' || (cmd.action === 'toggle' && !shown.value?.playing)) && useHandoffStore().takePending()) return true
    if (remote.value) return command(cmd)
    // Активного нет в сети. Новое (альбом, волна) — просто играем здесь
    if (cmd.action === 'playTracks' || cmd.action === 'startWave') {
      leave()
      return false
    }
    if (cmd.action === 'pause') return true
    const play = cmd.action === 'play' || cmd.action === 'toggle'
    void continueHere(play ? true : undefined).then(() => {
      if (!play) local?.()
    })
    return true
  }

  function leave() {
    shown.value = null
    confirmed = null
    player.connectLeave()
  }

  /** Играть здесь: с того же места с той же очередью; прежнее активное остановится */
  async function continueHere(play?: boolean) {
    const s = shown.value
    const pos = positionOf(s)
    activeId.value = deviceId.value
    bridge.live.send({ type: 'connect.transfer', to: deviceId.value, play: play ?? null })
    shown.value = null
    confirmed = null
    await player.connectTakeOver(s, pos, play ?? !!s?.playing)
  }

  function transferTo(d: PlaybackDevice) {
    const id = crypto.randomUUID()
    if (!bridge.live.send({ type: 'connect.transfer', id, to: d.id })) {
      toast.error(t('connect.noLink'))
      return
    }
    // Здесь звук остановится, когда сервер подтвердит: тогда же придёт новое активное
    pendingCmds.set(id, {
      timer: setTimeout(() => {
        pendingCmds.delete(id)
        toast.error(t('connect.noAnswer', { name: d.name }))
      }, ACK_TIMEOUT_MS),
      success: t('connect.movedTo', { name: d.name }),
    })
  }

  /** Выбор в списке «Где играть» */
  function select(d: PlaybackDevice) {
    if (d.current) {
      if (player.mirrored) void continueHere()
      return
    }
    if (d.id === activeId.value && d.online) return
    // Часы — пульт: звук там не играет
    if (d.remote) return
    if (!d.online) {
      toast.show({ text: t('connect.offline', { name: d.name }), icon: 'ph:wifi-slash' })
      return
    }
    transferTo(d)
  }

  // ---------- События сервера ----------

  function onDevices(e: ServerEventOf<'connect.devices'>) {
    list.value = e.devices
    if (!claiming) activeId.value = e.activeDeviceId
  }

  function onActive(e: ServerEventOf<'connect.active'>) {
    // Первый ответ после открытия страницы, а здесь уже успели включить музыку: это свежий выбор
    // человека — заявляем о себе, а не уступаем прежнему активному
    const first = !ready.value
    ready.value = true
    if (first && e.deviceId && e.deviceId !== deviceId.value && !e.transfer && !player.mirrored && player.current) {
      claim()
      return
    }
    if (e.deviceId === deviceId.value) claiming = false
    // Ответ на hello пришёл позже, чем здесь включили музыку: наша заявка — следом, не уступаем
    else if (claiming && !e.transfer && !player.mirrored && player.playing) return
    claiming = false
    activeId.value = e.deviceId
    if (!e.deviceId) return
    if (e.deviceId === deviceId.value) {
      // Сюда перенесли воспроизведение — или после перезагрузки вкладка всё ещё активная: очередь
      // возвращается (на паузе, если это не перенос). Уже играем сами («Продолжить здесь») — не трогаем
      const s = e.state
      if (player.mirrored || (!player.current && s?.current)) {
        shown.value = null
        confirmed = null
        void player.connectTakeOver(s, positionOf(s), e.transfer ? !!s?.playing : false)
      }
      else schedulePublish()
      return
    }
    // Активно другое: звук здесь останавливается, плеер — пульт
    player.connectYield()
    if (e.state) adopt(e.state)
  }

  function onState(e: ServerEventOf<'connect.state'>) {
    if (e.deviceId === deviceId.value) return
    if (activeId.value !== e.deviceId) {
      activeId.value = e.deviceId
      player.connectYield()
    }
    adopt(e.state)
  }

  async function run(cmd: RemoteCommand) {
    switch (cmd.action) {
      case 'play': return player.play()
      case 'pause': return player.pause()
      case 'toggle': return player.toggle()
      case 'next': return player.next('skipped')
      case 'previous': return player.prev()
      case 'seek': return player.seek(cmd.positionMs)
      case 'setVolume': return player.setVolume(cmd.volume)
      case 'setMuted':
        player.muted = cmd.muted
        return
      case 'setTransition':
        player.transition = cmd.mode as TransitionMode
        return
      case 'setShuffle': return player.setShuffle(cmd.shuffle)
      case 'setRepeat': return player.setRepeat(cmd.mode)
      case 'dislike': return player.dislike()
      case 'notNow': return player.notNow()
      case 'moreLikeThis': return player.moreLikeThis()
      case 'playTracks': {
        const found = await bridge.catalog.lookup(cmd.trackIds)
        const byId = new Map(found.map(tr => [tr.id, tr]))
        const tracks = cmd.trackIds.map(id => byId.get(id)).filter((tr): tr is Track => !!tr)
        const ctx = cmd.context
        return player.playTracks(tracks, cmd.index ?? 0, { type: ctx.type, id: ctx.id ?? null, title: ctx.title }, { shuffle: !!cmd.shuffle })
      }
      case 'startWave': return player.startWave(cmd.settings ? { mood: cmd.settings.mood, activity: cmd.settings.activity, character: cmd.settings.character, language: cmd.settings.language } : undefined)
      case 'setWave':
        if (cmd.settings) player.setWave({ mood: cmd.settings.mood, activity: cmd.settings.activity, character: cmd.settings.character, language: cmd.settings.language })
        return
      case 'jumpTo': return player.jumpTo(cmd.uid)
      case 'remove': return player.remove(cmd.uid)
      case 'playNext': return player.playNext(cmd.track)
      case 'enqueue': return player.enqueue(cmd.track)
      case 'clearManual': return player.clearManual()
      case 'moveManual': return player.moveManual(cmd.from, cmd.to)
    }
  }

  async function onCommand(e: ServerEventOf<'connect.command'>) {
    const ack = (ok: boolean, error: string | null = null) => bridge.live.send({ type: 'connect.ack', id: e.id, to: e.from, ok, error })
    // Команда опоздала: звук уже не здесь
    if (!isActive.value || player.mirrored) return ack(false, 'inactive')
    try {
      await run(e.command)
      ack(true)
      schedulePublish()
    }
    catch {
      ack(false, 'failed')
    }
  }

  function onAck(e: ServerEventOf<'connect.ack'>) {
    const p = pendingCmds.get(e.id)
    if (!p) return
    clearTimeout(p.timer)
    pendingCmds.delete(e.id)
    if (!e.ok) {
      const name = list.value.find(d => d.id === e.from)?.name ?? activeName.value
      fail(e.error === 'offline' ? t('connect.offline', { name }) : t('connect.failed', { name }))
      return
    }
    if (p.success) toast.show({ text: p.success, icon: 'ph:speaker-hifi' })
  }

  // ---------- Запуск ----------

  function start() {
    if (started.value || !import.meta.client) return
    started.value = true
    player.setConnectRoute(route)

    // Вкладку продублировали — у копии тот же id из sessionStorage: она берёт новый
    if (!app && 'BroadcastChannel' in window) {
      const nonce = crypto.randomUUID()
      const ch = new BroadcastChannel('dw.connect')
      ch.onmessage = (m: MessageEvent<{ type: string, id: string, nonce: string }>) => {
        const d = m.data
        if (d?.type === 'claim' && d.id === deviceId.value && d.nonce !== nonce) ch.postMessage({ type: 'taken', id: d.id, nonce: d.nonce })
        else if (d?.type === 'taken' && d.nonce === nonce && d.id === deviceId.value) {
          deviceId.value = newId('web')
          storage('session')?.setItem('dw.connect.tab', deviceId.value)
          sendHello()
        }
      }
      ch.postMessage({ type: 'claim', id: deviceId.value, nonce })
    }

    bridge.live.on('hello', (e) => {
      offset = Date.parse(e.serverTime) - Date.now()
      sendHello()
      // Пока связи не было, здесь начали играть — заявляем о себе
      if (!player.mirrored && player.playing) claim()
    })
    bridge.live.on('connect.devices', onDevices)
    bridge.live.on('connect.active', onActive)
    bridge.live.on('connect.state', onState)
    bridge.live.on('connect.command', e => void onCommand(e))
    bridge.live.on('connect.ack', onAck)

    // Звук заиграл здесь — это устройство становится активным, прежнее остановится
    watch(() => player.playing && !player.mirrored, (here) => {
      if (here && activeId.value !== deviceId.value) claim()
    })
    watch(isActive, (on) => {
      if (on) schedulePublish()
    })
    watch(signature, (sig) => {
      if (sig) schedulePublish()
    })
    // Перемотка: позиция ушла от ожидаемой — снимок сразу, не ждём пяти секунд
    watch(() => player.positionMs, (pos) => {
      if (!isActive.value || player.mirrored || !lastPublished) return
      const expected = lastPublished.playing ? lastPublished.pos + (Date.now() - lastPublished.at) : lastPublished.pos
      if (Math.abs(pos - expected) > SEEK_JUMP_MS) schedulePublish()
    })
    watch(devices, d => (player.devices = d), { immediate: true })
    // Активное пропало: звук там уже не идёт для нас — пульт показывает паузу на последней позиции
    watch(orphaned, (lost) => {
      const s = shown.value
      if (!lost || !s?.playing) return
      shown.value = { ...s, playing: false, positionMs: Math.round(positionOf(s)), at: new Date(now()).toISOString() }
      mirror()
    })

    setInterval(() => {
      heartbeat()
      if (isActive.value) publish()
    }, HEARTBEAT_MS)
    // Пульт: позиция активного идёт сама
    setInterval(() => {
      const s = shown.value
      if (player.mirrored && s?.playing && !s.pending) player.positionMs = positionOf(s)
    }, TICK_MS)
    // Вернулись на вкладку (или приложение iPhone из фона): сразу на связи. Сокет переподключает мост
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState !== 'visible') return
      heartbeat()
      if (isActive.value) publish()
    })
  }

  function refresh() {
    heartbeat()
  }

  // ---------- Для экранов пульта ----------

  /** Громкость: на пульте — активного устройства */
  const volume = computed({
    get: () => (player.mirrored && shown.value ? shown.value.volume ?? 1 : player.volume),
    set: (v: number) => {
      if (remote.value && player.mirrored) sendVolume(v)
      else player.setVolume(v)
    },
  })

  const muted = computed({
    get: () => (player.mirrored && shown.value ? !!shown.value.muted : player.muted),
    set: (m: boolean) => {
      if (remote.value && player.mirrored) command({ action: 'setMuted', muted: m })
      else player.muted = m
    },
  })

  const transition = computed({
    get: () => (player.mirrored && shown.value?.transition ? (shown.value.transition as TransitionMode) : player.transition),
    set: (mode: TransitionMode) => {
      if (remote.value && player.mirrored) command({ action: 'setTransition', mode })
      else player.transition = mode
    },
  })

  /** «Диджей» есть у устройства, где играет звук (движок приложения) */
  const djAvailable = computed(() => (remote.value ? !!active.value?.capabilities.dj : app))

  /** Можно ли менять громкость активного устройства отсюда */
  const volumeControl = computed(() => !remote.value || active.value?.capabilities.volume !== false)

  return {
    started, ready, deviceId, activeId, active, isActive, remote, orphaned, activeName, activeKind, devices, shown,
    volume, muted, transition, volumeControl, djAvailable,
    start, refresh, select, continueHere, transferTo,
  }
})
