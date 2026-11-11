import type { ImageSet, Track } from '~/api/types'

/** Что недавно искали и открывали из поиска — только в этом браузере */
export interface RecentItem {
  kind: 'query' | 'artist' | 'album' | 'track' | 'playlist'
  /** Для запроса — сам текст */
  id: string
  title: string
  /** Артист у альбома и трека */
  sub?: string
  image?: ImageSet | null
  covers?: ImageSet[]
  /** Трек целиком — чтобы включить его сразу */
  track?: Track
}

const KEY = 'dw.search.history'
const LEGACY = 'dw.search.recent'
const LIMIT = 12

const items = shallowRef<RecentItem[]>([])
let loaded = false

function read(): RecentItem[] {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) {
      const list = JSON.parse(raw) as unknown
      return Array.isArray(list) ? (list as RecentItem[]).filter(x => x && typeof x.id === 'string' && typeof x.title === 'string') : []
    }
    // Старый формат: просто строки запросов
    const old = JSON.parse(localStorage.getItem(LEGACY) ?? '[]') as unknown
    return Array.isArray(old) ? old.filter((x): x is string => typeof x === 'string').map(q => ({ kind: 'query' as const, id: q, title: q })) : []
  }
  catch {
    return []
  }
}

function write(list: RecentItem[]) {
  items.value = list
  try {
    localStorage.setItem(KEY, JSON.stringify(list))
  }
  catch {
    // Хранилище недоступно (приватный режим) — список живёт до перезагрузки
  }
}

const same = (a: RecentItem, b: Pick<RecentItem, 'kind' | 'id'>) =>
  a.kind === b.kind && (a.kind === 'query' ? a.id.toLowerCase() === b.id.toLowerCase() : a.id === b.id)

export function useSearchRecent() {
  if (!loaded && import.meta.client) {
    loaded = true
    items.value = read()
  }

  function add(item: RecentItem) {
    write([item, ...items.value.filter(x => !same(x, item))].slice(0, LIMIT))
  }

  function remove(item: Pick<RecentItem, 'kind' | 'id'>) {
    write(items.value.filter(x => !same(x, item)))
  }

  function clear() {
    write([])
  }

  return { items, add, remove, clear }
}
