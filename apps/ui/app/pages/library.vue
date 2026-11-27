<script setup lang="ts">
import type { FollowedArtist, PlaylistSummary, SavedAlbum, Track, UpcomingRelease } from '~/api/types'
import type { TreeNode } from '~/composables/usePlaylistTree'
import type { LibraryItem } from '~/components/library/Entry.vue'
import type { MenuEntry } from '~/types/ui'

// Медиатека как «Your Library» в Spotify и Apple Music: цветной ряд «Любимые треки», «Моя волна»,
// «Недавно добавленные»; ниже — всё сохранённое одним списком с фильтрами, сортировкой и видом
// «сетка/список». Закреплённые плейлисты всегда первыми
type Filter = 'playlists' | 'albums' | 'artists' | 'downloaded' | 'recent'
type View = 'grid' | 'list'

// «Скачанное» — только в приложении: закреплённое на этом устройстве
const BASE_FILTERS: Filter[] = ['playlists', 'albums', 'artists']
const KIND_OF: Record<Exclude<Filter, 'downloaded' | 'recent'>, LibraryItem['kind'][]> = { playlists: ['playlist', 'folder'], albums: ['album'], artists: ['artist'] }

const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const library = useLibraryStore()
const player = usePlayerStore()
const auth = useAuthStore()
const toast = useToast()
const ui = useUiStore()
const offline = useOfflineStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()
const { playAlbum, playArtist } = usePlayActions()
const pla = usePlaylistActions()
const pui = usePlaylistUi()
const tree = usePlaylistTree()
useHead({ title: () => t('title') })

// Открытая папка (?folder=…): её плейлисты вместо всей медиатеки
const folderId = computed(() => (typeof route.query.folder === 'string' ? route.query.folder : null))
const openFolder = computed(() => tree.folderOf(folderId.value))
function closeFolder() {
  void router.replace({ query: { ...route.query, folder: undefined } })
}
// Папку удалили (здесь или на другом устройстве) — назад к медиатеке
watch([openFolder, () => library.playlists], ([f, list]) => {
  if (folderId.value && list && !f) closeFolder()
})

const FILTERS = computed<Filter[]>(() => (offline.available ? [...BASE_FILTERS, 'downloaded'] : BASE_FILTERS))

// Фильтр живёт в адресе (?tab=albums): ссылки из других мест открывают нужный раздел
const filter = computed<Filter | null>({
  // «Недавно добавленные» — без своего чипа: открывается плиткой, виден как включённый чип с крестиком
  get: () => ([...FILTERS.value, 'recent'] as string[]).includes(String(route.query.tab)) ? route.query.tab as Filter : null,
  set: v => void router.replace({ query: { ...route.query, tab: v ?? undefined } }),
})
function toggleFilter(f: Filter) {
  filter.value = filter.value === f ? null : f
}

// Выбранный фильтр на узком экране не прячется за краем ленты
const chipsEl = ref<HTMLElement | null>(null)
watch(filter, () => nextTick(() => chipsEl.value?.querySelector('.is-on')?.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })))
onMounted(() => chipsEl.value?.querySelector('.is-on')?.scrollIntoView({ block: 'nearest', inline: 'nearest' }))

// Сортировка и вид запоминаются на устройстве
function stored<T extends string>(key: string, allowed: readonly T[], fallback: () => T) {
  const v = ref<T>(fallback())
  onMounted(() => {
    try {
      const s = localStorage.getItem(key)
      if (s && (allowed as readonly string[]).includes(s)) v.value = s as T
    }
    catch {
      // Без хранилища — значения по умолчанию
    }
  })
  watch(v, val => {
    try {
      localStorage.setItem(key, val)
    }
    catch {
      // Не страшно: выбор просто не переживёт перезапуск
    }
  })
  return v
}
// Порядок — настройка слушателя (общая для устройств): «Недавние», «По названию», «Свой порядок»
const sort = tree.sort
const view = stored<View>('dw.library.view', ['grid', 'list'], () => (ui.isPhone ? 'list' : 'grid'))

const albums = ref<SavedAlbum[] | null>(null)
const artists = ref<FollowedArtist[] | null>(null)
const failed = ref(false)
// «Скоро выйдет»: сохранённое заранее и анонсы подписок; не загрузилось — полки просто нет
const upcoming = ref<UpcomingRelease[]>([])
const allUpcoming = ref(false)
const shownUpcoming = computed(() => (allUpcoming.value || upcoming.value.length <= 4 ? upcoming.value : upcoming.value.slice(0, 3)))

async function load() {
  failed.value = false
  try {
    await Promise.all([
      library.loadOverview(true),
      library.loadPlaylists(true),
      bridge.library.savedAlbums().then(p => (albums.value = p.items)),
      bridge.library.followed().then(p => (artists.value = p.items)),
      bridge.library.upcoming().then(p => (upcoming.value = p.items)).catch(() => {}),
    ])
  }
  catch {
    failed.value = true
  }
}
onMounted(load)
watch(() => library.revision, load)

