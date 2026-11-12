<script setup lang="ts">
import type { PlaylistFolder, PlaylistSummary } from '~/api/types'
import type { TreeNode } from '~/composables/usePlaylistTree'
import type { MenuEntry } from '~/types/ui'

// Боковая панель: знак, разделы, закреплённое, подписки и состояние загрузки подписок.
// rail — узкая полоса значков на планшете. Правый щелчок (долгое нажатие) по плейлисту — его
// меню; треки из списков можно бросить на свой плейлист
withDefaults(defineProps<{ rail?: boolean }>(), {})

const route = useRoute()
const library = useLibraryStore()
const player = usePlayerStore()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n()

const NAV = [
  { to: '/', key: 'nav.home', icon: 'ph:house', iconOn: 'ph:house-fill', kbd: null },
  { to: '/search', key: 'nav.search', icon: 'ph:magnifying-glass', iconOn: 'ph:magnifying-glass-bold', kbd: ['⌘', 'K'] },
  { to: '/library', key: 'nav.library', icon: 'ph:books', iconOn: 'ph:books-fill', kbd: null },
] as const

const isActive = (to: string) => (to === '/' ? route.path === '/' : route.path.startsWith(to))

onMounted(() => {
  void library.loadOverview().catch(() => {})
  void library.loadPlaylists().catch(() => {})
})

const pui = usePlaylistUi()
const pla = usePlaylistActions()
const tree = usePlaylistTree()
const playlists = computed(() => (library.playlists ?? library.overview?.pinnedPlaylists ?? []).filter(p => !pui.hidden.has(p.id)))

// Папки и плейлисты одной лентой: развёрнутая папка — со своими плейлистами под ней
type Row = { key: string, node: TreeNode, depth: 0 | 1 }
const rows = computed<Row[]>(() => {
  // Пока полный список не пришёл — закреплённые из обзора, без папок
  if (!library.playlists) return playlists.value.map(p => ({ key: p.id, node: { kind: 'playlist', id: p.id, playlist: p }, depth: 0 }))
  const out: Row[] = []
  for (const n of tree.tree.value) {
    out.push({ key: n.id, node: n, depth: 0 })
    if (n.kind === 'folder' && !n.folder.collapsed) {
      for (const p of n.children) out.push({ key: p.id, node: { kind: 'playlist', id: p.id, playlist: p }, depth: 1 })
    }
  }
  return out
})
const folderMeta = (n: { folder: PlaylistFolder, children: PlaylistSummary[] }) => (n.children.length ? formatCount(n.children.length, 'playlist', lang.value) : t('folder.empty'))

// Меню элемента: пункты собираются для того, по которому щёлкнули
const menuItems = ref<MenuEntry[]>([])
function onContext(target: EventTarget | null) {
  const key = (target as HTMLElement | null)?.closest<HTMLElement>('[data-pl]')?.dataset.pl
  if (key === 'liked') menuItems.value = pla.likedMenu()
  else if (key?.startsWith('folder:')) {
    const f = tree.folderOf(key.slice(7))
    menuItems.value = f ? tree.folderMenu(f) : []
  }
  else {
    const p = playlists.value.find(x => x.id === key)
    menuItems.value = p ? pla.menuFor(p) : []
  }
}

// Треки тянут из списка: свой плейлист подсвечивается и принимает их. Сами строки (плейлисты и папки)
// тоже тянутся: в папку, из неё и — в «Своём порядке» — на новое место
const dropOn = ref<string | null>(null)
function onDragOver(n: TreeNode, e: DragEvent) {
  if (!pui.dragTracks) return tree.onDragOver(n, e)
  if (n.kind !== 'playlist' || n.playlist.kind !== 'user') return
  e.preventDefault()
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  dropOn.value = n.id
}
function onDragLeave(n: TreeNode) {
  if (dropOn.value === n.id) dropOn.value = null
  tree.onDragLeave(n)
}
function onDrop(n: TreeNode, e: DragEvent) {
  if (!pui.dragTracks) return void tree.onDrop(n, e)
  e.preventDefault()
  dropOn.value = null
  const tracks = pui.dragTracks
  pui.dragTracks = null
  if (tracks?.length && n.kind === 'playlist') void pla.addTo(n.playlist, tracks)
}

