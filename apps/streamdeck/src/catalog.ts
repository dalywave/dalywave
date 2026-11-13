// Что можно включить: коллекции, поиск, волна, «Открытия», устройства, очередь. Каждое — список
// элементов с обложкой и действием: кнопка играет первый (или выбранный), крутилка листает.
import { api } from './api'
import { t } from './i18n'
import { player } from './player'
import type { GlyphName } from './render/glyphs.gen'
import type {
  AlbumDetail, ArtistDetail, ChartDetail, ChartList, ConnectDevice, FollowedArtistPage, HistoryPage, Home,
  LikedTrackPage, PlayContext, PlaylistDetail, PlaylistList, PlaylistSummary, SavedAlbumPage, SearchResults,
  Track, WaveActivity, WaveMood, WavePreset, WaveSession, WaveSettings,
} from './types'

export interface Entry {
  /** Постоянный ключ: по нему крутилка помнит, где стоит */
  key: string
  title: string
  subtitle: string
  /** Путь обложки на сервере */
  image: string | null
  fallback: GlyphName
  /** Цвета волны (настроение) вместо обложки */
  colors?: readonly string[]
  play: () => Promise<boolean>
}

// ---------- коллекции ----------

/** Что играет кнопка/крутилка «Играть коллекцию» */
export type CollectionSource =
  | 'playlist' | 'album' | 'artist' | 'chart' | 'liked' | 'history'
  | 'playlists' | 'mixes' | 'albums' | 'artists' | 'charts'

export interface CollectionSettings {
  source?: CollectionSource
  /** id плейлиста, альбома, артиста или чарта */
  itemId?: string
  /** Название — чтобы кнопка подписалась без запроса */
  itemTitle?: string
  shuffle?: boolean
}

const LIST_SOURCES: CollectionSource[] = ['playlists', 'mixes', 'albums', 'artists', 'charts']

export function isListSource(s: CollectionSource | undefined): boolean {
  return !!s && LIST_SOURCES.includes(s)
}

function artists(track: Track): string {
  return track.artists.filter(a => a.role !== 'featured').map(a => a.name).join(', ') || track.artists.map(a => a.name).join(', ')
}

export function trackSubtitle(track: Track): string {
  return artists(track)
}

function cover(track: Track): string | null {
  return track.album?.image?.small ?? null
}

function trackEntries(tracks: Track[], context: PlayContext, shuffle = false): Entry[] {
  return tracks.map((tr, i) => ({
    key: `${tr.id}:${i}`,
    title: tr.title,
    subtitle: artists(tr),
    image: cover(tr),
    fallback: 'music-notes-bold' as const,
    play: () => player.playTracks(tracks, i, context, shuffle),
  }))
}

async function playlistTracks(id: string): Promise<{ tracks: Track[], context: PlayContext }> {
  const d = await api.get<PlaylistDetail>(`/api/playlists/${id}`)
  return { tracks: d.items.map(i => i.track), context: { type: 'playlist', id, title: d.playlist.title } }
}

async function albumTracks(id: string): Promise<{ tracks: Track[], context: PlayContext }> {
  const d = await api.get<AlbumDetail>(`/api/albums/${id}`)
  return { tracks: d.tracks, context: { type: 'album', id, title: d.album.title } }
}

async function artistTracks(id: string): Promise<{ tracks: Track[], context: PlayContext }> {
  const d = await api.get<ArtistDetail>(`/api/artists/${id}`)
  return { tracks: d.topTracks, context: { type: 'artist', id, title: d.artist.name } }
}

async function chartTracks(id: string): Promise<{ tracks: Track[], context: PlayContext }> {
  const d = await api.get<ChartDetail>(`/api/charts/${id}`)
  return { tracks: d.tracks, context: { type: 'search', id: `chart:${id}`, title: d.chart.title } }
}

async function likedTracks(): Promise<{ tracks: Track[], context: PlayContext }> {
  const d = await api.get<LikedTrackPage>('/api/library/tracks?limit=300')
  return { tracks: d.items.map(i => i.track), context: { type: 'library', id: 'liked', title: t().liked } }
}

async function historyTracks(): Promise<{ tracks: Track[], context: PlayContext }> {
  const d = await api.get<HistoryPage>('/api/me/history?limit=100')
  const seen = new Set<string>()
  const tracks = d.items.map(i => i.track).filter(tr => !seen.has(tr.id) && !!seen.add(tr.id))
  return { tracks, context: { type: 'library', id: 'history', title: t().history } }
}