const ready = computed(() => !!library.playlists && !!albums.value && !!artists.value)

// ---------- Всё сохранённое одним списком ----------


function playlistItem(p: PlaylistSummary): LibraryItem {
  return {
    key: `playlist:${p.id}`,
    kind: 'playlist',
    id: p.id,
    title: p.title,
    sub: pla.subOf(p),
    short: p.tracksCount ? formatCount(p.tracksCount, 'track', lang.value) : pla.subOf(p),
    pinned: p.pinned,
    added: p.createdAt,
    recent: p.playedAt && p.playedAt > p.updatedAt ? p.playedAt : p.updatedAt,
    playlist: p,
  }
}

/** Папки и плейлисты вне папок — в порядке дерева (он же «Свой порядок») */
const listItems = computed<LibraryItem[]>(() => tree.tree.value.map((n) => {
  if (n.kind === 'playlist') return playlistItem(n.playlist)
  const recent = [n.folder.updatedAt, ...n.children.map(p => (p.playedAt && p.playedAt > p.updatedAt ? p.playedAt : p.updatedAt))].sort().at(-1)
  return {
    key: `folder:${n.id}`,
    kind: 'folder',
    id: n.id,
    title: n.folder.title,
    sub: n.children.length ? `${tg('folder.kind')} · ${formatCount(n.children.length, 'playlist', lang.value)}` : tg('folder.empty'),
    short: n.children.length ? formatCount(n.children.length, 'playlist', lang.value) : tg('folder.empty'),
    pinned: false,
    added: n.folder.createdAt,
    recent,
    folder: n.folder,
  }
}))

const items = computed<LibraryItem[]>(() => {
  if (openFolder.value) return tree.childrenOf(openFolder.value.id).map(playlistItem)
  const out: LibraryItem[] = [...listItems.value]
  // Убранное только что (из меню) исчезает сразу, не дожидаясь перезагрузки
  for (const s of albums.value ?? []) {
    if (!library.isSaved(s.album)) continue
    out.push({
      key: `album:${s.album.id}`,
      kind: 'album',
      id: s.album.id,
      title: s.album.title,
      sub: `${tg(`record.${s.album.recordType}`)} · ${s.album.artist.name}`,
      short: s.album.artist.name,
      pinned: false,
      added: s.savedAt,
      image: s.album.image,
      explicit: s.album.explicit,
    })
  }
  for (const f of artists.value ?? []) {
    if (!library.isFollowed(f.artist)) continue
    out.push({
      key: `artist:${f.artist.id}`,
      kind: 'artist',
      id: f.artist.id,
      title: f.artist.name,
      sub: tg('record.artist'),
      pinned: false,
      added: f.followedAt,
      image: f.artist.image,
    })
  }
  return out
})

const collator = computed(() => new Intl.Collator(lang.value, { sensitivity: 'base', numeric: true }))
// Плейлисты и папки уже в порядке дерева; в «Своём порядке» альбомы и артисты — следом, новые сверху
const sorted = computed(() => {
  if (sort.value === 'custom' || openFolder.value) {
    const lists = items.value.filter(i => i.kind === 'playlist' || i.kind === 'folder')
    const rest = items.value.filter(i => i.kind === 'album' || i.kind === 'artist').sort((a, b) => b.added.localeCompare(a.added))
    return [...lists, ...rest]
  }
  return [...items.value].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1
    if (sort.value === 'title') return collator.value.compare(a.title, b.title)
    return (b.recent ?? b.added).localeCompare(a.recent ?? a.added)
  })
})

const visible = computed(() => {
  const f = filter.value
  if (!f) return sorted.value
  if (f === 'recent') return newest.value.slice(0, RECENT_LIMIT)
  if (f === 'downloaded') {
    // Альбом из медиатеки и он же, догруженный отдельно, — один раз (догрузка может прийти позже списка)
    const seen = new Set<string>()
    return [...sorted.value.filter(i => i.kind !== 'folder' && offline.isPinned(i.kind, i.id)), ...pinnedOutside.value].filter(i => !seen.has(i.key) && !!seen.add(i.key))
  }
  return sorted.value.filter(i => KIND_OF[f].includes(i.kind))
})

// ---------- Скачанное (приложение) ----------