// Порядок плейлистов и новая папка — меню у заголовка раздела
const listMenu = computed<MenuEntry[]>(() => [
  { kind: 'heading', label: t('plSort.heading') },
  ...tree.sortItems.value,
  { kind: 'separator' },
  { label: t('folder.new'), icon: 'ph:folder-simple-plus', onSelect: () => void tree.createFolder() },
])
const followed = computed(() => library.overview?.followedArtists ?? [])
const loaded = computed(() => !!library.overview)

// Загрузка подписок: кольцо показывает долю, подпись — чьи треки ещё качаются. Всё скачано — строки нет
const sync = computed(() => {
  const list = followed.value
  const total = list.reduce((s, f) => s + f.tracksTotal, 0)
  const done = list.reduce((s, f) => s + Math.min(f.tracksOnServer, f.tracksTotal), 0)
  const pending = list.filter(f => f.tracksOnServer < f.tracksTotal).map(f => f.artist.name)
  return { total, done, pending, complete: !pending.length }
})

// Новый плейлист — та же короткая форма, что в медиатеке
const createOpen = ref(false)
const newTitle = ref('')
const creating = ref(false)
async function createPlaylist() {
  const title = newTitle.value.trim()
  if (!title || creating.value) return
  creating.value = true
  try {
    const p = await bridge.playlists.create({ title })
    createOpen.value = false
    newTitle.value = ''
    void library.loadPlaylists(true)
    await navigateTo(`/playlist/${p.id}`)
  }
  catch {
    toast.error(t('nav.createFailed'))
  }
  finally {
    creating.value = false
  }
}
</script>