function tracksOf(source: CollectionSource, id: string | undefined): Promise<{ tracks: Track[], context: PlayContext }> | null {
  switch (source) {
    case 'playlist': return id ? playlistTracks(id) : null
    case 'album': return id ? albumTracks(id) : null
    case 'artist': return id ? artistTracks(id) : null
    case 'chart': return id ? chartTracks(id) : null
    case 'liked': return likedTracks()
    case 'history': return historyTracks()
    default: return null
  }
}

function playlistEntry(p: PlaylistSummary, shuffle: boolean): Entry {
  return {
    key: p.id,
    title: p.title,
    subtitle: p.kind === 'mix' || p.kind === 'system' ? '' : `${p.tracksCount}`,
    image: p.image?.small ?? p.covers[0]?.small ?? null,
    fallback: 'playlist-bold',
    play: async () => {
      const { tracks, context } = await playlistTracks(p.id)
      return player.playTracks(tracks, 0, context, shuffle)
    },
  }
}

/** Элементы коллекции для крутилки */
export async function collectionEntries(s: CollectionSettings): Promise<Entry[]> {
  const shuffle = !!s.shuffle
  switch (s.source ?? 'liked') {
    case 'playlists': {
      const d = await api.get<PlaylistList>('/api/playlists')
      return d.items.filter(p => p.kind !== 'mix').map(p => playlistEntry(p, shuffle))
    }
    case 'mixes': {
      const home = await api.get<Home>('/api/home')
      const shelf = home.shelves.find(sh => sh.kind === 'daily_mixes')
      return (shelf?.items ?? []).flatMap(it => (it.type === 'playlist' ? [playlistEntry(it as unknown as PlaylistSummary, shuffle)] : []))
    }
    case 'albums': {
      const d = await api.get<SavedAlbumPage>('/api/library/albums?limit=100')
      return d.items.map(({ album }) => ({
        key: album.id,
        title: album.title,
        subtitle: album.artist.name,
        image: album.image?.small ?? null,
        fallback: 'vinyl-record-bold' as const,
        play: async () => {
          const { tracks, context } = await albumTracks(album.id)
          return player.playTracks(tracks, 0, context, shuffle)
        },
      }))
    }
    case 'artists': {
      const d = await api.get<FollowedArtistPage>('/api/library/artists?limit=100')
      return d.items.map(({ artist }) => ({
        key: artist.id,
        title: artist.name,
        subtitle: '',
        image: artist.image?.small ?? null,
        fallback: 'user-bold' as const,
        play: async () => {
          const { tracks, context } = await artistTracks(artist.id)
          return player.playTracks(tracks, 0, context, shuffle)
        },
      }))
    }
    case 'charts': {
      const d = await api.get<ChartList>('/api/charts')
      return d.items.map(c => ({
        key: c.id,
        title: c.title,
        subtitle: '',
        image: c.image?.small ?? null,
        fallback: 'chart-line-up-bold' as const,
        play: async () => {
          const { tracks, context } = await chartTracks(c.id)
          return player.playTracks(tracks, 0, context, shuffle)
        },
      }))
    }
    default: {
      const got = tracksOf(s.source ?? 'liked', s.itemId)
      if (!got) return []
      const { tracks, context } = await got
      return trackEntries(tracks, context, shuffle)
    }
  }
}

/** Кнопка «Играть коллекцию»: всё с начала (список коллекций — первую) */
export async function playCollection(s: CollectionSettings): Promise<boolean> {
  const source = s.source ?? 'liked'
  if (isListSource(source)) {
    const first = (await collectionEntries(s))[0]
    return first ? first.play() : false
  }
  const got = tracksOf(source, s.itemId)
  if (!got) return false
  const { tracks, context } = await got
  return player.playTracks(tracks, 0, context, !!s.shuffle)
}

