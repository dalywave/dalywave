// Списки: коллекции, сохранённый поиск, очередь, «Моя волна», «Открытия», устройства. Кнопка
// включает своё, крутилка листает элементы (на полосе — обложка и название), нажатие — играть.
// Ещё «Добавить в…»: текущий трек в плейлист или в «Любимые».
import { api } from '../api'
import {
  type CollectionSettings, type Entry, type SearchSettings, type WaveKeySettings, KIND_GLYPH, activityGlyph,
  collectionEntries, collectionFace, deviceEntries, discoverEntries, forgetPresets, playCollection, playSearch,
  playableDevices, queueEntries, searchEntries, startDiscover, waveColors, waveEntries, waveOf,
} from '../catalog'
import { t } from '../i18n'
import { player } from '../player'
import { artKey, iconKey } from '../render/keys'
import { LAYOUT, listFields } from '../render/strip'
import { DeckAction, type Instance, type Outcome } from './base'

type Json = Record<string, string | number | boolean | null | undefined>

interface ListState {
  entries?: Entry[]
  loadedAt?: number
  loading?: boolean
  covers?: Map<string, string | null>
}

/** Крутилка со списком: курсор, ленивые обложки, перезагрузка по долгому касанию */
abstract class ListAction<S extends Json> extends DeckAction<S> {
  /** Сколько список живёт без перезагрузки */
  protected ttl = 120_000
  /** Список берётся из состояния плеера при каждой отрисовке (очередь, устройства) */
  protected live = false

  protected abstract load(inst: Instance<S>): Promise<Entry[]> | Entry[]

  private state(inst: Instance<S>): ListState {
    inst.data.list ??= {}
    return inst.data.list as ListState
  }

  protected entries(inst: Instance<S>): Entry[] | null {
    const st = this.state(inst)
    if (this.live) return this.load(inst) as Entry[]
    const stale = !st.loadedAt || Date.now() - st.loadedAt > this.ttl
    if (stale && !st.loading) void this.reload(inst)
    return st.entries ?? null
  }

  protected async reload(inst: Instance<S>, reset = false): Promise<void> {
    const st = this.state(inst)
    st.loading = true
    try {
      st.entries = await this.load(inst)
      st.loadedAt = Date.now()
      if (reset) inst.cursor = 0
      inst.cursor = Math.min(inst.cursor, Math.max(0, st.entries.length - 1))
    }
    catch {
      st.loadedAt = Date.now() - this.ttl + 15_000
    }
    finally {
      st.loading = false
      this.renderOne(inst.action.id)
    }
  }

  protected override settingsChanged(inst: Instance<S>): void {
    inst.data.list = {}
    inst.cursor = 0
  }

  protected selected(inst: Instance<S>): Entry | null {
    const list = this.entries(inst)
    return list?.[Math.min(inst.cursor, list.length - 1)] ?? null
  }

  /** Обложка элемента: из кеша или null, пока грузится */
  protected coverOf(inst: Instance<S>, entry: Entry | null): string | null {
    if (!entry?.image) return null
    const st = this.state(inst)
    st.covers ??= new Map()
    if (st.covers.has(entry.image)) return st.covers.get(entry.image) ?? null
    st.covers.set(entry.image, null)
    void api.cover(entry.image).then((uri) => {
      st.covers?.set(entry.image!, uri)
      if (uri) this.renderOne(inst.action.id)
    })
    return null
  }

  protected override renderDial(inst: Instance<S>): void {
    this.setLayout(inst, LAYOUT.list)
    const list = this.entries(inst)
    if (!list) {
      this.feedback(inst, listFields({ cover: null, fallback: 'music-notes-bold', title: t().loading, subtitle: '', counter: '' }))
      return
    }
    if (!list.length) {
      this.feedback(inst, listFields({ cover: null, fallback: 'music-notes-bold', title: this.emptyText(), subtitle: '', counter: '' }))
      return
    }
    inst.cursor = Math.min(inst.cursor, list.length - 1)
    const e = list[inst.cursor]!
    this.feedback(inst, listFields({
      cover: this.coverOf(inst, e),
      fallback: e.fallback,
      colors: e.colors,
      title: e.title,
      subtitle: e.subtitle,
      counter: list.length > 1 ? `${inst.cursor + 1} ${t().of} ${list.length}` : '',
    }))
  }

  protected emptyText(): string {
    return t().empty
  }

  protected override rotate(inst: Instance<S>, ticks: number): void {
    const list = this.entries(inst)
    if (!list?.length) return
    inst.cursor = Math.min(list.length - 1, Math.max(0, inst.cursor + ticks))
    this.renderOne(inst.action.id)
  }