const likedPinned = computed(() => offline.isPinned('liked', null))
const pinnedTrackIds = computed(() => (offline.status?.pins ?? []).filter(p => p.kind === 'track' && p.refId).map(p => p.refId!))
const pinnedTracks = ref<Track[]>([])
// Закреплённые альбомы, которых нет в медиатеке (скачаны со страницы альбома), — тоже здесь
const pinnedOutside = ref<LibraryItem[]>([])
const pinnedAlbumIds = computed(() => (offline.status?.pins ?? []).filter(p => p.kind === 'album' && p.refId).map(p => p.refId!))
watch([filter, pinnedAlbumIds, items], async ([f, ids, list]) => {
  const missing = f === 'downloaded' ? ids.filter(id => !list.some(i => i.kind === 'album' && i.id === id)) : []
  const loaded = await Promise.all(missing.map(id => bridge.catalog.album(id).catch(() => null)))
  pinnedOutside.value = loaded.filter(d => !!d).map(d => ({
    key: `album:${d!.album.id}`,
    kind: 'album',
    id: d!.album.id,
    title: d!.album.title,
    sub: `${tg(`record.${d!.album.recordType}`)} · ${d!.album.artist.name}`,
    short: d!.album.artist.name,
    pinned: false,
    added: new Date().toISOString(),
    image: d!.album.image,
    explicit: d!.album.explicit,
  }))
}, { immediate: true })
watch([filter, pinnedTrackIds], async ([f, ids]) => {
  if (f !== 'downloaded' || !ids.length) {
    pinnedTracks.value = []
    return
  }
  pinnedTracks.value = await bridge.catalog.lookup(ids).catch(() => [])
}, { immediate: true })
const deviceSummary = computed(() => {
  const s = offline.status
  if (!s) return ''
  // Только скачанное вручную: умный запас волны живёт сам по себе и здесь не показывается
  return t('deviceSize', { size: formatBytes(s.usage.pinnedBytes, lang.value) })
})
const downloadedEmpty = computed(() => !visible.value.length && !likedPinned.value && !pinnedTracks.value.length)

const newest = computed(() => [
  ...(library.playlists ?? []).filter(p => !pui.hidden.has(p.id)).map(playlistItem),
  ...items.value.filter(i => i.kind === 'album'),
].sort((a, b) => b.added.localeCompare(a.added)))
const recentCovers = computed(() => newest.value.slice(0, 3).map(i => i.image ?? i.playlist?.covers[0]).filter(c => !!c))

const showTiles = computed(() => !failed.value && !openFolder.value && (filter.value === null || filter.value === 'playlists'))

function dateOf(i: LibraryItem) {
  return ui.isPhone ? undefined : formatShortDate(i.added, lang.value)
}

// ---------- Плитки ----------

const waveOn = computed(() => player.waveActive && player.playing)
function onWave() {
  if (player.waveActive && player.current) return player.toggle()
  return player.startWave()
}

const listEl = ref<HTMLElement | null>(null)
// «Недавно добавленные»: отдельный срез — последние сохранённые альбомы и плейлисты, новые сверху
const RECENT_LIMIT = 40
function showRecent() {
  filter.value = 'recent'
}

// ---------- Сортировка ----------

const sortItems = computed<MenuEntry[]>(() => [
  { kind: 'heading', label: t('sortBy') },
  ...tree.sortItems.value,
])

// Перетаскивание строк (компьютер, список): в папку, из неё и в «Своём порядке» — на новое место
const canDrag = computed(() => !ui.isTouchUi && view.value === 'list' && filter.value !== 'recent' && filter.value !== 'downloaded')
function nodeOf(i: LibraryItem): TreeNode | null {
  if (i.kind === 'folder' && i.folder) return { kind: 'folder', id: i.id, folder: i.folder, children: tree.childrenOf(i.id) }
  if (i.kind === 'playlist' && i.playlist) return { kind: 'playlist', id: i.id, playlist: i.playlist }
  return null
}
function dragAttrs(i: LibraryItem) {
  const n = canDrag.value ? nodeOf(i) : null
  if (!n) return {}
  return {
    draggable: 'true',
    class: tree.dropClass(n.id),
    onDragstart: (e: DragEvent) => tree.onDragStart(n.kind, n.id, e),
    onDragend: () => tree.onDragEnd(),
    onDragover: (e: DragEvent) => tree.onDragOver(n, e),
    onDragleave: () => tree.onDragLeave(n),
    onDrop: (e: DragEvent) => void tree.onDrop(n, e),
  }
}

// ---------- Меню элемента: правый щелчок или долгое нажатие ----------

const menuItems = ref<MenuEntry[]>([])
function onContext(target: EventTarget | null) {
  const key = (target as HTMLElement | null)?.closest<HTMLElement>('[data-key]')?.dataset.key
  const item = key ? items.value.find(i => i.key === key) : undefined
  if (item?.kind === 'folder' && item.folder) {
    menuItems.value = tree.folderMenu(item.folder)
    return
  }
  menuItems.value = item ? menuFor(item) : []
}