/** Подпись и обложка кнопки коллекции */
export async function collectionFace(s: CollectionSettings): Promise<{ title: string, image: string | null, glyph: GlyphName }> {
  const source = s.source ?? 'liked'
  const title = s.itemTitle ?? ''
  switch (source) {
    case 'liked': return { title: t().liked, image: null, glyph: 'heart-fill' }
    case 'history': return { title: t().history, image: null, glyph: 'clock-counter-clockwise-bold' }
    case 'playlist': {
      if (!s.itemId) return { title, image: null, glyph: 'playlist-bold' }
      const d = await player.loadPlaylist(s.itemId)
      return { title: d?.playlist.title ?? title, image: d?.playlist.image?.small ?? d?.playlist.covers[0]?.small ?? null, glyph: 'playlist-bold' }
    }
    case 'album': {
      if (!s.itemId) return { title, image: null, glyph: 'vinyl-record-bold' }
      const d = await api.get<AlbumDetail>(`/api/albums/${s.itemId}`)
      return { title: d.album.title, image: d.album.image?.small ?? null, glyph: 'vinyl-record-bold' }
    }
    case 'artist': {
      if (!s.itemId) return { title, image: null, glyph: 'user-bold' }
      const d = await api.get<ArtistDetail>(`/api/artists/${s.itemId}`)
      return { title: d.artist.name, image: d.artist.image?.small ?? null, glyph: 'user-bold' }
    }
    case 'chart': {
      if (!s.itemId) return { title, image: null, glyph: 'chart-line-up-bold' }
      const d = await api.get<ChartDetail>(`/api/charts/${s.itemId}`)
      return { title: d.chart.title, image: d.chart.image?.small ?? null, glyph: 'chart-line-up-bold' }
    }
    case 'playlists': return { title: title || '', image: null, glyph: 'playlist-bold' }
    case 'mixes': return { title: '', image: null, glyph: 'sparkle-bold' }
    case 'albums': return { title: '', image: null, glyph: 'vinyl-record-bold' }
    case 'artists': return { title: '', image: null, glyph: 'user-bold' }
    case 'charts': return { title: '', image: null, glyph: 'chart-line-up-bold' }
  }
}

// ---------- сохранённый поиск ----------

export type SearchKind = 'track' | 'album' | 'artist'

export interface SearchSettings {
  query?: string
  kind?: SearchKind
  scope?: 'catalog' | 'library'
}

export async function searchEntries(s: SearchSettings): Promise<Entry[]> {
  const q = s.query?.trim()
  if (!q) return []
  const kind = s.kind ?? 'track'
  const params = new URLSearchParams({ q, types: kind, limit: '20', scope: s.scope ?? 'catalog' })
  const r = await api.get<SearchResults>(`/api/search?${params}`)
  if (kind === 'track') return trackEntries(r.tracks, { type: 'search', id: q, title: q })
  if (kind === 'album') {
    return r.albums.map(a => ({
      key: a.id,
      title: a.title,
      subtitle: a.artist.name,
      image: a.image?.small ?? null,
      fallback: 'vinyl-record-bold' as const,
      play: async () => {
        const { tracks, context } = await albumTracks(a.id)
        return player.playTracks(tracks, 0, context)
      },
    }))
  }
  return r.artists.map(a => ({
    key: a.id,
    title: a.name,
    subtitle: '',
    image: a.image?.small ?? null,
    fallback: 'user-bold' as const,
    play: async () => {
      const { tracks, context } = await artistTracks(a.id)
      return player.playTracks(tracks, 0, context)
    },
  }))
}

/** Кнопка поиска: треки — все результаты подряд; альбом и артист — первый найденный */
export async function playSearch(s: SearchSettings): Promise<boolean> {
  const first = (await searchEntries(s))[0]
  return first ? first.play() : false
}

// ---------- волна ----------

/** Цвета шара настроения, как в интерфейсе (utils/wave-options.ts) */
export const MOOD_COLORS: Record<WaveMood, readonly [string, string, string]> = {
  any: ['#4b21ff', '#ff3d7a', '#ff9a3c'],
  energetic: ['#7a2208', '#ff6a1a', '#ffd27a'],
  happy: ['#5c0f5e', '#ff4fa8', '#ffe9a8'],
  calm: ['#0a2a52', '#1d8cbc', '#aef0ff'],
  sad: ['#1c1946', '#5b50c4', '#d3ccff'],
}

/** Светлые цвета волны на кнопке: тёмный край шара на чёрном не видно */
export function waveColors(mood: WaveMood | undefined): readonly string[] {
  const c = MOOD_COLORS[mood ?? 'any']
  return mood && mood !== 'any' ? [c[1], c[2]] : ['#8e6bff', '#d45bef', '#ff6f95', '#ffa25c']
}

const MOODS: WaveMood[] = ['any', 'energetic', 'happy', 'calm', 'sad']
const ACTIVITY_GLYPH: Record<WaveActivity, GlyphName> = {
  any: 'infinity-bold', wake: 'sun-horizon-bold', road: 'car-bold', work: 'laptop-bold', sport: 'barbell-bold',
  rest: 'coffee-bold', sleep: 'moon-stars-bold', party: 'confetti-bold',
}