  protected override async dialPress(inst: Instance<S>): Promise<boolean> {
    const e = this.selected(inst)
    return e ? e.play() : false
  }

  /** Касание — играть выбранное; долгое — обновить список и вернуться к началу */
  protected override touch(inst: Instance<S>, hold: boolean): Outcome {
    if (!hold) return this.dialPress(inst)
    return this.reload(inst, true)
  }
}

// ---------- Играть коллекцию ----------

export class CollectionAction extends ListAction<CollectionSettings & Json> {
  override readonly manifestId = 'rest.daly.wave.collection'

  protected load(inst: Instance<CollectionSettings & Json>): Promise<Entry[]> {
    return collectionEntries(inst.settings)
  }

  protected override appeared(inst: Instance<CollectionSettings & Json>): void {
    void this.loadFace(inst)
  }

  protected override settingsChanged(inst: Instance<CollectionSettings & Json>): void {
    super.settingsChanged(inst)
    void this.loadFace(inst)
  }

  private async loadFace(inst: Instance<CollectionSettings & Json>): Promise<void> {
    if (!inst.action.isKey() || !player.paired) return
    try {
      const face = await collectionFace(inst.settings)
      inst.data.face = { ...face, cover: face.image ? await api.cover(face.image) : null }
    }
    catch {
      inst.data.face = undefined
    }
    this.renderOne(inst.action.id)
  }

  protected override renderKey(inst: Instance<CollectionSettings & Json>): void {
    const face = inst.data.face as { title: string, cover: string | null, glyph: Parameters<typeof artKey>[1] } | undefined
    if (!face) {
      if (!inst.data.faceAsked) {
        inst.data.faceAsked = true
        void this.loadFace(inst)
      }
      this.setImage(inst, iconKey({ glyph: 'playlist-bold', tone: 'off' }))
      return
    }
    if (inst.settings.source === 'liked' || !inst.settings.source) {
      this.setImage(inst, iconKey({ glyph: 'heart-fill', tone: 'accent', label: face.title }))
      return
    }
    this.setImage(inst, artKey(face.cover, face.glyph, face.title || undefined))
  }

  protected override press(inst: Instance<CollectionSettings & Json>): Promise<boolean> {
    return playCollection(inst.settings)
  }
}

// ---------- Сохранённый поиск ----------

export class SearchAction extends ListAction<SearchSettings & Json> {
  override readonly manifestId = 'rest.daly.wave.search'

  protected load(inst: Instance<SearchSettings & Json>): Promise<Entry[]> {
    return searchEntries(inst.settings)
  }

  protected override emptyText(): string {
    return t().search
  }

  protected override renderKey(inst: Instance<SearchSettings & Json>): void {
    const q = inst.settings.query?.trim()
    this.setImage(inst, iconKey({ glyph: 'magnifying-glass-bold', label: q || t().search, tone: q ? 'normal' : 'off' }))
  }

  protected override press(inst: Instance<SearchSettings & Json>): Promise<boolean> {
    return playSearch(inst.settings)
  }
}

// ---------- Очередь ----------

export class QueueAction extends ListAction<Json> {
  override readonly manifestId = 'rest.daly.wave.queue'
  protected override live = true

  protected load(): Entry[] {
    return queueEntries()
  }

  protected override emptyText(): string {
    return t().queueEmpty
  }

  protected override renderKey(inst: Instance<Json>): void {
    const next = player.upcoming()[0]
    if (!next) {
      this.setImage(inst, iconKey({ glyph: 'queue-bold', tone: 'off', label: t().next }))
      return
    }
    const entry = queueEntries()[0]!
    this.setImage(inst, artKey(this.coverOf(inst, entry), 'music-notes-bold', next.track.title))
  }

  /** Кнопка: к следующему в очереди */
  protected override press(): Promise<boolean> {
    const next = player.upcoming()[0]
    return next ? player.jumpTo(next.uid) : player.next()
  }

  protected override async dialPress(inst: Instance<Json>): Promise<boolean> {
    const ok = await super.dialPress(inst)
    inst.cursor = 0
    return ok
  }
}

// ---------- Моя волна ----------

export class WaveAction extends ListAction<WaveKeySettings & Json> {
  override readonly manifestId = 'rest.daly.wave.wave'

  constructor() {
    super()
    player.link.on('library', (entities) => {
      if (!entities.includes('wavePresets')) return
      forgetPresets()
      for (const inst of this.instances.values()) {
        inst.data.list = {}
        void this.loadFace(inst)
      }
    })
  }

