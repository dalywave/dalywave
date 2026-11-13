// Управление воспроизведением: кнопки, громкость, «нравится», переходы, таймер сна, «Сейчас играет»
// и крутилка Multimedia.
import type { DialAction } from '@elgato/streamdeck'
import type { JsonObject } from '@elgato/utils'
import { account } from '../account'
import { t } from '../i18n'
import { player } from '../player'
import { pulse } from '../pulse'
import { type GlyphName } from '../render/glyphs.gen'
import { coverKey, iconKey } from '../render/keys'
import { LAYOUT, neoFields, pulseFields, trackFields, volumeFields } from '../render/strip'
import { api } from '../api'
import { trackSubtitle } from '../catalog'
import type { TransitionMode } from '../types'
import { DeckAction, type Instance, type Outcome } from './base'

/** Обложка текущего трека (data URI), пока грузится — null */
const coverCache = { path: '', uri: null as string | null }

export function currentCover(): string | null {
  const path = player.track?.album?.image?.small ?? ''
  if (path !== coverCache.path) {
    coverCache.path = path
    coverCache.uri = null
    if (path) {
      void api.cover(path).then((uri) => {
        if (coverCache.path === path) {
          coverCache.uri = uri
          player.changed()
        }
      })
    }
  }
  return coverCache.uri
}

// ---------- Играть / пауза / следующий / предыдущий ----------

type PlaybackMode = 'toggle' | 'play' | 'pause' | 'next' | 'previous'
interface PlaybackSettings { [k: string]: string | undefined, mode?: PlaybackMode }

const PLAYBACK_GLYPH: Record<Exclude<PlaybackMode, 'toggle'>, GlyphName> = {
  play: 'play-fill', pause: 'pause-fill', next: 'skip-forward-fill', previous: 'skip-back-fill',
}

export class PlaybackAction extends DeckAction<PlaybackSettings> {
  override readonly manifestId = 'rest.daly.wave.playback'

  protected override renderKey(inst: Instance<PlaybackSettings>): void {
    const mode = inst.settings.mode ?? 'toggle'
    const g = mode === 'toggle' ? (player.playing ? 'pause-fill' : 'play-fill') : PLAYBACK_GLYPH[mode]
    this.setImage(inst, iconKey({ glyph: g }))
  }

  protected override press(inst: Instance<PlaybackSettings>): Promise<boolean> {
    switch (inst.settings.mode ?? 'toggle') {
      case 'play': return player.play()
      case 'pause': return player.pause()
      case 'next': return player.next()
      case 'previous': return player.previous()
      default: return player.toggle()
    }
  }
}

// ---------- Перемешивание ----------

interface ShuffleSettings { [k: string]: string | undefined, mode?: 'toggle' | 'on' | 'off' }

export class ShuffleAction extends DeckAction<ShuffleSettings> {
  override readonly manifestId = 'rest.daly.wave.shuffle'

  protected override renderKey(inst: Instance<ShuffleSettings>): void {
    this.setImage(inst, iconKey({ glyph: 'shuffle-bold', tone: player.shuffle ? 'on' : 'off' }))
  }

  protected override press(inst: Instance<ShuffleSettings>): Promise<boolean> {
    const mode = inst.settings.mode ?? 'toggle'
    return player.setShuffle(mode === 'toggle' ? !player.shuffle : mode === 'on')
  }
}

// ---------- Повтор ----------

export class RepeatAction extends DeckAction {
  override readonly manifestId = 'rest.daly.wave.repeat'

  protected override renderKey(inst: Instance<JsonObject>): void {
    const r = player.repeat
    this.setImage(inst, iconKey({ glyph: r === 'one' ? 'repeat-once-bold' : 'repeat-bold', tone: r === 'off' ? 'off' : 'on' }))
  }

  protected override press(): Promise<boolean> {
    return player.cycleRepeat()
  }
}

// ---------- Громкость (кнопка и крутилка) ----------

interface VolumeSettings { [k: string]: string | number | undefined, mode?: 'up' | 'down' | 'mute', step?: number | string }

function stepOf(s: VolumeSettings, fallback: number): number {
  const n = Number(s.step)
  return Number.isFinite(n) && n > 0 ? Math.min(50, n) / 100 : fallback / 100
}

function levelLabel(): string {
  return player.muted ? t().muted : `${Math.round(player.volume * 100)} %`
}

function trackLine(): string {
  const tr = player.track
  return tr ? `${tr.title} — ${trackSubtitle(tr)}` : t().nothing
}

export class VolumeAction extends DeckAction<VolumeSettings> {
  override readonly manifestId = 'rest.daly.wave.volume'

