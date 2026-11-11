<script setup lang="ts">
import type { FollowedArtist, SavedAlbum } from '~/api/types'

// Медиатека: плейлисты (и любимые треки), сохранённые альбомы, подписки
type Tab = 'playlists' | 'albums' | 'artists'

const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const library = useLibraryStore()
const toast = useToast()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()
useHead({ title: () => t('title') })

const tab = computed<Tab>({
  get: () => (route.query.tab === 'albums' || route.query.tab === 'artists' ? route.query.tab : 'playlists'),
  set: v => void router.replace({ query: { ...route.query, tab: v === 'playlists' ? undefined : v } }),
})

const albums = ref<SavedAlbum[] | null>(null)
const artists = ref<FollowedArtist[] | null>(null)
const failed = ref(false)

async function load() {
  failed.value = false
  try {
    await Promise.all([
      library.loadOverview(true),
      library.loadPlaylists(true),
      bridge.library.savedAlbums().then(p => (albums.value = p.items)),
      bridge.library.followed().then(p => (artists.value = p.items)),
    ])
  }
  catch {
    failed.value = true
  }
}
onMounted(load)
watch(() => library.revision, load)

const counts = computed(() => library.overview?.counts)
const tabs = computed(() => [
  { value: 'playlists' as Tab, label: t('playlists'), count: (counts.value?.playlists ?? 0) + 1 },
  { value: 'albums' as Tab, label: t('albums'), count: counts.value?.savedAlbums },
  { value: 'artists' as Tab, label: t('artists'), count: counts.value?.followedArtists },
])

// Новый плейлист
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
</script>

<template>
  <div class="lib">
    <header class="lib__head">
      <h1 class="lib__title">{{ t('title') }}</h1>
      <div class="lib__bar">
        <UiTabs v-model="tab" :items="tabs" :label="t('title')" class="lib__tabs" />
        <!-- Новый плейлист: кнопка рядом с вкладками, форма — во всплывающей панели -->
        <UiPopover v-if="tab === 'playlists'" v-model:open="createOpen" :title="t('newPlaylist')" side="bottom" align="end" :width="300">
          <template #trigger>
            <UiButton v-if="!ui.isPhone" size="sm" icon="ph:plus" class="lib__new">{{ t('newPlaylist') }}</UiButton>
            <UiIconButton v-else icon="ph:plus" :label="t('newPlaylist')" size="lg" :tooltip="false" class="lib__new lib__new--icon" />
          </template>
          <form class="newpl__form" @submit.prevent="create">
            <input v-model="newTitle" class="newpl__input" :placeholder="t('newPlaceholder')" :aria-label="t('newPlaylist')" maxlength="120" autofocus>
            <UiButton type="submit" variant="primary" :loading="creating" :disabled="!newTitle.trim()" block>{{ t('create') }}</UiButton>
          </form>
        </UiPopover>
      </div>
    </header>

    <UiEmptyState v-if="failed" icon="ph:cloud-warning" :title="tg('common.loadFailed')" :text="tg('common.loadFailedText')" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="load">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>

    <!-- Плейлисты -->
    <div v-else-if="tab === 'playlists'" class="lib__grid">
      <NuxtLink to="/liked" class="liked">
        <span class="liked__icon"><Icon name="ph:heart-fill" /></span>
        <span class="liked__title">{{ tg('nav.liked') }}</span>
        <span class="liked__meta">{{ formatCount(library.likedCount, 'track', lang) }}</span>
      </NuxtLink>
      <MusicMixCard v-for="p in library.playlists ?? []" :key="p.id" :playlist="p" :meta="p.pinned ? `${t('pinned')} · ${formatCount(p.tracksCount, 'track', lang)}` : formatCount(p.tracksCount, 'track', lang)" />
      <template v-if="!library.playlists">
        <UiSkeleton v-for="n in 4" :key="n" class="lib__skel" />
      </template>
    </div>

    <!-- Альбомы -->
    <template v-else-if="tab === 'albums'">
      <div v-if="albums?.length" class="lib__grid">
        <MusicAlbumCard v-for="s in albums" :key="s.album.id" :album="s.album" :meta="`${s.album.artist.name} · ${tg(`record.${s.album.recordType}`)}`" />
      </div>
      <UiEmptyState v-else-if="albums" icon="ph:vinyl-record" :title="t('noAlbums')" :text="t('noAlbumsText')">
        <UiButton to="/search" icon="ph:magnifying-glass">{{ t('find') }}</UiButton>
      </UiEmptyState>
      <div v-else class="lib__grid"><UiSkeleton v-for="n in 5" :key="n" class="lib__skel" /></div>
    </template>

    <!-- Артисты -->
    <template v-else>
      <div v-if="artists?.length" class="lib__grid">
        <MusicArtistCard v-for="f in artists" :key="f.artist.id" :artist="f.artist" :meta="tg('record.artist')" />
      </div>
      <UiEmptyState v-else-if="artists" icon="ph:user-circle-plus" :title="t('noArtists')" :text="t('noArtistsText')">
        <UiButton to="/search" icon="ph:magnifying-glass">{{ t('find') }}</UiButton>
      </UiEmptyState>
      <div v-else class="lib__grid"><UiSkeleton v-for="n in 5" :key="n" class="lib__skel" variant="round" /></div>
    </template>
  </div>