<template>
  <aside class="side" :class="{ 'side--rail': rail }" :aria-label="t('nav.aside')">
    <div class="side__brand"><BrandLogo :compact="rail" /></div>

    <nav class="side__nav" :aria-label="t('nav.main')">
      <UiTooltip v-for="n in NAV" :key="n.to" :text="rail ? t(n.key) : undefined" side="right" :kbd="n.kbd ? [...n.kbd] : undefined">
        <NuxtLink :to="n.to" class="nav" :class="{ 'is-active': isActive(n.to) }" :aria-current="isActive(n.to) ? 'page' : undefined">
          <Icon :name="isActive(n.to) ? n.iconOn : n.icon" class="nav__icon" />
          <span class="nav__label">{{ t(n.key) }}</span>
          <UiKbd v-if="n.kbd && !rail" :keys="[...n.kbd]" class="nav__kbd" />
        </NuxtLink>
      </UiTooltip>
    </nav>

    <div class="side__scroll no-scrollbar">
      <section class="sec">
        <header class="sec__head">
          <span class="sec__title">{{ t('nav.pinned') }}</span>
          <span v-if="!rail" class="sec__actions">
          <UiMenu :items="listMenu" align="start">
            <UiIconButton icon="ph:sliders-horizontal" :label="t('plSort.options')" size="xs" />
          </UiMenu>
          <UiPopover v-model:open="createOpen" :title="t('nav.newPlaylist')" side="right" align="start" :width="300">
            <template #trigger>
              <UiIconButton icon="ph:plus" :label="t('nav.newPlaylist')" size="xs" />
            </template>
            <form class="newpl" @submit.prevent="createPlaylist">
              <input v-model="newTitle" class="newpl__input" :placeholder="t('nav.newPlaylistName')" :aria-label="t('nav.newPlaylist')" maxlength="120" autofocus>
              <UiButton type="submit" variant="primary" :loading="creating" :disabled="!newTitle.trim()" block>{{ t('nav.create') }}</UiButton>
            </form>
          </UiPopover>
          </span>
        </header>
        <UiContextMenu :items="menuItems" @open="onContext">
          <div class="sec__list">
            <UiTooltip :text="rail ? t('nav.liked') : undefined" side="right">
              <NuxtLink to="/liked" data-pl="liked" class="item" :class="{ 'is-active': route.path === '/liked', 'is-current': player.context?.type === 'library' }">
                <span class="item__art item__art--liked"><Icon name="ph:heart-fill" /></span>
                <span class="item__text">
                  <span class="item__title">{{ t('nav.liked') }}</span>
                  <span class="item__meta"><Icon name="ph:push-pin-fill" class="item__pin" />{{ formatCount(library.likedCount, 'track', lang) }}</span>
                </span>
              </NuxtLink>
            </UiTooltip>
            <template v-if="loaded || playlists.length">
              <template v-for="r in rows" :key="r.key">
                <UiTooltip v-if="r.node.kind === 'folder'" :text="rail ? r.node.folder.title : undefined" side="right">
                  <button
                    type="button"
                    :data-pl="`folder:${r.node.id}`"
                    class="item item--folder"
                    :class="tree.dropClass(r.node.id)"
                    :draggable="!rail"
                    :aria-expanded="!r.node.folder.collapsed"
                    @click="tree.toggleFolder(r.node.folder)"
                    @dragstart="tree.onDragStart('folder', r.node.id, $event)"
                    @dragend="tree.onDragEnd()"
                    @dragover="onDragOver(r.node, $event)"
                    @dragleave="onDragLeave(r.node)"
                    @drop="onDrop(r.node, $event)"
                  >
                    <span class="item__art item__art--folder"><Icon name="ph:folder-simple" /></span>
                    <span class="item__text">
                      <span class="item__title">{{ r.node.folder.title }}</span>
                      <span class="item__meta"><span class="item__sub">{{ folderMeta(r.node) }}</span></span>
                    </span>
                    <Icon name="ph:caret-down" class="item__caret" :class="{ 'is-collapsed': r.node.folder.collapsed }" aria-hidden="true" />
                  </button>
                </UiTooltip>
                <UiTooltip v-else :text="rail ? r.node.playlist.title : undefined" side="right">
                  <NuxtLink
                    :to="`/playlist/${r.node.id}`"
                    :data-pl="r.node.id"
                    class="item"
                    :class="[{ 'item--nested': r.depth === 1, 'is-active': route.path === `/playlist/${r.node.id}`, 'is-current': player.context?.type === 'playlist' && player.context.id === r.node.id, 'is-drop': dropOn === r.node.id }, tree.dropClass(r.node.id)]"
                    :draggable="!rail"
                    @dragstart="tree.onDragStart('playlist', r.node.id, $event)"
                    @dragend="tree.onDragEnd()"
                    @dragover="onDragOver(r.node, $event)"
                    @dragleave="onDragLeave(r.node)"
                    @drop="onDrop(r.node, $event)"
                  >
                    <MusicPlaylistCover :id="r.node.id" :title="r.node.playlist.title" :covers="r.node.playlist.covers" :image="r.node.playlist.image" :kind="r.node.playlist.kind" :show-title="false" class="item__art" />
                    <span class="item__text">
                      <span class="item__title">{{ r.node.playlist.title }}</span>
                      <span class="item__meta"><Icon v-if="r.node.playlist.pinned" name="ph:push-pin-fill" class="item__pin" /><span class="item__sub">{{ pla.subOf(r.node.playlist) }}</span></span>
                    </span>
                  </NuxtLink>
                </UiTooltip>
              </template>
            </template>
            <template v-else>
              <div v-for="n in 3" :key="n" class="item item--skel"><UiSkeleton width="36px" height="36px" /><span class="item__text"><UiSkeleton variant="line" width="70%" /><UiSkeleton variant="line" width="40%" /></span></div>
            </template>
          </div>
        </UiContextMenu>
      </section>

      <section v-if="followed.length" class="sec">
        <header class="sec__head"><span class="sec__title">{{ t('nav.artists') }}</span></header>
        <UiTooltip v-for="f in followed" :key="f.artist.id" :text="rail ? f.artist.name : undefined" side="right">
          <NuxtLink :to="`/artist/${f.artist.id}`" class="item item--artist" :class="{ 'is-active': route.path === `/artist/${f.artist.id}` }">
            <MusicCoverArt :image="f.artist.image" size="small" round :alt="f.artist.name" class="item__art" fallback-icon="ph:user" />
            <span class="item__text"><span class="item__title">{{ f.artist.name }}</span></span>
            <UiTooltip v-if="f.tracksOnServer < f.tracksTotal && !rail" :text="t('nav.downloading')" side="right">
              <span class="item__dl" role="img" :aria-label="t('nav.downloading')" />
            </UiTooltip>
          </NuxtLink>
        </UiTooltip>
      </section>
    </div>

    <!-- Подвал: строки состояния друг под другом. Сюда одной строкой встаёт <DownloaderIndicator />;
         пустой подвал прячется сам (:empty) -->
    <footer class="side__foot">
      <UiTooltip v-if="followed.length && !sync.complete" :text="rail ? t('nav.syncTitle') : undefined" side="right">
        <NuxtLink to="/library?tab=artists" class="sync">
          <UiProgressRing :state="sync.done === 0 ? 'queued' : 'progress'" :value="sync.total ? sync.done / sync.total : 1" :size="28" />
          <span class="sync__text">
            <span class="sync__title">{{ t('nav.syncTitle') }}</span>
            <span class="sync__meta">{{ sync.pending.join(', ') }}</span>
          </span>
        </NuxtLink>
      </UiTooltip>
      <ShellDownloaderIndicator :rail="rail" />
    </footer>
  </aside>
