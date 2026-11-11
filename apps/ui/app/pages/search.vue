<script setup lang="ts">
import type { AlbumSummary, ArtistSummary, SearchResults, SearchScope, Track } from '~/api/types'

// Поиск: по всему каталогу или только по своей медиатеке. Запрос живёт в адресе (?q=), поэтому
// «назад» возвращает к прошлому поиску. Пустой запрос — недавние поиски и свои артисты
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const ui = useUiStore()
const library = useLibraryStore()
const player = usePlayerStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const q = computed(() => (typeof route.query.q === 'string' ? route.query.q.trim() : ''))
const scope = computed<SearchScope>(() => (route.query.scope === 'library' ? 'library' : 'catalog'))
useHead({ title: () => (q.value ? `${q.value} — ${t('title')}` : t('title')) })

const results = shallowRef<SearchResults | null>(null)
const state = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
let seq = 0

async function run() {
  const query = q.value
  const my = ++seq
  if (!query) {
    results.value = null
    state.value = 'idle'
    return
  }
  state.value = 'loading'
  try {
    const r = await bridge.catalog.search(query, scope.value, 24)
    if (my !== seq) return
    results.value = r
    state.value = 'ready'
    remember(query)
  }
  catch {
    if (my === seq) state.value = 'error'
  }
}
watch([q, scope], run, { immediate: true })

// Поле на телефоне (на большом экране поиск — в верхней строке)
const field = ref(q.value)
watch(q, v => (field.value = v))
let timer: ReturnType<typeof setTimeout> | undefined
function onField(v: string) {
  clearTimeout(timer)
  timer = setTimeout(() => void router.replace({ query: { ...route.query, q: v.trim() || undefined } }), 220)
}

function setScope(s: SearchScope) {
  void router.replace({ query: { ...route.query, scope: s === 'library' ? 'library' : undefined } })
}

// Недавние поиски — только в этом браузере
const recent = useLocalStorage<string[]>('dw.search.recent', [])
function remember(query: string) {
  recent.value = [query, ...recent.value.filter(x => x.toLowerCase() !== query.toLowerCase())].slice(0, 8)
}
function forget(query: string) {
  recent.value = recent.value.filter(x => x !== query)
}
function searchFor(query: string) {
  void router.push({ path: '/search', query: { q: query, scope: route.query.scope } })
}

onMounted(() => void library.loadOverview().catch(() => {}))
const followed = computed(() => library.overview?.followedArtists.map(f => f.artist) ?? [])

// Лучшее совпадение: артист с этим именем, иначе первый трек, иначе альбом
type Top = { kind: 'artist', item: ArtistSummary } | { kind: 'track', item: Track } | { kind: 'album', item: AlbumSummary }
const top = computed<Top | null>(() => {
  const r = results.value
  if (!r) return null
  const needle = q.value.toLowerCase()
  const artist = r.artists.find(a => a.name.toLowerCase() === needle)
  if (artist) return { kind: 'artist', item: artist }
  const album = r.albums.find(a => a.title.toLowerCase() === needle)
  if (album) return { kind: 'album', item: album }
  if (r.tracks[0]) return { kind: 'track', item: r.tracks[0] }
  if (r.artists[0]) return { kind: 'artist', item: r.artists[0] }
  if (r.albums[0]) return { kind: 'album', item: r.albums[0] }
  return null
})
const empty = computed(() => !!results.value && !results.value.tracks.length && !results.value.albums.length && !results.value.artists.length)
const ctx = computed(() => ({ type: 'search' as const, id: q.value, title: t('resultsFor', { q: q.value }) }))
const { playArtist, playAlbum, isPlayingContext } = usePlayActions()

function playTop() {
  const tp = top.value
  if (!tp) return
  if (tp.kind === 'artist') return playArtist(tp.item.id)
  if (tp.kind === 'album') return playAlbum(tp.item.id)
  if (player.current?.track.id === tp.item.id) return player.toggle()
  return player.playTracks(results.value?.tracks ?? [tp.item], 0, ctx.value)
}
const topPlaying = computed(() => {
  const tp = top.value
  if (!tp) return false
  if (tp.kind === 'track') return player.current?.track.id === tp.item.id && player.playing
  return isPlayingContext(tp.kind, tp.item.id)
})
</script>