export interface WaveKeySettings {
  /** Сохранённая настройка; нет — свои значения ниже */
  presetId?: string
  mood?: WaveMood
  activity?: WaveActivity
  character?: WaveSettings['character']
  language?: WaveSettings['language']
}

let presets: Promise<WavePreset[]> | null = null

export function wavePresets(fresh = false): Promise<WavePreset[]> {
  if (!presets || fresh) {
    presets = api.get<{ items: WavePreset[] }>('/api/wave/presets').then(d => d.items).catch(() => {
      presets = null
      return []
    })
  }
  return presets
}

export function forgetPresets(): void {
  presets = null
}

/** Настройки волны кнопки и её подпись */
export async function waveOf(s: WaveKeySettings): Promise<{ settings: WaveSettings, title: string }> {
  if (s.presetId) {
    const p = (await wavePresets()).find(x => x.id === s.presetId)
    if (p) return { settings: p.settings, title: p.name }
  }
  const settings: WaveSettings = {
    mood: s.mood ?? 'any',
    activity: s.activity ?? 'any',
    character: s.character ?? 'mix',
    language: s.language ?? 'any',
  }
  return { settings, title: waveTitle(settings) }
}

/** «Работа · Спокойное», иначе «Моя волна» */
export function waveTitle(s: WaveSettings): string {
  const parts = [t().activity[s.activity ?? 'any'], s.mood && s.mood !== 'any' ? t().mood[s.mood] : '', t().language[s.language ?? 'any']].filter(Boolean)
  return parts.length ? parts.slice(0, 2).join(' · ') : t().wave
}

/** Крутилка волны: сохранённые настройки, затем настроения */
export async function waveEntries(): Promise<Entry[]> {
  const out: Entry[] = (await wavePresets()).map(p => ({
    key: `preset:${p.id}`,
    title: p.name,
    subtitle: '',
    image: null,
    fallback: 'sparkle-bold' as const,
    colors: waveColors(p.settings.mood),
    play: () => player.startWave(p.settings),
  }))
  for (const mood of MOODS) {
    out.push({
      key: `mood:${mood}`,
      title: mood === 'any' ? t().wave : t().mood[mood],
      subtitle: '',
      image: null,
      fallback: 'waves-bold',
      colors: waveColors(mood),
      play: () => player.startWave({ mood, activity: 'any', character: 'mix', language: 'any' }),
    })
  }
  return out
}

export function activityGlyph(a: WaveActivity | undefined): GlyphName {
  return ACTIVITY_GLYPH[a ?? 'any']
}

// ---------- «Открытия» ----------

const DISCOVER: WaveSettings = { character: 'new', mood: 'any', activity: 'any', language: 'any' }

/** Кнопка «Открытия»: волна только из нового на активном устройстве */
export function startDiscover(): Promise<boolean> {
  return player.startWave(DISCOVER)
}

/** Новые треки для крутилки: начало волны «только новое», без того, что уже нравится */
export async function discoverEntries(): Promise<Entry[]> {
  const s = await api.post<WaveSession>('/api/wave/sessions', { settings: DISCOVER })
  const tracks = s.queue.map(i => i.track).filter(tr => !tr.liked)
  return trackEntries(tracks, { type: 'mix', id: 'discover', title: t().discover })
}

// ---------- устройства ----------

export const KIND_GLYPH: Record<ConnectDevice['kind'], GlyphName> = {
  laptop: 'laptop-bold',
  desktop: 'desktop-bold',
  phone: 'device-mobile-bold',
  tablet: 'device-tablet-bold',
  web: 'browser-bold',
  watch: 'watch-bold',
  tv: 'television-simple-bold',
}

/** Куда можно перенести звук: на связи и не пульт */
export function playableDevices(): ConnectDevice[] {
  return player.devices.filter(d => d.online && !d.capabilities.remote)
}

export function deviceEntries(): Entry[] {
  return playableDevices().map(d => ({
    key: d.id,
    title: d.name,
    subtitle: d.id === player.link.activeId ? t().playing : '',
    image: null,
    fallback: KIND_GLYPH[d.kind] ?? 'devices-bold',
    play: () => player.transfer(d.id),
  }))
}

// ---------- очередь ----------

export function queueEntries(): Entry[] {
  return player.upcoming().map(it => ({
    key: it.uid,
    title: it.track.title,
    subtitle: artists(it.track),
    image: cover(it.track),
    fallback: 'music-notes-bold' as const,
    play: () => player.jumpTo(it.uid),
  }))
}