</template>

<style scoped>
.side {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  padding-top: var(--titlebar-inset, 0px);
  background: var(--side-bg);
  border-right: 1px solid var(--side-edge);
}
.side__brand { padding: calc(22px - min(var(--titlebar-inset, 0px), 18px)) 20px 18px; }
.side__nav { display: flex; flex-direction: column; gap: 2px; padding: 0 12px; }
.nav { display: flex; align-items: center; gap: 12px; height: 38px; padding: 0 12px; border-radius: var(--r-3); font-weight: 550; color: var(--text-2); transition: color var(--dur-1), background-color var(--dur-1); }
.nav__icon { flex: none; font-size: 20px; }
.nav:hover { color: var(--text-1); background: var(--side-hover); }
.nav.is-active { color: var(--text-1); background: var(--side-active); }
.nav__kbd { margin-left: auto; opacity: 0; transition: opacity var(--dur-1); }
.nav:hover .nav__kbd { opacity: 1; }
.side__scroll { flex: 1; min-height: 0; overflow-y: auto; padding: 20px 12px 12px; }
.sec + .sec { margin-top: 20px; }
.sec__head { display: flex; align-items: center; justify-content: space-between; height: 28px; padding: 0 4px 0 12px; }
.sec__title { font-size: var(--fs-micro); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.item { display: flex; align-items: center; gap: 12px; min-width: 0; min-height: 48px; padding: 6px 8px; border-radius: var(--r-3); transition: background-color var(--dur-1); }
.item:hover, .item.is-active { background: var(--side-hover); }
.item__art { width: 36px; height: 36px; flex: none; border-radius: 8px; overflow: hidden; }
.item__art--liked { display: grid; place-items: center; background: radial-gradient(110% 110% at 25% 15%, var(--m3), var(--m2) 45%, var(--m1)); color: #fff; font-size: 16px; }
.item__text { display: flex; flex: 1; flex-direction: column; gap: 4px; min-width: 0; }
.item__title { overflow: hidden; font-weight: 550; text-overflow: ellipsis; white-space: nowrap; }
.item__text:has(.item__meta) { gap: 0; }
.item__meta { display: flex; align-items: center; gap: 4px; overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); white-space: nowrap; }
.item__pin { flex: none; font-size: 12px; color: var(--accent); }
.item.is-current .item__title { color: var(--accent); }
.item.is-drop { background: var(--accent-soft); box-shadow: inset 0 0 0 1.5px var(--accent-line); }
/* Перетаскивание строк: линия места — сверху или снизу строки */
.item.is-drop-before { box-shadow: inset 0 2px 0 var(--accent); border-radius: 0 0 var(--r-3) var(--r-3); }
.item.is-drop-after { box-shadow: inset 0 -2px 0 var(--accent); border-radius: var(--r-3) var(--r-3) 0 0; }
.item[draggable='true'] { -webkit-user-drag: element; }
.item--folder { width: 100%; border: 0; background: transparent; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.item--folder:focus-visible, .item:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.item__art--folder { display: grid; place-items: center; background: var(--fill-2); color: var(--text-2); font-size: 18px; }
.item__caret { flex: none; margin-right: 4px; font-size: 14px; color: var(--text-3); transition: transform var(--dur-1); }
.item__caret.is-collapsed { transform: rotate(-90deg); }
/* Плейлист в папке — с отступом под значок папки */
.item--nested { padding-left: 24px; }
.sec__actions { display: inline-flex; align-items: center; gap: 2px; }
.item__sub { overflow: hidden; text-overflow: ellipsis; }
.item--artist { min-height: 40px; padding-top: 4px; padding-bottom: 4px; }
.item--artist .item__art { width: 28px; height: 28px; margin: 0 4px; border-radius: 50%; }
.item--skel { pointer-events: none; }
.item__dl { width: 5px; height: 5px; flex: none; margin-right: 8px; border-radius: 50%; background: var(--accent); opacity: 0.8; }
.newpl { display: flex; flex-direction: column; gap: 8px; padding: 0 6px 6px; }
.newpl__input { height: 40px; padding: 0 14px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.newpl__input::placeholder { color: var(--text-4); }
.newpl__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.side__foot { display: flex; flex-direction: column; gap: 2px; padding: 12px; border-top: 1px solid var(--hair); }
.side__foot:empty { display: none; }
.sync { display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px 10px; border-radius: var(--r-3); text-align: left; transition: background-color var(--dur-1); }
.sync:hover { background: var(--side-hover); }
.sync__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.sync__title { font-size: var(--fs-small); font-weight: 600; white-space: nowrap; }
.sync__meta { overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }

/* Узкая полоса значков */
.side--rail .side__brand { display: flex; justify-content: center; padding: calc(22px - min(var(--titlebar-inset, 0px), 18px)) 0 18px; }
.side--rail .side__nav { padding: 0 10px; }
.side--rail .nav { justify-content: center; padding: 0; height: 44px; }
.side--rail .nav__label, .side--rail .nav__kbd { display: none; }
.side--rail .side__scroll { padding: 16px 10px 12px; }
/* Подписи разделов в полосе не помещаются — вместо них волосок */
.side--rail .sec__head { height: 1px; margin: 0 14px 12px; padding: 0; background: var(--hair); }
.side--rail .sec__title { display: none; }
.side--rail .item { justify-content: center; padding: 6px 0; }
.side--rail .item__text { display: none; }
.side--rail .item__caret { display: none; }
.side--rail .item--nested { padding-left: 0; }
.side--rail .item--nested .item__art { width: 30px; height: 30px; }
.side--rail .item--artist .item__art { width: 32px; height: 32px; margin: 0; }
.side--rail .side__foot { padding: 10px; }
.side--rail .sync { justify-content: center; padding: 8px 0; }
.side--rail .sync__text { display: none; }
</style>