<template>
  <div class="search">
    <header class="search__head">
      <h1 v-if="ui.isPhone || !q" class="search__title">{{ t('title') }}</h1>
      <h1 v-else class="search__title search__title--q">{{ t('resultsFor', { q }) }}</h1>
      <UiSearchField v-if="ui.isPhone" v-model="field" size="lg" :autofocus="!q" :placeholder="tg('search.placeholder')" class="search__field" @update:model-value="onField" />
      <UiSegmentedControl
        :model-value="scope"
        :options="[{ value: 'catalog', label: t('catalog'), icon: 'ph:globe-hemisphere-east' }, { value: 'library', label: t('mine'), icon: 'ph:books' }]"
        :label="t('scope')"
        size="sm"
        :block="ui.isPhone"
        class="search__scope"
        @update:model-value="setScope"
      />
    </header>

    <!-- Пустой запрос -->
    <section v-if="state === 'idle'" class="search__idle">
      <div v-if="recent.length" class="search__group">
        <h2 class="search__h2">{{ t('recent') }}</h2>
        <div class="search__chips">
          <UiChip v-for="r in recent" :key="r" icon="ph:clock-counter-clockwise" removable @click="searchFor(r)" @remove="forget(r)">{{ r }}</UiChip>
        </div>
      </div>
      <div v-if="followed.length" class="search__group">
        <h2 class="search__h2">{{ t('yourArtists') }}</h2>
        <div class="search__artists">
          <NuxtLink v-for="a in followed" :key="a.id" :to="`/artist/${a.id}`" class="search__artist">
            <MusicCoverArt :image="a.image" size="small" round :alt="a.name" fallback-icon="ph:user" />
            <span class="ellipsis">{{ a.name }}</span>
          </NuxtLink>
        </div>
      </div>
      <UiEmptyState v-if="!recent.length && !followed.length" icon="ph:magnifying-glass" :title="t('idleTitle')" :text="t('idleText')" />
    </section>

    <div v-else-if="state === 'loading' && !results" class="search__skel" aria-busy="true">
      <UiSkeleton height="220px" class="search__skel-top" />
      <div class="search__skel-rows"><UiSkeleton v-for="n in 5" :key="n" variant="line" height="40px" /></div>
    </div>

    <UiEmptyState v-else-if="state === 'error'" icon="ph:cloud-warning" :title="tg('common.loadFailed')" :text="tg('common.loadFailedText')" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="run">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>

    <UiEmptyState v-else-if="empty" icon="ph:magnifying-glass" :title="t('nothing', { q })" :text="scope === 'library' ? t('nothingMine') : t('nothingText')">
      <UiButton v-if="scope === 'library'" icon="ph:globe-hemisphere-east" @click="setScope('catalog')">{{ t('searchCatalog') }}</UiButton>
    </UiEmptyState>

    <template v-else-if="results">
      <div class="search__lead" :class="{ 'is-stale': state === 'loading' }">
        <section v-if="top" class="topres" :aria-label="t('top')">
          <h2 class="search__h2">{{ t('top') }}</h2>
          <div class="topres__card">
            <NuxtLink :to="top.kind === 'artist' ? `/artist/${top.item.id}` : top.kind === 'album' ? `/album/${top.item.id}` : (top.item.album ? `/album/${top.item.album.id}` : '/search')" class="topres__link" :aria-label="top.kind === 'artist' ? top.item.name : top.item.title" />
            <MusicCoverArt
              :image="top.kind === 'artist' ? top.item.image : top.kind === 'album' ? top.item.image : top.item.album?.image"
              size="medium"
              :round="top.kind === 'artist'"
              class="topres__art"
              :fallback-icon="top.kind === 'artist' ? 'ph:user' : undefined"
            />
            <div class="topres__name">{{ top.kind === 'artist' ? top.item.name : top.item.title }}</div>
            <div class="topres__meta">
              <UiBadge variant="meta">{{ top.kind === 'artist' ? tg('record.artist') : top.kind === 'album' ? tg(`record.${top.item.recordType}`) : t('track') }}</UiBadge>
              <span v-if="top.kind === 'album'">{{ top.item.artist.name }}</span>
              <span v-else-if="top.kind === 'track'">{{ top.item.artists.map(a => a.name).join(', ') }}</span>
              <span v-else-if="top.item.fans">{{ formatCount(top.item.fans, 'listener', lang) }}</span>
            </div>
            <MusicPlayButton class="topres__play" :playing="topPlaying" :size="48" @click="playTop" />
          </div>
        </section>
        <section v-if="results.tracks.length" class="search__tracks" :aria-label="t('tracks')">
          <h2 class="search__h2">{{ t('tracks') }}</h2>
          <MusicTrackList :tracks="results.tracks.slice(0, 5)" variant="compact" :context="ctx" :label="t('tracks')" />
        </section>
      </div>

      <MusicShelf v-if="results.artists.length" :title="t('artists')" :cols="6" phone-width="120px">
        <MusicArtistCard v-for="a in results.artists" :key="a.id" :artist="a" />
      </MusicShelf>
      <MusicShelf v-if="results.albums.length" :title="t('albums')" :cols="5">
        <MusicAlbumCard v-for="a in results.albums" :key="a.id" :album="a" :meta="`${a.artist.name}${a.releaseDate ? ` · ${formatYear(a.releaseDate)}` : ''}`" />
      </MusicShelf>
      <section v-if="results.tracks.length > 5" class="search__more" :aria-label="t('moreTracks')">
        <h2 class="search__h2">{{ t('moreTracks') }}</h2>
        <MusicTrackList :tracks="results.tracks.slice(5)" variant="wide" :context="ctx" :start="6" />
      </section>
    </template>
  </div>