function menuFor(i: LibraryItem): MenuEntry[] {
  const head: MenuEntry = { kind: 'heading', label: i.title, hint: i.sub, image: i.image ?? i.playlist?.image ?? i.playlist?.covers[0] ?? null, round: i.kind === 'artist' }
  if (i.kind === 'playlist' && i.playlist) return pla.menuFor(i.playlist)
  if (i.kind === 'album') {
    const s = albums.value?.find(a => a.album.id === i.id)
    return [
      head,
      { label: tg('common.play'), icon: 'ph:play', onSelect: () => playAlbum(i.id) },
      { label: tg('common.shuffle'), icon: 'ph:shuffle', onSelect: () => playAlbum(i.id, { shuffle: true }) },
      { kind: 'separator' },
      { label: t('removeAlbum'), icon: 'ph:minus-circle', danger: true, onSelect: () => s && library.setSaved(s.album, false) },
    ]
  }
  const f = artists.value?.find(a => a.artist.id === i.id)
  return [
    head,
    { label: tg('common.play'), icon: 'ph:play', onSelect: () => playArtist(i.id) },
    { kind: 'separator' },
    { label: t('unfollow'), icon: 'ph:bell-slash', danger: true, onSelect: () => f && library.setFollowed(f.artist, false) },
  ]
}

// ---------- Новый плейлист ----------

const creating = ref(false)
const newTitle = ref('')
const createOpen = ref(false)
async function create() {
  const title = newTitle.value.trim()
  if (!title || creating.value) return
  creating.value = true
  try {
    const p = await bridge.playlists.create({ title })
    createOpen.value = false
    newTitle.value = ''
    void library.loadPlaylists(true)
    toast.show({ text: t('created', { title }), icon: 'ph:playlist' })
    await navigateTo(`/playlist/${p.id}`)
  }
  catch {
    toast.error(t('createFailed'))
  }
  finally {
    creating.value = false
  }
}

function byDescription() {
  createOpen.value = false
  void navigateTo('/describe')
}

async function newFolder() {
  createOpen.value = false
  const id = await tree.createFolder()
  if (id) void router.push({ query: { ...route.query, tab: 'playlists', folder: id } })
}

const initial = computed(() => (auth.user?.name ?? '?').trim().charAt(0).toUpperCase())
const friends = useFriendsStore()
</script>

