import type { ResolvedLink } from '~/api/types'
import type { ShareKind } from './useShare'

// «Поделиться → DalyWave»: ссылка из другого сервиса (Deezer, Spotify, Apple Music, YouTube,
// SoundCloud, Bandcamp) становится треком, альбомом или артистом каталога. Приходит из меню
// «Поделиться» iPhone и macOS (dalywave://share?url=…), вставкой в поиск или в окно, перетаскиванием.
// Своя ссылка (wave.daly.rest/album/…) просто открывает экран. Лист с ответом — <MusicSharedLink />.

const HOSTS = [
  /(^|\.)deezer\.com$/, /^deezer\.page\.link$/, /^link\.deezer\.com$/,
  /^open\.spotify\.com$/, /^spotify\.link$/,
  /^(music|itunes|geo\.music)\.apple\.com$/,
  /(^|\.)youtube\.com$/, /^youtu\.be$/,
  /(^|\.)soundcloud\.com$/, /^on\.soundcloud\.com$/,
  /\.bandcamp\.com$/,
]
const OWN_HOSTS = ['wave.daly.rest']
const KINDS: ShareKind[] = ['track', 'album', 'artist', 'playlist']
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
/** Код ссылки-приглашения: «Добавить в друзья», вход в совместный плейлист */
const CODE = /^[\w-]{8,32}$/

/** Приглашения и друзья: `friends`, `friends/{id}`, `friends/add/{code}`, `playlist/join/{code}` */
function socialRoute(parts: string[]): string | null {
  const [a, b, c] = parts
  if (a === 'friends' && !b) return '/friends'
  if (a === 'friends' && b && UUID.test(b) && !c) return `/friends/${b}`
  if (a === 'friends' && b === 'add' && c && CODE.test(c)) return `/friends/add/${c}`
  if (a === 'playlist' && b === 'join' && c && CODE.test(c)) return `/playlist/join/${c}`
  return null
}

function ownHost(host: string) {
  return OWN_HOSTS.includes(host) || (typeof location !== 'undefined' && /^https?:$/.test(location.protocol) && host === location.host)
}

/** Своя ссылка → экран: `/album/{id}` */
export function ownRoute(url: URL): string | null {
  if (!ownHost(url.host)) return null
  const parts = url.pathname.split('/').filter(Boolean)
  const [kind, id] = parts
  const social = socialRoute(parts)
  if (social) return social
  return kind && id && KINDS.includes(kind as ShareKind) && UUID.test(id) && parts.length === 2 ? `/${kind}/${id}` : null
}

/** Разделы, которые открываются ссылкой приложения как есть (уведомления: перенос, итоги, админка) */
const SECTIONS = ['import', 'rewind', 'notifications', 'history', 'library', 'settings', 'liked']
const ADMIN_SECTIONS = ['disputed', 'sources', 'downloads', 'import', 'catalog', 'storage', 'reco', 'access', 'log', 'settings']

/** Ссылка на экран приложения → путь: `dalywave://album/{id}` (виджет «Недавнее», уведомление) →
 *  `/album/{id}`, `dalywave://rewind?year=2026` → `/rewind?year=2026`, `dalywave://admin/disputed` → `/admin/disputed` */
export function appRoute(text: string): string | null {
  const raw = text.trim()
  if (!raw.startsWith('dalywave://')) return null
  try {
    const url = new URL(raw)
    const host = url.host
    const parts = url.pathname.split('/').filter(Boolean)
    const social = socialRoute([host, ...parts])
    if (social) return social
    if (KINDS.includes(host as ShareKind)) {
      const [id] = parts
      return id && UUID.test(id) ? `/${host}/${id}` : null
    }
    if (SECTIONS.includes(host) && !parts.length) return `/${host}${url.search}`
    if (host === 'admin' && parts.length <= 1 && (!parts[0] || ADMIN_SECTIONS.includes(parts[0]))) return `/admin${parts[0] ? `/${parts[0]}` : ''}${url.search}`
    return null
  }
  catch {
    return null
  }
}

/** Первая ссылка в тексте, которую умеем открыть; dalywave://share?url=… разворачивается */
export function findLink(text: string): URL | null {
  const raw = text.trim()
  if (raw.startsWith('dalywave://')) {
    try {
      const inner = new URL(raw).searchParams.get('url') ?? new URL(raw).searchParams.get('text')
      return inner ? findLink(inner) : null
    }
    catch {
      return null
    }
  }
  for (const m of raw.matchAll(/https?:\/\/[^\s<>"'«»]+/g)) {
    try {
      const url = new URL(m[0].replace(/[),.;!?]+$/, ''))
      if (ownRoute(url) || HOSTS.some(h => h.test(url.hostname))) return url
    }
    catch {
      // не ссылка — ищем дальше
    }
  }
  return null
}

export interface SharedLinkState {
  url: string
  status: 'loading' | 'ready' | 'error'
  result: ResolvedLink | null
  /** `unsupported_link`, `not_found`, `network` */
  error: string | null
}

const state = ref<SharedLinkState | null>(null)
let seq = 0

export function useSharedLink() {
  const bridge = useBridge()
  const ui = useUiStore()

  /** Открыть ссылку из текста. false — ссылки в тексте нет (вставка идёт как обычно) */
  function open(text: string): boolean {
    const screen = appRoute(text)
    if (screen) {
      state.value = null
      void ui.navigate(screen)
      return true
    }
    const url = findLink(text)
    if (!url) return false
    const route = ownRoute(url)
    if (route) {
      state.value = null
      void ui.navigate(route)
      return true
    }
    const run = ++seq
    state.value = { url: url.href, status: 'loading', result: null, error: null }
    bridge.catalog.resolveLink(url.href).then((result) => {
      if (run !== seq || !state.value) return
      // Артист — сразу на его страницу: предлагать там нечего
      if (result.kind === 'artist' && result.artist) {
        state.value = null
        void ui.navigate(`/artist/${result.artist.id}`)
        return
      }
      if (result.kind === 'playlist' && result.playlistId) {
        state.value = null
        void ui.navigate(`/playlist/${result.playlistId}`)
        return
      }
      state.value = { ...state.value, status: 'ready', result }
    }, (e: unknown) => {
      if (run !== seq || !state.value) return
      const code = (e as { code?: string | null } | null)?.code
      state.value = { ...state.value, status: 'error', error: code === 'unsupported_link' || code === 'not_found' ? code : 'network' }
    })
    return true
  }

  function close() {
    seq++
    state.value = null
  }

  return { state: computed(() => state.value), open, close }
}