</template>

<style scoped>
.search { display: flex; flex-direction: column; gap: 36px; padding-top: 16px; }
.search__head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 16px; }
.search__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.search__title--q { font-size: var(--fs-h3); }
.search__h2 { margin: 0 0 12px; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; }
.search__group { display: flex; flex-direction: column; }
.search__idle { display: flex; flex-direction: column; gap: 32px; }
.search__chips { display: flex; flex-wrap: wrap; gap: 8px; }
.search__artists { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 8px; }
.search__artist { display: flex; align-items: center; gap: 12px; min-width: 0; padding: 8px 12px 8px 8px; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); font-weight: 600; transition: background-color var(--dur-1); }
.search__artist:hover { background: var(--fill-2); }
.search__artist :deep(.art) { width: 36px; }
.search__lead { display: grid; grid-template-columns: minmax(280px, 1fr) minmax(0, 1.7fr); gap: 32px; transition: opacity var(--dur-2); }
.search__lead.is-stale { opacity: 0.6; }
.topres__card { position: relative; display: flex; flex-direction: column; gap: 6px; padding: 20px; border-radius: var(--r-5); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); transition: background-color var(--dur-1); }
.topres__card:hover { background: var(--fill-2); }
.topres__link { position: absolute; inset: 0; z-index: 1; border-radius: inherit; }
.topres__art { width: 96px; margin-bottom: 12px; border-radius: var(--r-3); box-shadow: var(--shadow-1); }
.topres__name { overflow: hidden; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; text-overflow: ellipsis; white-space: nowrap; }
.topres__meta { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: var(--fs-small); color: var(--text-2); }
.topres__play { position: absolute; right: 20px; bottom: 20px; z-index: 2; box-shadow: 0 8px 20px -6px rgba(0, 0, 0, 0.7); }
.search__tracks { min-width: 0; }
.search__skel { display: grid; grid-template-columns: minmax(280px, 1fr) minmax(0, 1.7fr); gap: 32px; }
.search__skel-top { border-radius: var(--r-5); }
.search__skel-rows { display: flex; flex-direction: column; gap: 16px; padding-top: 8px; }
@media (max-width: 1099px) {
  .search__lead, .search__skel { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 699px) {
  .search { gap: 28px; padding-top: 8px; }
  .search__head { padding: 0 16px; }
  .search__title { width: 100%; font-size: 28px; }
  .search__field { width: 100%; }
  .search__scope { width: 100%; }
  .search__scope :deep(.seg__item) { height: 32px; }
  .search__artists { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .search__artist { padding: 6px 12px 6px 6px; }
  .search__idle, .search__lead, .search__skel { padding: 0 16px; }
  .search__more { padding: 0 8px; }
  .search__more .search__h2 { padding: 0 8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Поиск",
    "resultsFor": "Результаты: «{q}»",
    "scope": "Где искать",
    "catalog": "Каталог",
    "mine": "Моя медиатека",
    "recent": "Недавние поиски",
    "yourArtists": "Ваши артисты",
    "idleTitle": "Что послушать?",
    "idleText": "Ищите треки, альбомы и артистов — во всём каталоге или только в своей медиатеке",
    "nothing": "Ничего не нашлось по «{q}»",
    "nothingText": "Проверьте написание или попробуйте короче — например, только имя артиста",
    "nothingMine": "В вашей медиатеке такого нет — поищите во всём каталоге",
    "searchCatalog": "Искать в каталоге",
    "top": "Лучшее совпадение",
    "track": "Трек",
    "tracks": "Треки",
    "artists": "Артисты",
    "albums": "Альбомы и синглы",
    "moreTracks": "Ещё треки"
  },
  "en": {
    "title": "Search",
    "resultsFor": "Results for “{q}”",
    "scope": "Where to search",
    "catalog": "Catalog",
    "mine": "My library",
    "recent": "Recent searches",
    "yourArtists": "Your artists",
    "idleTitle": "What to play?",
    "idleText": "Find tracks, albums and artists — across the catalog or just in your library",
    "nothing": "Nothing found for “{q}”",
    "nothingText": "Check the spelling or try something shorter — like just the artist's name",
    "nothingMine": "Nothing like that in your library — try the whole catalog",
    "searchCatalog": "Search the catalog",
    "top": "Top result",
    "track": "Track",
    "tracks": "Tracks",
    "artists": "Artists",
    "albums": "Albums and singles",
    "moreTracks": "More tracks"
  }
}
</i18n>