<template>
  <div class="lib">
    <!-- Открытая папка: «назад», название, меню папки -->
    <header v-if="openFolder" class="lib__head lib__head--folder">
      <UiIconButton icon="ph:caret-left" :label="t('back')" :size="ui.isTouchUi ? 'lg' : 'md'" :tooltip="false" class="lib__back" @click="closeFolder" />
      <h1 class="lib__title lib__title--folder ellipsis">{{ openFolder.title }}</h1>
      <UiMenu :items="tree.folderMenu(openFolder)" align="end">
        <UiIconButton icon="ph:dots-three" :label="tg('common.more')" :size="ui.isTouchUi ? 'lg' : 'md'" :tooltip="false" />
      </UiMenu>
    </header>
    <header v-else class="lib__head">
      <NuxtLink v-if="ui.isTouchUi" to="/settings" class="lib__avatar" :aria-label="tg('nav.settings')">
        <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="">
        <template v-else>{{ initial }}</template>
      </NuxtLink>
      <h1 class="lib__title">{{ t('title') }}</h1>
      <!-- Новый плейлист: на телефоне — лист снизу, на компьютере — всплывающая панель у кнопки -->
      <!-- «Недавно прослушанное» -->
      <UiIconButton icon="ph:clock-counter-clockwise" :label="tg('nav.history')" :size="ui.isTouchUi ? 'lg' : 'sm'" :tooltip="ui.isTouchUi ? false : undefined" to="/history" class="lib__hist" />
      <!-- Друзья: что слушают, запросы; точка — ждут ответа -->
      <span v-if="ui.isTouchUi" class="lib__friends">
        <UiIconButton icon="ph:users-three" :label="tg('friends.title')" size="lg" :tooltip="false" to="/friends" />
        <span v-if="friends.incoming.length" class="lib__dot" aria-hidden="true" />
      </span>
      <template v-if="ui.isTouchUi">
        <UiIconButton icon="ph:plus" :label="t('newPlaylist')" size="lg" :tooltip="false" class="lib__plus" @click="createOpen = true" />
        <UiSheet v-model:open="createOpen" :title="t('newPlaylist')">
          <form class="newpl newpl--sheet" @submit.prevent="create">
            <input v-model="newTitle" class="newpl__input" :placeholder="t('newPlaceholder')" :aria-label="t('newPlaylist')" maxlength="120" enterkeyhint="done" autofocus>
            <UiButton type="submit" variant="primary" size="lg" :loading="creating" :disabled="!newTitle.trim()" block>{{ t('create') }}</UiButton>
            <UiButton size="lg" icon="ph:folder-simple-plus" block @click="newFolder">{{ tg('folder.new') }}</UiButton>
            <UiButton size="lg" icon="ph:note-pencil" block @click="byDescription">{{ t('byDescription') }}</UiButton>
          </form>
        </UiSheet>
      </template>
      <UiPopover v-else v-model:open="createOpen" :title="t('newPlaylist')" side="bottom" align="end" :width="300">
        <template #trigger>
          <UiButton size="sm" icon="ph:plus">{{ t('newPlaylist') }}</UiButton>
        </template>
        <form class="newpl" @submit.prevent="create">
          <input v-model="newTitle" class="newpl__input" :placeholder="t('newPlaceholder')" :aria-label="t('newPlaylist')" maxlength="120" autofocus>
          <UiButton type="submit" variant="primary" :loading="creating" :disabled="!newTitle.trim()" block>{{ t('create') }}</UiButton>
          <UiButton icon="ph:note-pencil" block @click="byDescription">{{ t('byDescription') }}</UiButton>
        </form>
      </UiPopover>
      <UiIconButton v-if="!ui.isTouchUi" icon="ph:folder-simple-plus" :label="tg('folder.new')" size="sm" class="lib__folder" @click="newFolder" />
    </header>

    <!-- Фильтры: повторное нажатие или крестик возвращают всё -->
    <div v-if="!openFolder" ref="chipsEl" v-edge-fade class="chips no-scrollbar" role="toolbar" :aria-label="t('filters')">
      <button v-if="filter" type="button" class="chip chip--clear" :aria-label="t('showAll')" @click="filter = null">
        <Icon name="ph:x-bold" />
      </button>
      <button v-if="filter === 'recent'" type="button" class="chip is-on" aria-pressed="true" @click="filter = null">
        {{ t('filter.recent') }}
      </button>
      <button
        v-for="f in FILTERS"
        :key="f"
        type="button"
        class="chip"
        :class="{ 'is-on': filter === f }"
        :aria-pressed="filter === f"
        @click="toggleFilter(f)"
      >
        {{ t(`filter.${f}`) }}
      </button>
    </div>

    <UiEmptyState v-if="failed" icon="ph:cloud-warning" :title="tg('common.loadFailed')" :text="tg('common.loadFailedText')" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="load">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>

    <template v-else>
      <div v-if="showTiles" class="tiles" :class="{ 'tiles--two': !newest.length }">
        <LibraryTile tone="liked" icon="ph:heart-fill" :title="tg('nav.liked')" :meta="formatCount(library.likedCount, 'track', lang)" to="/liked" />
        <LibraryTile tone="wave" :icon="waveOn ? 'ph:pause-fill' : 'ph:play-fill'" :title="t('wave')" :meta="waveOn ? t('wavePlaying') : t('waveListen')" :active="player.waveActive" @click="onWave" />
        <LibraryTile v-if="newest.length" tone="recent" icon="ph:sparkle-fill" :title="t('recent')" :meta="newest[0]?.title" :covers="recentCovers" @click="showRecent" />
      </div>
      <RewindCard v-if="filter === null && !openFolder" class="lib__rewind" />

      <section v-if="upcoming.length && (filter === null || filter === 'albums') && !openFolder" class="soon" :aria-label="t('soon')">
        <header class="soon__head">
          <h2 class="soon__h2">{{ t('soon') }}</h2>
          <UiButton v-if="upcoming.length > 4" variant="ghost" size="sm" @click="allUpcoming = !allUpcoming">{{ allUpcoming ? t('soonLess') : t('soonAll') }}</UiButton>
        </header>
        <MusicUpcomingRelease v-for="r in shownUpcoming" :key="r.album.id" :release="r" show-artist />
      </section>

      <!-- Скачанное: сколько на устройстве, все лайки, отдельные треки -->
      <div v-if="filter === 'downloaded' && offline.status && !downloadedEmpty" class="dl">
        <p class="dl__sum">
          <Icon name="ph:arrow-circle-down-fill" class="dl__icon" />
          <span class="ellipsis">{{ deviceSummary }}</span>
          <NuxtLink to="/settings#downloads" class="dl__link">{{ t('downloadSettings') }}</NuxtLink>
        </p>
        <div v-if="likedPinned" class="tiles">
          <LibraryTile tone="liked" icon="ph:heart-fill" :title="tg('nav.liked')" :meta="formatCount(library.likedCount, 'track', lang)" to="/liked" />
        </div>
        <section v-if="pinnedTracks.length" class="dl__tracks">
          <h2 class="dl__h2">{{ t('tracks') }}</h2>
          <MusicTrackList :tracks="pinnedTracks" variant="wide" :context="{ type: 'library', id: 'downloaded', title: t('filter.downloaded') }" :label="t('tracks')" />
        </section>
      </div>

      <section ref="listEl" class="list" :aria-label="filter ? t(`filter.${filter}`) : t('all')">
        <!-- Сортировка слева, вид справа -->
        <div v-if="visible.length" class="list__bar">
          <span v-if="filter === 'recent'" class="sortbtn sortbtn--static">{{ t('sort.added') }}</span>
          <UiMenu v-else :items="sortItems" align="start">
            <button type="button" class="sortbtn">
              <Icon name="ph:arrows-down-up" />{{ tg(`plSort.${sort}`) }}
            </button>
          </UiMenu>
          <span class="list__gap" />
          <UiIconButton
            :icon="view === 'grid' ? 'ph:rows' : 'ph:squares-four'"
            :label="view === 'grid' ? t('asList') : t('asGrid')"
            :tooltip="ui.isTouchUi ? false : undefined"
            class="list__view"
            @click="view = view === 'grid' ? 'list' : 'grid'"
          />
        </div>

        <UiContextMenu v-if="visible.length" :items="menuItems" @open="onContext">
          <div class="items" :class="`items--${view}`">
            <LibraryEntry v-for="i in visible" :key="i.key" :item="i" :layout="view" :date="view === 'list' ? dateOf(i) : undefined" v-bind="dragAttrs(i)" />
          </div>
        </UiContextMenu>

        <!-- Загрузка -->
        <div v-else-if="!ready && filter !== 'downloaded'" class="items" :class="`items--${view}`" aria-hidden="true">
          <template v-if="view === 'grid'">
            <UiSkeleton v-for="n in 6" :key="n" class="skel-grid" />
          </template>
          <div v-for="n in 6" v-else :key="n" class="skel-row">
            <UiSkeleton class="skel-row__art" />
            <div class="skel-row__text"><UiSkeleton class="skel-row__l1" /><UiSkeleton class="skel-row__l2" /></div>
          </div>
        </div>

        <!-- Пусто: подсказываем следующий шаг -->
        <UiEmptyState v-else-if="filter === 'downloaded' && downloadedEmpty" icon="ph:arrow-circle-down" :title="t('empty.downloaded')" :text="t('empty.downloadedText')">
          <UiButton to="/settings#downloads" icon="ph:gear-six">{{ t('downloadSettings') }}</UiButton>
        </UiEmptyState>
        <UiEmptyState v-else-if="filter === 'albums'" icon="ph:vinyl-record" :title="t('empty.albums')" :text="t('empty.albumsText')">
          <UiButton to="/search" icon="ph:magnifying-glass">{{ t('find') }}</UiButton>
        </UiEmptyState>
        <UiEmptyState v-else-if="filter === 'artists'" icon="ph:user-circle-plus" :title="t('empty.artists')" :text="t('empty.artistsText')">
          <UiButton to="/search" icon="ph:magnifying-glass">{{ t('find') }}</UiButton>
        </UiEmptyState>
        <UiEmptyState v-else-if="openFolder" icon="ph:folder-simple" :title="tg('folder.empty')" :text="t('empty.folderText')" />
        <UiEmptyState v-else icon="ph:playlist" :title="t('empty.playlists')" :text="t('empty.playlistsText')">
          <UiButton icon="ph:plus" @click="createOpen = true">{{ t('newPlaylist') }}</UiButton>
        </UiEmptyState>
      </section>

      <!-- Умный запас волны — тихой строкой внизу «Скачанного», сам список на отдельной странице -->
      <NuxtLink v-if="filter === 'downloaded' && (offline.status?.reserve.ready ?? 0) > 0" to="/downloads/reserve" class="rsvrow">
        <span class="ellipsis">{{ t('reserve', { n: formatCount(offline.status!.reserve.ready, 'track', lang) }) }}</span>
        <Icon name="ph:caret-right" class="rsvrow__chev" aria-hidden="true" />
      </NuxtLink>
    </template>
  </div>