  protected override renderKey(inst: Instance<VolumeSettings>): void {
    const mode = inst.settings.mode ?? 'up'
    const g: GlyphName = mode === 'mute' ? (player.muted ? 'speaker-x-bold' : 'speaker-high-bold') : mode === 'up' ? 'speaker-high-bold' : 'speaker-low-bold'
    const supported = player.target?.capabilities.volume !== false
    this.setImage(inst, iconKey({ glyph: g, label: supported ? levelLabel() : '', tone: mode === 'mute' && player.muted ? 'off' : 'normal' }))
  }

  protected override renderDial(inst: Instance<VolumeSettings>): void {
    this.setLayout(inst, LAYOUT.volume)
    this.feedback(inst, volumeFields({ level: player.volume, muted: player.muted, label: levelLabel(), track: trackLine() }))
  }

  protected override press(inst: Instance<VolumeSettings>): Promise<boolean> {
    const mode = inst.settings.mode ?? 'up'
    if (mode === 'mute') return player.setMuted(!player.muted)
    const step = stepOf(inst.settings, 10)
    return player.adjustVolume(mode === 'up' ? step : -step)
  }

  private pending: NodeJS.Timeout | null = null

  protected override rotate(inst: Instance<VolumeSettings>, ticks: number): void {
    // Тики копятся ~80 мс: одна команда на рывок крутилки, а не десять
    const step = stepOf(inst.settings, 2)
    const target = Math.min(1, Math.max(0, (Number(inst.data.target ?? player.volume)) + ticks * step))
    inst.data.target = target
    if (this.pending) clearTimeout(this.pending)
    this.pending = setTimeout(() => {
      this.pending = null
      delete inst.data.target
      void player.setVolume(target)
    }, 80)
    this.feedback(inst, volumeFields({ level: target, muted: false, label: `${Math.round(target * 100)} %`, track: trackLine() }))
  }

  protected override dialPress(): Promise<boolean> {
    return player.toggle()
  }

  protected override touch(_inst: Instance<VolumeSettings>, hold: boolean): Promise<boolean> {
    return hold ? player.setMuted(!player.muted) : player.toggle()
  }
}

// ---------- Нравится / не нравится / не сейчас ----------

export class LikeAction extends DeckAction {
  override readonly manifestId = 'rest.daly.wave.like'

  protected override renderKey(inst: Instance<JsonObject>): void {
    const liked = player.liked()
    this.setImage(inst, iconKey(liked ? { glyph: 'heart-fill', tone: 'accent' } : { glyph: 'heart-bold', tone: player.track ? 'normal' : 'off' }))
  }

  protected override press(): Promise<boolean> {
    return player.toggleLiked()
  }
}

export class DislikeAction extends DeckAction {
  override readonly manifestId = 'rest.daly.wave.dislike'

  protected override renderKey(inst: Instance<JsonObject>): void {
    this.setImage(inst, iconKey({ glyph: 'thumbs-down-bold', tone: player.track ? 'normal' : 'off' }))
  }

  protected override press(): Promise<boolean> {
    return player.dislike()
  }
}

export class NotNowAction extends DeckAction {
  override readonly manifestId = 'rest.daly.wave.notnow'

  protected override renderKey(inst: Instance<JsonObject>): void {
    this.setImage(inst, iconKey({ glyph: 'arrow-bend-up-right-bold', tone: player.track ? 'normal' : 'off' }))
  }

  protected override press(): Promise<boolean> {
    return player.notNow()
  }
}

// ---------- Режим переходов ----------

const TRANSITION_GLYPH: Record<TransitionMode, GlyphName> = {
  gapless: 'link-simple-horizontal-bold',
  smooth: 'wave-sine-bold',
  smart: 'sparkle-bold',
  dj: 'vinyl-record-bold',
}

export class TransitionAction extends DeckAction {
  override readonly manifestId = 'rest.daly.wave.transition'

  protected override renderKey(inst: Instance<JsonObject>): void {
    const m = player.transition
    this.setImage(inst, iconKey({ glyph: TRANSITION_GLYPH[m], label: t().transition[m] }))
  }

  protected override press(): Promise<boolean> {
    return player.cycleTransition()
  }
}

// ---------- Таймер сна ----------

interface SleepSettings { [k: string]: string | number | undefined, minutes?: number | string, mode?: 'toggle' | 'restart' | 'extend' }

function minutesOf(s: SleepSettings): number {
  const n = Number(s.minutes)
  return Number.isFinite(n) && n > 0 ? Math.min(600, n) : 30
}