</template>

<style scoped>
.lib { display: flex; flex-direction: column; gap: 28px; padding-top: 16px; }
.lib__head { display: flex; flex-direction: column; gap: 12px; }
.lib__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.lib__grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(176px, 1fr)); gap: 28px 24px; align-items: start; }
.lib__skel { aspect-ratio: 1; border-radius: var(--r-3); }
.liked {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 2px;
  aspect-ratio: 1;
  padding: 16px;
  overflow: hidden;
  border-radius: var(--r-3);
  background: radial-gradient(110% 110% at 25% 15%, var(--m3), var(--m2) 45%, var(--m1));
  box-shadow: var(--shadow-1);
  color: #fff;
  transition: transform var(--dur-3) var(--ease-out);
}
.liked:hover { transform: translateY(-3px); }
.liked__icon { position: absolute; left: 16px; top: 16px; font-size: 28px; }
.liked__title { font-size: var(--fs-h4); font-weight: 700; }
.liked__meta { font-size: var(--fs-small); opacity: 0.85; }
.lib__bar { display: flex; align-items: flex-end; gap: 16px; }
.lib__tabs { flex: 1; min-width: 0; }
.lib__new { flex: none; margin-bottom: 6px; }
.newpl__form { display: flex; flex-direction: column; gap: 8px; padding: 0 6px 6px; }
.newpl__input { height: 40px; padding: 0 14px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.newpl__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
@media (max-width: 699px) {
  .lib { gap: 20px; padding: 8px 16px 0; }
  .lib__title { font-size: 28px; }
  .lib__bar :deep(.lib__new--icon) { margin: 0 -6px 0 0; }
  .lib__grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 20px 12px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Медиатека",
    "playlists": "Плейлисты",
    "albums": "Альбомы",
    "artists": "Артисты",
    "newPlaylist": "Новый плейлист",
    "pinned": "Закреплён",
    "newPlaceholder": "Название",
    "create": "Создать",
    "created": "Плейлист «{title}» создан",
    "createFailed": "Не получилось создать плейлист — попробуйте ещё раз",
    "noAlbums": "Сохранённых альбомов пока нет",
    "noAlbumsText": "Сохраняйте альбомы — они появятся здесь",
    "noArtists": "Подписок пока нет",
    "noArtistsText": "Подпишитесь на артистов — они появятся здесь",
    "find": "Найти музыку"
  },
  "en": {
    "title": "Library",
    "playlists": "Playlists",
    "albums": "Albums",
    "artists": "Artists",
    "newPlaylist": "New playlist",
    "pinned": "Pinned",
    "newPlaceholder": "Title",
    "create": "Create",
    "created": "Playlist “{title}” created",
    "createFailed": "Couldn't create the playlist — try again",
    "noAlbums": "No saved albums yet",
    "noAlbumsText": "Save albums — they will show up here",
    "noArtists": "Not following anyone yet",
    "noArtistsText": "Follow artists — they will show up here",
    "find": "Find music"
  }
}
</i18n>