</template>

<style scoped>
.lib { display: flex; flex-direction: column; gap: 20px; padding-top: 16px; }

.lib__head { display: flex; align-items: center; gap: 12px; min-height: 44px; }
.lib__head--folder { gap: 4px; }
.lib__back { margin-left: -10px; }
.lib__title.lib__title--folder { font-size: 28px; }
.lib__title { flex: 1; min-width: 0; margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.lib__avatar { display: grid; place-items: center; width: 36px; height: 36px; flex: none; overflow: hidden; border-radius: 50%; background: var(--fill-3); box-shadow: inset 0 0 0 1px var(--hair); font-size: 15px; font-weight: 700; }
.lib__avatar img { width: 100%; height: 100%; object-fit: cover; }
.lib__avatar:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.lib__friends { position: relative; display: inline-grid; flex: none; }
.lib__dot { position: absolute; top: 9px; right: 9px; width: 8px; height: 8px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 2px var(--ink-1); pointer-events: none; }

.newpl { display: flex; flex-direction: column; gap: 8px; padding: 0 6px 6px; }
.newpl__input { height: 40px; padding: 0 14px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.newpl__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.newpl--sheet { gap: 12px; padding: 4px 16px 16px; }
.newpl--sheet .newpl__input { height: 48px; padding: 0 16px; font-size: 16px; }

/* Фильтры — как в Spotify: заливка, выбранный — светлый */
.chips { display: flex; align-items: center; gap: 8px; min-width: 0; margin-block: -6px; padding-block: 6px; overflow-x: auto; overscroll-behavior-x: contain; }
.chip {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 32px;
  flex: none;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-2);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 550;
  color: var(--text-1);
  white-space: nowrap;
  transition: background-color var(--dur-1), color var(--dur-1), transform var(--dur-1);
}
/* Зона нажатия 44 pt при высоте 32 */
.chip::before { content: ''; position: absolute; inset: -6px -2px; }
.chip:hover { background: var(--fill-3); }
.chip:active { transform: scale(0.97); }
.chip:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.chip.is-on { background: var(--text-1); color: var(--on-light); }
.chip--clear { width: 32px; padding: 0; font-size: 14px; color: var(--text-2); }

.tiles { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; }
/* Пустая медиатека: «Недавно добавленных» нет — две плитки на всю ширину */
.tiles--two { grid-template-columns: repeat(2, minmax(0, 1fr)); }

.dl { display: flex; flex-direction: column; gap: 16px; }
.dl__sum { display: flex; align-items: center; gap: 8px; min-width: 0; margin: 0; font-size: var(--fs-small); color: var(--text-2); }
.dl__icon { flex: none; font-size: 18px; color: var(--accent); }
.dl__link { flex: none; margin-left: auto; color: var(--text-1); font-weight: 600; }
.dl__link:hover { text-decoration: underline; text-underline-offset: 3px; }
.dl__link:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: 4px; }
.dl__tracks { display: flex; flex-direction: column; gap: 8px; }
.rsvrow { display: flex; align-items: center; gap: 8px; min-height: 48px; padding: 4px 0; border-top: 1px solid var(--hair); font-size: var(--fs-small); color: var(--text-2); transition: color var(--dur-1); }
.rsvrow:hover { color: var(--text-1); }
.rsvrow:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: 4px; }
.rsvrow__chev { flex: none; margin-left: auto; font-size: 14px; color: var(--text-3); }
.dl__h2 { margin: 0; font-size: var(--fs-h4, 18px); font-weight: 700; }
.soon { display: flex; flex-direction: column; }
.soon__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 36px; }
.soon__h2 { margin: 0; font-size: var(--fs-h4, 18px); font-weight: 700; }