  protected load(): Promise<Entry[]> {
    return waveEntries()
  }

  protected override appeared(inst: Instance<WaveKeySettings & Json>): void {
    void this.loadFace(inst)
  }

  protected override settingsChanged(inst: Instance<WaveKeySettings & Json>): void {
    super.settingsChanged(inst)
    void this.loadFace(inst)
  }

  private async loadFace(inst: Instance<WaveKeySettings & Json>): Promise<void> {
    if (!player.paired) return
    const w = await waveOf(inst.settings)
    inst.data.wave = w
    this.renderOne(inst.action.id)
  }

  protected override renderKey(inst: Instance<WaveKeySettings & Json>): void {
    const w = inst.data.wave as Awaited<ReturnType<typeof waveOf>> | undefined
    const mood = w?.settings.mood ?? inst.settings.mood
    const activity = w?.settings.activity ?? inst.settings.activity
    const label = w?.title ?? t().wave
    // Волна с занятием — его значок; иначе — волна в цветах настроения
    if (activity && activity !== 'any') {
      this.setImage(inst, iconKey({ glyph: activityGlyph(activity), label, color: waveColors(mood)[0] }))
      return
    }
    this.setImage(inst, iconKey({ glyph: 'wave', waveColors: waveColors(mood), label }))
  }

  protected override async press(inst: Instance<WaveKeySettings & Json>): Promise<boolean> {
    const w = await waveOf(inst.settings)
    return player.startWave(w.settings)
  }
}

// ---------- Открытия ----------

export class DiscoverAction extends ListAction<Json> {
  override readonly manifestId = 'rest.daly.wave.discover'
  protected override ttl = 15 * 60_000

  protected load(): Promise<Entry[]> {
    return discoverEntries()
  }

  protected override renderKey(inst: Instance<Json>): void {
    this.setImage(inst, iconKey({ glyph: 'compass-bold', label: t().discover }))
  }

  protected override press(): Promise<boolean> {
    return startDiscover()
  }
}

// ---------- Передать на устройство ----------

interface TransferSettings extends Json { deviceId?: string, deviceName?: string }

export class TransferAction extends ListAction<TransferSettings> {
  override readonly manifestId = 'rest.daly.wave.transfer'
  protected override live = true
  protected override needsDevice = false

  protected load(): Entry[] {
    return deviceEntries()
  }

  protected override emptyText(): string {
    return t().noDevice
  }

  protected override renderKey(inst: Instance<TransferSettings>): void {
    const id = inst.settings.deviceId
    const d = player.devices.find(x => x.id === id)
    const name = d?.name ?? inst.settings.deviceName ?? ''
    const glyph = d ? KIND_GLYPH[d.kind] ?? 'devices-bold' : 'devices-bold'
    const active = !!id && id === player.link.activeId
    this.setImage(inst, iconKey({ glyph, label: name || t().noDevice, tone: !d?.online ? 'off' : active ? 'on' : 'normal' }))
  }

  protected override press(inst: Instance<TransferSettings>): Promise<boolean> {
    const id = inst.settings.deviceId
    const d = playableDevices().find(x => x.id === id)
    return d ? player.transfer(d.id) : Promise.resolve(false)
  }
}

// ---------- Добавить в… ----------

interface AddSettings extends Json { target?: 'library' | 'playlist', playlistId?: string }

export class AddToAction extends DeckAction<AddSettings> {
  override readonly manifestId = 'rest.daly.wave.add'

  protected override renderKey(inst: Instance<AddSettings>): void {
    const s = inst.settings
    if ((s.target ?? 'library') === 'library' || !s.playlistId) {
      const liked = player.liked()
      this.setImage(inst, iconKey(liked ? { glyph: 'heart-fill', tone: 'accent', label: t().liked } : { glyph: 'heart-bold', label: t().liked, tone: player.track ? 'normal' : 'off' }))
      return
    }
    const present = player.inPlaylist(s.playlistId)
    this.setImage(inst, iconKey({ glyph: present ? 'check-bold' : 'list-plus-bold', label: player.playlistTitle(s.playlistId) ?? '', tone: player.track ? 'normal' : 'off' }))
  }

  protected override async press(inst: Instance<AddSettings>): Promise<boolean> {
    const s = inst.settings
    if ((s.target ?? 'library') === 'library' || !s.playlistId) return player.toggleLiked()
    const ok = await player.togglePlaylist(s.playlistId)
    if (ok && inst.action.isKey()) void inst.action.showOk()
    return ok
  }
}
