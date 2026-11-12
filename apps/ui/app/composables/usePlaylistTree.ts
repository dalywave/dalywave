import type { PlaylistFolder, PlaylistList, PlaylistSort, PlaylistSummary } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

export type TreeNode =
  | { kind: 'folder', id: string, folder: PlaylistFolder, children: PlaylistSummary[] }
  | { kind: 'playlist', id: string, playlist: PlaylistSummary }

/** Куда бросили: перед строкой, после неё или внутрь папки */
export type DropZone = 'before' | 'after' | 'into'

interface Dragged { kind: 'folder' | 'playlist', id: string }

/**
 * Папки и порядок плейлистов — одно дерево для боковой панели и медиатеки. Порядок — настройка
 * слушателя: «Недавние» (игравшие или изменённые), «По названию», «Свой порядок» (перетаскиванием).
 * Закреплённые — первыми на своём уровне. Перетаскивание (компьютер) в «Своём порядке» ставит
 * строку перед или после соседа, на папку — внутрь; в остальных порядках — только в папку и из неё
 */
export function usePlaylistTree() {
  const library = useLibraryStore()
  const account = useAccountSettings()
  const pui = usePlaylistUi()
  const bridge = useBridge()
  const toast = useToast()
  const lang = useLang()
  const t = useNuxtApp().$i18n.t

  const sort = computed<PlaylistSort>({
    get: () => account.settings.playlistSort,
    set: v => void account.update({ playlistSort: v }),
  })

  const collator = computed(() => new Intl.Collator(lang.value, { sensitivity: 'base', numeric: true }))
  const recency = (p: PlaylistSummary) => (p.playedAt && p.playedAt > p.updatedAt ? p.playedAt : p.updatedAt)

  function sortPlaylists(list: PlaylistSummary[]): PlaylistSummary[] {
    return [...list].sort((a, b) => {
      if (a.pinned !== b.pinned) return a.pinned ? -1 : 1
      if (sort.value === 'title') return collator.value.compare(a.title, b.title)
      if (sort.value === 'custom') return (a.position ?? 0) - (b.position ?? 0)
      return recency(b).localeCompare(recency(a))
    })
  }

  const visible = computed(() => (library.playlists ?? []).filter(p => !pui.hidden.has(p.id)))

  /** Плейлисты папки в текущем порядке */
  function childrenOf(folderId: string): PlaylistSummary[] {
    return sortPlaylists(visible.value.filter(p => p.folderId === folderId))
  }

  /** Вне папок: папки и плейлисты одной лентой в текущем порядке */
  const tree = computed<TreeNode[]>(() => {
    const known = new Set(library.folders.map(f => f.id))
    const folders: TreeNode[] = library.folders.map(f => ({ kind: 'folder', id: f.id, folder: f, children: childrenOf(f.id) }))
    // Плейлист из папки, которой уже нет (пришёл раньше её удаления), — вне папок
    const loose = visible.value.filter(p => !p.folderId || !known.has(p.folderId))
    const playlists: TreeNode[] = loose.map(p => ({ kind: 'playlist', id: p.id, playlist: p }))
    const key = (n: TreeNode) => {
      if (n.kind === 'playlist') return { pinned: n.playlist.pinned, title: n.playlist.title, position: n.playlist.position ?? 0, recent: recency(n.playlist) }
      const recent = [n.folder.updatedAt, ...n.children.map(recency)].sort().at(-1)!
      return { pinned: false, title: n.folder.title, position: n.folder.position, recent }
    }
    return [...folders, ...playlists].sort((a, b) => {
      const ka = key(a)
      const kb = key(b)
      if (ka.pinned !== kb.pinned) return ka.pinned ? -1 : 1
      if (sort.value === 'title') return collator.value.compare(ka.title, kb.title)
      if (sort.value === 'custom') return ka.position - kb.position
      return kb.recent.localeCompare(ka.recent)
    })
  })

  const folderOf = (id: string | null | undefined) => library.folders.find(f => f.id === id) ?? null

  function apply(list: PlaylistList) {
    library.applyList(list)
  }

  async function guard<T>(job: () => Promise<T>): Promise<T | undefined> {
    try {
      return await job()
    }
    catch {
      toast.error(t('folder.failed'))
    }
  }

  // ---------- Папки ----------

  /** Новая папка (с плейлистами сразу); id новой */
  async function createFolder(playlistIds: string[] = []): Promise<string | undefined> {
    const title = await pui.prompt({ title: t('folder.new'), placeholder: t('folder.newName'), action: t('folder.create') })
    if (!title) return
    const before = new Set(library.folders.map(f => f.id))
    const list = await guard(() => bridge.playlists.createFolder({ title, playlistIds }))
    if (!list) return
    apply(list)
    return (list.folders ?? []).find(f => !before.has(f.id))?.id
  }

  async function renameFolder(f: PlaylistFolder) {
    const title = await pui.prompt({ title: t('folder.renameTitle'), value: f.title, placeholder: t('folder.newName'), action: t('folder.save') })
    if (!title || title === f.title) return
    const updated = await guard(() => bridge.playlists.updateFolder(f.id, { title }))
    if (updated) library.folders = library.folders.map(x => (x.id === f.id ? updated : x))
  }

  async function deleteFolder(f: PlaylistFolder) {
    const ok = await pui.confirm({ title: t('folder.deleteTitle', { title: f.title }), text: t('folder.deleteText'), action: t('pl.delete'), danger: true })
    if (!ok) return
    const list = await guard(() => bridge.playlists.removeFolder(f.id))
    if (!list) return
    apply(list)
    toast.show({ text: t('folder.deleted', { title: f.title }), icon: 'ph:trash' })
  }

  /** Свернуть или развернуть — сразу, запоминается на сервере для всех устройств */
  function toggleFolder(f: PlaylistFolder) {
    const collapsed = !f.collapsed
    library.folders = library.folders.map(x => (x.id === f.id ? { ...x, collapsed } : x))
    void bridge.playlists.updateFolder(f.id, { collapsed }).catch(() => {
      library.folders = library.folders.map(x => (x.id === f.id ? { ...x, collapsed: !collapsed } : x))
    })
  }

  function folderMenu(f: PlaylistFolder): MenuEntry[] {
    const n = childrenOf(f.id).length
    return [
      { kind: 'heading', label: f.title, hint: n ? `${t('folder.kind')} · ${formatCount(n, 'playlist', lang.value)}` : t('folder.empty') },
      { label: t('folder.rename'), icon: 'ph:pencil-simple', onSelect: () => void renameFolder(f) },
      { kind: 'separator' },
      { label: t('folder.delete'), icon: 'ph:trash', danger: true, onSelect: () => void deleteFolder(f) },
    ]
  }

  // ---------- Перекладка ----------

  /** В папку (в конец) или вне папок (`null`, в конец) */
  async function moveTo(p: PlaylistSummary, folderId: string | null, opts: { quiet?: boolean } = {}) {
    if ((p.folderId ?? null) === folderId) return
    const list = await guard(() => bridge.playlists.move(p.id, { folderId }))
    if (!list) return
    apply(list)
    if (opts.quiet) return
    const f = folderOf(folderId)
    toast.show({ text: f ? t('folder.moved', { title: p.title, folder: f.title }) : t('folder.movedOut', { title: p.title }), icon: 'ph:folder-simple' })
  }

  /** Подменю «Переместить в папку»: папки, «Новая папка…», «Убрать из папки» */
  function moveMenu(p: PlaylistSummary): MenuEntry {
    const children: MenuEntry[] = library.folders
      .filter(f => f.id !== p.folderId)
      .map(f => ({ label: f.title, icon: 'ph:folder-simple', onSelect: () => void moveTo(p, f.id) }))
    if (children.length) children.push({ kind: 'separator' })
    children.push({ label: `${t('folder.new')}…`, icon: 'ph:folder-simple-plus', onSelect: () => void createFolder([p.id]) })
    if (p.folderId) children.push({ label: t('folder.moveOut'), icon: 'ph:folder-simple-minus', onSelect: () => void moveTo(p, null) })
    return { label: t('folder.moveTo'), icon: 'ph:folder-simple', children }
  }

  // ---------- Перетаскивание (компьютер) ----------

  const dragging = ref<Dragged | null>(null)
  const over = ref<{ id: string, zone: DropZone } | null>(null)

  function onDragStart(kind: Dragged['kind'], id: string, e: DragEvent) {
    // Треки тянут из списков — здесь только свои строки
    dragging.value = { kind, id }
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move'
      e.dataTransfer.setData('text/plain', id)
    }
  }

  function onDragEnd() {
    dragging.value = null
    over.value = null
  }

  /** Зона под указателем: у папки середина — «внутрь»; в чужом порядке только «внутрь» и «на уровень» */
  function zoneOf(target: TreeNode, e: DragEvent): DropZone | null {
    const d = dragging.value
    if (!d || d.id === target.id) return null
    const el = e.currentTarget as HTMLElement
    const r = el.getBoundingClientRect()
    const y = (e.clientY - r.top) / Math.max(1, r.height)
    const custom = sort.value === 'custom'
    if (target.kind === 'folder' && d.kind === 'playlist') {
      if (!custom || (y > 0.25 && y < 0.75)) return 'into'
    }
    if (!custom) return d.kind === 'playlist' ? 'after' : null
    return y < 0.5 ? 'before' : 'after'
  }

  function onDragOver(target: TreeNode, e: DragEvent) {
    const zone = zoneOf(target, e)
    if (!zone) return
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'
    over.value = { id: target.id, zone }
  }

  function onDragLeave(target: TreeNode) {
    if (over.value?.id === target.id) over.value = null
  }

  /** Соседи на уровне в «Своём порядке» (как на сервере): вне папок — папки и свободные плейлисты */
  function level(folderId: string | null): string[] {
    if (folderId) return sortCustom(visible.value.filter(p => p.folderId === folderId).map(p => ({ id: p.id, position: p.position ?? 0 })))
    const known = new Set(library.folders.map(f => f.id))
    return sortCustom([
      ...library.folders.map(f => ({ id: f.id, position: f.position })),
      ...visible.value.filter(p => !p.folderId || !known.has(p.folderId)).map(p => ({ id: p.id, position: p.position ?? 0 })),
    ])
  }
  const sortCustom = (xs: { id: string, position: number }[]) => [...xs].sort((a, b) => a.position - b.position || a.id.localeCompare(b.id)).map(x => x.id)

  async function onDrop(target: TreeNode, e: DragEvent) {
    e.preventDefault()
    const d = dragging.value
    const zone = over.value?.id === target.id ? over.value.zone : zoneOf(target, e)
    onDragEnd()
    if (!d || !zone) return
    // Куда: уровень соседа (у плейлиста в папке — его папка), внутрь — в конец папки
    const dest = zone === 'into' ? target.id : target.kind === 'playlist' ? (target.playlist.folderId && folderOf(target.playlist.folderId) ? target.playlist.folderId : null) : null
    if (d.kind === 'folder' && dest) return
    let body: { afterId?: string | null, atStart?: boolean } = {}
    if (zone !== 'into' && sort.value === 'custom') {
      // Перед соседом — значит после того, кто перед ним (без самой перетаскиваемой строки)
      const sib = level(dest).filter(id => id !== d.id)
      const at = sib.indexOf(target.kind === 'playlist' && target.playlist.folderId && !dest ? target.playlist.folderId : target.id)
      if (zone === 'after') body = { afterId: sib[at] ?? null }
      else body = at > 0 ? { afterId: sib[at - 1] } : { atStart: true }
    }
    const list = await guard(() => (d.kind === 'folder'
      ? bridge.playlists.moveFolder(d.id, body)
      : bridge.playlists.move(d.id, { folderId: dest, ...body })))
    if (list) apply(list)
  }

  function dropClass(id: string) {
    const o = over.value
    if (!o || o.id !== id) return null
    return o.zone === 'into' ? 'is-drop' : o.zone === 'before' ? 'is-drop-before' : 'is-drop-after'
  }

  const SORTS: PlaylistSort[] = ['recent', 'title', 'custom']
  const sortItems = computed<MenuEntry[]>(() => SORTS.map(s => ({ label: t(`plSort.${s}`), checked: sort.value === s, onSelect: () => (sort.value = s) })))

  return {
    sort, sortItems, tree, childrenOf, folderOf, sortPlaylists,
    createFolder, renameFolder, deleteFolder, toggleFolder, folderMenu, moveTo, moveMenu,
    dragging, over, onDragStart, onDragEnd, onDragOver, onDragLeave, onDrop, dropClass,
  }
}