.list { display: flex; flex-direction: column; gap: 12px; scroll-margin-top: 72px; }
.list__bar { display: flex; align-items: center; gap: 12px; min-height: 40px; }
.list__gap { flex: 1; }
.list__view { margin-right: -10px; }

.sortbtn { display: inline-flex; align-items: center; gap: 8px; height: 36px; margin-left: -10px; padding: 0 10px; border: 0; border-radius: var(--r-pill); background: transparent; font: inherit; font-size: var(--fs-small); font-weight: 600; color: var(--text-1); transition: background-color var(--dur-1); }
.sortbtn :deep(svg) { font-size: 16px; color: var(--text-2); }
.sortbtn:hover, .sortbtn[data-state='open'] { background: var(--fill-1); }
.sortbtn:focus-visible { outline: 2px solid var(--focus); outline-offset: 0; }

.items--grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(168px, 1fr)); gap: 28px 24px; align-items: start; }
.items--list { display: flex; flex-direction: column; }

.skel-grid { aspect-ratio: 1; border-radius: var(--r-3); }
.skel-row { display: flex; align-items: center; gap: 14px; min-height: 64px; }
.skel-row__art { width: 48px; height: 48px; border-radius: var(--r-2); }
.skel-row__text { display: flex; flex: 1; flex-direction: column; gap: 6px; }
.skel-row__l1 { width: 40%; height: 14px; border-radius: 4px; }
.skel-row__l2 { width: 24%; height: 12px; border-radius: 4px; }