function formatLeft(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000))
  const h = Math.floor(total / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}` : `${m}:${String(s).padStart(2, '0')}`
}

/** Один таймер на все кнопки: по его концу — пауза на активном устройстве */
class SleepTimer {
  private timer: NodeJS.Timeout | null = null

  get until(): number | null {
    const u = account.settings.sleepUntil
    return typeof u === 'number' && u > Date.now() ? u : null
  }

  get total(): number {
    return Number(account.settings.sleepTotal) || 1
  }

  /** После перезапуска плагина таймер продолжает идти */
  restore(): void {
    this.arm()
  }

  async start(minutes: number): Promise<void> {
    const ms = minutes * 60_000
    await account.save({ sleepUntil: Date.now() + ms, sleepTotal: ms })
    this.arm()
  }

  async extend(minutes: number): Promise<void> {
    const until = this.until
    if (!until) return this.start(minutes)
    const ms = minutes * 60_000
    await account.save({ sleepUntil: until + ms, sleepTotal: until + ms - Date.now() })
    this.arm()
  }

  async stop(): Promise<void> {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    await account.save({ sleepUntil: null, sleepTotal: null })
    player.changed()
  }

  private arm(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    const until = this.until
    if (!until) return
    // setTimeout дольше ~24 дней не умеет; таймер сна — часы
    this.timer = setTimeout(() => void this.fire(), Math.min(until - Date.now(), 2 ** 31 - 1))
    player.changed()
  }

  private async fire(): Promise<void> {
    this.timer = null
    await player.pause()
    await this.stop()
  }
}

export const sleepTimer = new SleepTimer()

export class SleepAction extends DeckAction<SleepSettings> {
  override readonly manifestId = 'rest.daly.wave.sleep'
  protected override ticking = true

  protected override renderKey(inst: Instance<SleepSettings>): void {
    const until = sleepTimer.until
    if (!until) {
      this.setImage(inst, iconKey({ glyph: 'moon-stars-bold', label: `${minutesOf(inst.settings)} ${t().min}` }))
      return
    }
    const left = until - Date.now()
    this.setImage(inst, iconKey({ glyph: 'moon-stars-bold', big: formatLeft(left), ring: Math.max(0, left / sleepTimer.total) }))
  }

  protected override async press(inst: Instance<SleepSettings>): Promise<void> {
    const minutes = minutesOf(inst.settings)
    switch (inst.settings.mode ?? 'toggle') {
      case 'restart': return sleepTimer.start(minutes)
      case 'extend': return sleepTimer.extend(minutes)
      default: return sleepTimer.until ? sleepTimer.stop() : sleepTimer.start(minutes)
    }
  }

  /** Долгое нажатие — продлить на тот же срок */
  protected override async longPress(inst: Instance<SleepSettings>): Promise<void> {
    return sleepTimer.extend(minutesOf(inst.settings))
  }
}

// ---------- Сейчас играет (кнопка и экран Neo) ----------

interface NowSettings { [k: string]: boolean | string | undefined, pulse?: boolean | string }

function pulseOn(s: { pulse?: boolean | string }): boolean {
  return s.pulse !== false && s.pulse !== 'false'
}

export class NowPlayingAction extends DeckAction<NowSettings> {
  override readonly manifestId = 'rest.daly.wave.nowplaying'
  protected override ticking = true
  protected override needsDevice = false
  private flash = false

  constructor() {
    super()
    pulse.on('beat', (on) => {
      this.flash = on
      for (const inst of this.instances.values()) {
        if (pulseOn(inst.settings) && inst.action.isKey()) this.renderOne(inst.action.id)
      }
    })
  }

  protected override appeared(inst: Instance<NowSettings>): void {
    inst.data.pulse = pulseOn(inst.settings) && inst.action.isKey()
    if (inst.data.pulse) pulse.use(true)
  }

  protected override settingsChanged(inst: Instance<NowSettings>): void {
    const want = pulseOn(inst.settings) && inst.action.isKey()
    if (want !== inst.data.pulse) pulse.use(want)
    inst.data.pulse = want
  }

  override onWillDisappear(ev: Parameters<DeckAction<NowSettings>['onWillDisappear']>[0]): void {
    const inst = this.instances.get(ev.action.id)
    if (inst?.data.pulse) pulse.use(false)
    super.onWillDisappear(ev)
  }

  protected override renderKey(inst: Instance<NowSettings>): void {
    const tr = player.track
    const shade = inst.data.pulse && pulse.active && !this.flash ? 0.14 : 0
    this.setImage(inst, coverKey({ cover: tr ? currentCover() : null, progress: player.progress(), playing: player.playing || !tr, shade }))
  }

  protected override renderNeo(inst: Instance<NowSettings>): void {
    this.setLayout(inst, LAYOUT.neo)
    const tr = player.track
    this.feedback(inst, neoFields({ cover: tr ? currentCover() : null, title: tr?.title ?? t().nothing, subtitle: tr ? trackSubtitle(tr) : '' }))
  }

  protected override press(): Promise<boolean> {
    return player.toggle()
  }

  protected override longPress(): Promise<boolean> {
    return player.toggleLiked()
  }
}

// ---------- Multimedia (крутилка) ----------

type MediaMode = 'tracks' | 'seek' | 'volume'
interface MediaSettings { [k: string]: string | boolean | undefined, mode?: MediaMode, pulse?: boolean | string }
const MEDIA_MODES: MediaMode[] = ['tracks', 'seek', 'volume']

export class MultimediaAction extends DeckAction<MediaSettings> {
  override readonly manifestId = 'rest.daly.wave.multimedia'
  protected override ticking = true
  private flash = false

  constructor() {
    super()
    pulse.on('beat', (on) => {
      this.flash = on
      for (const inst of this.instances.values()) {
        if (inst.data.pulse && this.modeOf(inst) !== 'volume' && !this.problem()) this.feedback(inst, pulseFields(on && pulse.active ? 0.1 : 0))
      }
    })
  }

  private modeOf(inst: Instance<MediaSettings>): MediaMode {
    return (inst.data.mode as MediaMode | undefined) ?? inst.settings.mode ?? 'tracks'
  }

  protected override appeared(inst: Instance<MediaSettings>): void {
    inst.data.pulse = pulseOn(inst.settings)
    if (inst.data.pulse) pulse.use(true)
  }

  protected override settingsChanged(inst: Instance<MediaSettings>): void {
    delete inst.data.mode
    const want = pulseOn(inst.settings)
    if (want !== inst.data.pulse) pulse.use(want)
    inst.data.pulse = want
  }

  override onWillDisappear(ev: Parameters<DeckAction<MediaSettings>['onWillDisappear']>[0]): void {
    const inst = this.instances.get(ev.action.id)
    if (inst?.data.pulse) pulse.use(false)
    super.onWillDisappear(ev)
  }

  protected override renderDial(inst: Instance<MediaSettings> & { action: DialAction<MediaSettings> }): void {
    const mode = this.modeOf(inst)
    if (mode === 'volume') {
      this.setLayout(inst, LAYOUT.volume)
      const level = Number(inst.data.target ?? player.volume)
      this.feedback(inst, volumeFields({ level, muted: player.muted && inst.data.target === undefined, label: inst.data.target !== undefined ? `${Math.round(level * 100)} %` : levelLabel(), track: trackLine() }))
      return
    }
    this.setLayout(inst, LAYOUT.track)
    const tr = player.track
    this.feedback(inst, trackFields({
      cover: tr ? currentCover() : null,
      title: tr?.title ?? t().nothing,
      subtitle: tr ? trackSubtitle(tr) : '',
      progress: player.progress(),
      playing: player.playing || !tr,
    }))
    if (!pulse.active || !inst.data.pulse) this.feedback(inst, pulseFields(0))
    const rotate = mode === 'seek' ? t().modeSeek : t().modeTracks
    if (inst.data.trigger !== rotate) {
      inst.data.trigger = rotate
      void inst.action.setTriggerDescription({ rotate }).catch(() => {})
    }
  }

  private pending: NodeJS.Timeout | null = null
  private seekAcc = 0

  protected override rotate(inst: Instance<MediaSettings>, ticks: number, pressed: boolean): void {
    const mode = this.modeOf(inst)
    if (mode === 'volume') {
      const target = Math.min(1, Math.max(0, Number(inst.data.target ?? player.volume) + ticks * 0.02))
      inst.data.target = target
      if (this.pending) clearTimeout(this.pending)
      this.pending = setTimeout(() => {
        this.pending = null
        delete inst.data.target
        void player.setVolume(target)
      }, 80)
      this.renderOne(inst.action.id)
      return
    }
    // Перемотка — в режиме «перемотка» и в «треках» с нажатой крутилкой: 5 с за щелчок
    if (mode === 'seek' || pressed) {
      this.seekAcc += ticks * 5000
      if (this.pending) clearTimeout(this.pending)
      this.pending = setTimeout(() => {
        this.pending = null
        const delta = this.seekAcc
        this.seekAcc = 0
        void player.seekBy(delta)
      }, 120)
      return
    }
    // Треки: одна команда на рывок, не чаще трёх раз в секунду
    const now = Date.now()
    if (now - Number(inst.data.lastSkip ?? 0) < 300) return
    inst.data.lastSkip = now
    void (ticks > 0 ? player.next() : player.previous())
  }

  protected override dialPress(): Promise<boolean> {
    return player.toggle()
  }

  /** Касание — играть/пауза; долгое — следующий режим крутилки */
  protected override touch(inst: Instance<MediaSettings>, hold: boolean): Outcome {
    if (!hold) return player.toggle()
    const next = MEDIA_MODES[(MEDIA_MODES.indexOf(this.modeOf(inst)) + 1) % MEDIA_MODES.length] ?? 'tracks'
    inst.data.mode = next
    this.renderOne(inst.action.id)
  }
}