@media (max-width: 699px) {
  .lib { gap: 16px; padding: 8px 16px 0; }
  .lib__title { font-size: 28px; }
  .lib__title.lib__title--folder { font-size: 22px; }
  .lib__plus { margin-right: -6px; }
  .chips { --fade-w: 16px; margin-inline: -16px; padding-inline: 16px; scroll-padding-inline: 16px; }
  .tiles { gap: 10px; }
  .tiles--two :deep(.tile) { aspect-ratio: auto; min-height: 120px; }
  .list { gap: 4px; }
  .items--grid { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 20px 12px; }
  .skel-row { min-height: 72px; }
  .skel-row__art { width: 60px; height: 60px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Медиатека",
    "all": "Всё",
    "filters": "Что показать",
    "showAll": "Показать всё",
    "filter": { "playlists": "Плейлисты", "albums": "Альбомы", "artists": "Артисты", "downloaded": "Скачанное", "recent": "Недавно добавленные" },
    "kind": { "playlist": "Плейлист", "mix": "Микс", "smart": "Подборка" },
    "wave": "Моя волна",
    "waveListen": "Слушать",
    "wavePlaying": "Играет",
    "recent": "Недавно добавленные",
    "soon": "Скоро выйдет",
    "soonAll": "Все",
    "soonLess": "Свернуть",
    "sortBy": "Сортировка",
    "sort": { "added": "Сначала новые", "title": "По названию" },
    "back": "Медиатека",
    "asList": "Списком",
    "asGrid": "Сеткой",
    "pin": "Закрепить",
    "unpin": "Открепить",
    "removeAlbum": "Убрать из медиатеки",
    "unfollow": "Отписаться",
    "newPlaylist": "Новый плейлист",
    "byDescription": "Плейлист по описанию",
    "newPlaceholder": "Название",
    "create": "Создать",
    "created": "Плейлист «{title}» создан",
    "createFailed": "Не получилось создать плейлист — попробуйте ещё раз",
    "find": "Найти музыку",
    "deviceSize": "На устройстве: {size}",
    "downloadSettings": "Загрузки",
    "reserve": "Запас для волны · {n}",
    "tracks": "Треки",
    "empty": {
      "playlists": "Соберите первый плейлист",
      "playlistsText": "Любимое под рукой — в одном месте",
      "albums": "Здесь будут ваши альбомы",
      "albumsText": "Сохраняйте альбомы, которые хочется переслушивать",
      "artists": "Здесь будут ваши артисты",
      "artistsText": "Подпишитесь — новые релизы появятся сами",
      "downloaded": "Скачанного пока нет",
      "downloadedText": "Музыка, сохранённая на устройство, будет здесь",
      "folderText": "Плейлисты попадают сюда через «Переместить в папку» в их меню"
    }
  },
  "uk": {
    "title": "Медіатека",
    "all": "Усе",
    "filters": "Що показати",
    "showAll": "Показати все",
    "filter": { "playlists": "Плейлисти", "albums": "Альбоми", "artists": "Виконавці", "downloaded": "Завантажене", "recent": "Нещодавно додані" },
    "kind": { "playlist": "Плейлист", "mix": "Мікс", "smart": "Добірка" },
    "wave": "Моя хвиля",
    "waveListen": "Слухати",
    "wavePlaying": "Грає",
    "recent": "Нещодавно додані",
    "soon": "Незабаром вийде",
    "soonAll": "Усі",
    "soonLess": "Згорнути",
    "sortBy": "Сортування",
    "sort": { "added": "Спочатку нові", "title": "За назвою" },
    "back": "Медіатека",
    "asList": "Списком",
    "asGrid": "Сіткою",
    "pin": "Закріпити",
    "unpin": "Відкріпити",
    "removeAlbum": "Прибрати з медіатеки",
    "unfollow": "Відписатися",
    "newPlaylist": "Новий плейлист",
    "byDescription": "Плейлист за описом",
    "newPlaceholder": "Назва",
    "create": "Створити",
    "created": "Плейлист «{title}» створено",
    "createFailed": "Не вдалося створити плейлист — спробуйте ще раз",
    "find": "Знайти музику",
    "deviceSize": "На пристрої: {size}",
    "downloadSettings": "Завантаження",
    "reserve": "Запас для хвилі · {n}",
    "tracks": "Треки",
    "empty": {
      "playlists": "Зберіть перший плейлист",
      "playlistsText": "Улюблене під рукою — в одному місці",
      "albums": "Тут будуть ваші альбоми",
      "albumsText": "Зберігайте альбоми, які хочеться переслуховувати",
      "artists": "Тут будуть ваші виконавці",
      "artistsText": "Підпишіться — нові релізи з’являться самі",
      "downloaded": "Завантаженого поки немає",
      "downloadedText": "Музика, збережена на пристрій, буде тут",
      "folderText": "Плейлисти потрапляють сюди через «Перемістити в папку» в їхньому меню"
    }
  },
  "en": {
    "title": "Library",
    "all": "All",
    "filters": "Show",
    "showAll": "Show all",
    "filter": { "playlists": "Playlists", "albums": "Albums", "artists": "Artists", "downloaded": "Downloaded", "recent": "Recently added" },
    "kind": { "playlist": "Playlist", "mix": "Mix", "smart": "Smart playlist" },
    "wave": "My Wave",
    "waveListen": "Listen",
    "wavePlaying": "Playing",
    "recent": "Recently added",
    "soon": "Coming soon",
    "soonAll": "All",
    "soonLess": "Show less",
    "sortBy": "Sort by",
    "sort": { "added": "Newest first", "title": "Title" },
    "back": "Library",
    "asList": "List view",
    "asGrid": "Grid view",
    "pin": "Pin",
    "unpin": "Unpin",
    "removeAlbum": "Remove from library",
    "unfollow": "Unfollow",
    "newPlaylist": "New playlist",
    "byDescription": "Playlist from a description",
    "newPlaceholder": "Title",
    "create": "Create",
    "created": "Playlist “{title}” created",
    "createFailed": "Couldn't create the playlist — try again",
    "find": "Find music",
    "deviceSize": "On this device: {size}",
    "downloadSettings": "Downloads",
    "reserve": "Wave reserve · {n}",
    "tracks": "Tracks",
    "empty": {
      "playlists": "Make your first playlist",
      "playlistsText": "Everything you love, in one place",
      "albums": "Your albums will live here",
      "albumsText": "Save the albums you keep coming back to",
      "artists": "Your artists will live here",
      "artistsText": "Follow them — new releases show up on their own",
      "downloaded": "Nothing downloaded yet",
      "downloadedText": "Music saved to this device will be here",
      "folderText": "Add playlists with “Move to folder” in their menu"
    }
  }
}
</i18n>
