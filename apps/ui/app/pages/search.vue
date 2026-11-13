<script setup lang="ts">
import type { AlbumSummary, ArtistSummary, SearchResults, SearchScope, Track } from '~/api/types'
import type { SearchType } from '~/components/search/Filters.vue'

// Поиск. Запрос, фильтр и «в медиатеке» живут в адресе (?q, ?type, ?scope), поэтому «назад»
// возвращает к прошлому поиску. Пустой запрос — недавние, настроения и жанры, свои артисты
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const ui = useUiStore()
const library = useLibraryStore()
const player = usePlayerStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()
const { items: recentItems, add: addRecent } = useSearchRecent()

const TYPES: SearchType[] = ['all', 'tracks', 'artists', 'albums', 'playlists', 'sound']
const q = computed(() => (typeof route.query.q === 'string' ? route.query.q.trim() : ''))
const scope = computed<SearchScope>(() => (route.query.scope === 'library' ? 'library' : 'catalog'))
const type = computed<SearchType>(() => (TYPES.includes(route.query.type as SearchType) ? route.query.type as SearchType : 'all'))
useHead({ title: () => (q.value ? `${q.value} — ${t('title')}` : t('title')) })

function patchQuery(patch: Record<string, string | undefined>) {
  void router.replace({ query: { ...route.query, ...patch } })
}
const typeModel = computed<SearchType>({
  get: () => type.value,
  set: v => patchQuery({ type: v === 'all' ? undefined : v }),
})
const libraryModel = computed<boolean>({
  get: () => scope.value === 'library',
  set: v => patchQuery({ scope: v ? 'library' : undefined }),
})
const filterLink = (x: SearchType) => router.resolve({ query: { ...route.query, type: x } }).fullPath

// «Всё» — понемногу каждого вида, отдельный фильтр — больше. Уже загруженного хватает при переходе обратно
const results = shallowRef<SearchResults | null>(null)
const state = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
let seq = 0
let loaded = { key: '', limit: 0 }

async function run(force = false) {
  const query = q.value
  const my = ++seq
  if (!query) {
    results.value = null
    state.value = 'idle'
    loaded = { key: '', limit: 0 }
    return
  }
  // «По звучанию» ищет сам (SearchSoundResults)
  if (type.value === 'sound') return
  const key = `${scope.value}:${query}`
  const limit = type.value === 'all' ? 24 : 50
  if (!force && loaded.key === key && loaded.limit >= limit && state.value === 'ready') return
  state.value = 'loading'
  try {
    const r = await bridge.catalog.search(query, scope.value, limit)
    if (my !== seq) return
    results.value = r
    loaded = { key, limit }
    state.value = 'ready'
  }
  catch {
    if (my === seq) state.value = 'error'
  }
}
watch([q, scope, type], () => void run(), { immediate: true })

// Плейлисты ищутся среди своих по названию
const norm = (s: string) => s.toLowerCase().replace(/ё/g, 'е')
watch(q, (v) => {
  if (v) void library.loadPlaylists().catch(() => {})
}, { immediate: true })
const playlists = computed(() => {
  if (!q.value) return []
  const needle = norm(q.value)
  return (library.playlists ?? []).filter(p => norm(p.title).includes(needle))
})

// Поле на телефоне и планшете под пальцем (на компьютере поиск — в верхней строке)
const field = ref(q.value)
const fieldRef = ref<{ focus: () => void, blur: () => void } | null>(null)
watch(q, v => (field.value = v))
let timer: ReturnType<typeof setTimeout> | undefined
function onField(v: string) {
  clearTimeout(timer)
  timer = setTimeout(() => patchQuery({ q: v.trim() || undefined }), 220)
}

// Режим поиска на телефоне: заголовок уходит, справа «Отменить». Держится до «Отменить»,
// а не до потери фокуса — иначе нажатие на недавнее перестраивало бы экран под пальцем
const searching = ref(false)
function onFocus() {
  if (ui.isPhone) searching.value = true
}
function cancel() {
  clearTimeout(timer)
  searching.value = false
  field.value = ''
  fieldRef.value?.blur()
  void router.replace({ query: {} })
}
function onSubmit(v: string) {
  clearTimeout(timer)
  const s = v.trim()
  if (!s) return
  patchQuery({ q: s })
  addRecent({ kind: 'query', id: s, title: s })
  fieldRef.value?.blur()
}
const { y: scrollY } = useWindowScroll()
const solid = computed(() => searching.value || scrollY.value > 40)

onMounted(() => void library.loadOverview().catch(() => {}))
const followed = computed(() => library.overview?.followedArtists.map(f => f.artist) ?? [])

// Лучшее совпадение: в котором уверен сервер («капсайз» — Capsize), иначе артист с этим именем,
// альбом с этим названием, первый трек
type Top = { kind: 'artist', item: ArtistSummary } | { kind: 'track', item: Track } | { kind: 'album', item: AlbumSummary }
const top = computed<Top | null>(() => {
  const r = results.value
  if (!r) return null
  const sure = r.top
  if (sure?.kind === 'artist') {
    const item = r.artists.find(a => a.id === sure.id)
    if (item) return { kind: 'artist', item }
  }
  else if (sure?.kind === 'album') {
    const item = r.albums.find(a => a.id === sure.id)
    if (item) return { kind: 'album', item }
  }
  const needle = norm(q.value)
  const artist = r.artists.find(a => norm(a.name) === needle)
  if (artist) return { kind: 'artist', item: artist }
  const album = r.albums.find(a => norm(a.title) === needle)
  if (album) return { kind: 'album', item: album }
  if (r.tracks[0]) return { kind: 'track', item: r.tracks[0] }
  if (r.artists[0]) return { kind: 'artist', item: r.artists[0] }
  if (r.albums[0]) return { kind: 'album', item: r.albums[0] }
  return null
})
const empty = computed(() => {
  const r = results.value
  return !!r && !r.tracks.length && !r.albums.length && !r.artists.length && !playlists.value.length
})
// Запрос похож на описание звучания («спокойный инструментал для работы»): несколько слов и ни один
// артист, альбом или трек каталога не совпал с ним явно — тогда под результатами «Похоже по звучанию»
const words = computed(() => q.value.split(/\s+/).filter(w => w.length > 1).length)
const descriptive = computed(() => {
  const r = results.value
  if (!r || scope.value === 'library' || words.value < 2) return false
  const needle = norm(q.value)
  const strong = !!r.top
    || r.artists.some(a => norm(a.name) === needle)
    || r.albums.some(a => norm(a.title) === needle)
    || r.tracks.some((tr) => {
      const title = norm(tr.title)
      return title.includes(needle) || (needle.includes(title) && tr.artists.some(a => needle.includes(norm(a.name))))
    })
  return !strong
})
// Запрос похож на строчку песни: в кавычках или от четырёх слов — «По словам песни»
const lyricLike = computed(() => /^\s*["«“„']/.test(q.value) || q.value.split(/\s+/).filter(Boolean).length >= 4)
const soundLink = computed(() => filterLink('sound'))
const suggestionLink = computed(() => router.resolve({ query: { ...route.query, q: results.value?.suggestion ?? q.value } }).fullPath)
const ctx = computed(() => ({ type: 'search' as const, id: q.value, title: t('resultsFor', { q: q.value }) }))
const { playArtist, playAlbum, isPlayingContext } = usePlayActions()

// На телефоне трек из «лучшего совпадения» не повторяется в списке ниже
const phoneTracks = computed(() => {
  const list = results.value?.tracks ?? []
  const skip = top.value?.kind === 'track' ? top.value.item.id : null
  return list.filter(x => x.id !== skip).slice(0, 5)
})

const names = (tr: Track) => tr.artists.map(a => a.name).join(', ')
const topTitle = computed(() => (top.value ? (top.value.kind === 'artist' ? top.value.item.name : top.value.item.title) : ''))
const topImage = computed(() => {
  const tp = top.value
  if (!tp) return null
  return tp.kind === 'track' ? tp.item.album?.image ?? null : tp.item.image
})
const topSub = computed(() => {
  const tp = top.value
  if (!tp) return ''
  if (tp.kind === 'artist') return tg('record.artist')
  if (tp.kind === 'album') return `${tg(`record.${tp.item.recordType}`)} · ${tp.item.artist.name}`
  return `${t('track')} · ${names(tp.item)}`
})
const topLink = computed(() => {
  const tp = top.value
  if (!tp) return undefined
  if (tp.kind === 'artist') return `/artist/${tp.item.id}`
  if (tp.kind === 'album') return `/album/${tp.item.id}`
  return undefined
})

function playTop() {
  const tp = top.value
  if (!tp) return
  if (tp.kind === 'artist') return playArtist(tp.item.id)
  if (tp.kind === 'album') return playAlbum(tp.item.id)
  if (player.current?.track.id === tp.item.id) return player.toggle()
  return player.playTracks(results.value?.tracks ?? [tp.item], (results.value?.tracks ?? []).indexOf(tp.item), ctx.value)
}
const topPlaying = computed(() => {
  const tp = top.value
  if (!tp) return false
  if (tp.kind === 'track') return player.current?.track.id === tp.item.id && player.playing
  return isPlayingContext(tp.kind, tp.item.id)
})

// Недавние: что открыли из результатов и какой трек включили
let opened = false
function rememberArtist(a: ArtistSummary) {
  addRecent({ kind: 'artist', id: a.id, title: a.name, image: a.image })
}
function rememberAlbum(a: AlbumSummary) {
  addRecent({ kind: 'album', id: a.id, title: a.title, sub: a.artist.name, image: a.image })
}
function onResultClick(e: MouseEvent) {
  const href = (e.target as HTMLElement | null)?.closest('a')?.getAttribute('href') ?? ''
  const [, kind, id] = href.match(/^\/(artist|album|playlist)\/([^/?#]+)/) ?? []
  if (!kind || !id) return
  const r = results.value
  if (kind === 'artist') {
    const a = r?.artists.find(x => x.id === id)
    if (a) rememberArtist(a)
  }
  else if (kind === 'album') {
    const a = r?.albums.find(x => x.id === id)
    if (a) rememberAlbum(a)
  }
  else {
    const p = playlists.value.find(x => x.id === id)
    if (p) addRecent({ kind: 'playlist', id: p.id, title: p.title, covers: p.covers })
  }
  opened = true
}
watch(() => player.current?.track, (tr) => {
  if (!tr || player.context?.type !== 'search') return
  addRecent({ kind: 'track', id: tr.id, title: tr.title, sub: names(tr), image: tr.album?.image ?? null, track: tr })
  opened = true
})
function openTop() {
  const tp = top.value
  if (!tp) return
  if (tp.kind === 'artist') rememberArtist(tp.item)
  else if (tp.kind === 'album') rememberAlbum(tp.item)
  else void playTop()
  opened = true
}
// Ушли со страницы, ничего не открыв, — запомнить сам запрос
onBeforeRouteLeave(() => {
  if (q.value && !opened && results.value && !empty.value) addRecent({ kind: 'query', id: q.value, title: q.value })
})
watch(q, () => (opened = false))

const albumMeta = (a: AlbumSummary) => [tg(`record.${a.recordType}`), a.artist.name, formatYear(a.releaseDate)].filter(Boolean).join(' · ')
</script>

<template>
  <div class="search" :class="{ 'is-searching': searching }">
    <div v-if="ui.isPhone" class="search__titlewrap" :class="{ 'is-collapsed': searching }">
      <div class="search__titleclip"><h1 class="search__title">{{ t('title') }}</h1></div>
    </div>
    <h1 v-else-if="!q" class="search__title">{{ t('title') }}</h1>
    <h1 v-else class="sr-only">{{ t('resultsFor', { q }) }}</h1>

    <div v-if="ui.isPhone" class="sbar" :class="{ 'is-solid': solid }">
      <div class="sbar__row">
        <UiSearchField
          ref="fieldRef"
          v-model="field"
          links
          class="sbar__field"
          :placeholder="tg('search.placeholder')"
          @update:model-value="onField"
          @focus="onFocus"
          @submit="onSubmit"
        />
        <button v-if="searching" type="button" class="sbar__cancel" @click="cancel">{{ t('cancel') }}</button>
        <RecognizeButton v-else class="sbar__mic" />
      </div>
      <SearchFilters v-if="q" v-model:type="typeModel" v-model:library="libraryModel" class="sbar__filters" />
    </div>
    <!-- Планшет: верхней строки нет — поле здесь, крупное, закреплено при прокрутке -->
    <div v-else-if="ui.isTouch" class="tsbar" :class="{ 'is-solid': solid }">
      <UiSearchField
        ref="fieldRef"
        v-model="field"
        links
        size="lg"
        class="tsbar__field"
        :placeholder="tg('search.placeholder')"
        @update:model-value="onField"
        @submit="onSubmit"
      />
      <RecognizeButton class="tsbar__mic" />
    </div>
    <SearchFilters v-if="!ui.isPhone && q" v-model:type="typeModel" v-model:library="libraryModel" class="search__filters" />

    <!-- Пустой запрос -->
    <div v-if="!q" class="search__idle">
      <SearchRecent v-if="searching && recentItems.length" :limit="12" />
      <template v-else>
        <SearchRecent :limit="ui.isPhone ? 4 : 6" />
        <RecognizeHistory :limit="ui.isPhone ? 4 : 6" />
        <MusicDiscoverTile />
        <SearchCharts />
        <SearchBrowse />
        <MusicShelf v-if="followed.length" :title="t('yourArtists')" :cols="8" phone-width="88px" class="search__artists">
          <SearchAvatar v-for="a in followed" :key="a.id" :artist="a" />
        </MusicShelf>
      </template>
    </div>

    <div v-else-if="type === 'sound'" class="results">
      <SearchDescribeRow v-if="words >= 2" :text="q" />
      <SearchSoundResults :text="q" />
    </div>

    <div v-else-if="state === 'loading' && !results" class="search__skel" aria-busy="true">
      <template v-if="ui.isPhone">
        <div v-for="n in 7" :key="n" class="skrow">
          <UiSkeleton :class="n === 1 ? 'skrow__art skrow__art--round' : 'skrow__art'" />
          <div class="skrow__lines">
            <UiSkeleton variant="line" width="56%" height="14px" />
            <UiSkeleton variant="line" width="32%" height="12px" />
          </div>
        </div>
      </template>
      <template v-else>
        <UiSkeleton height="236px" class="search__skel-top" />
        <div class="search__skel-rows"><UiSkeleton v-for="n in 5" :key="n" variant="line" height="40px" /></div>
      </template>
    </div>

    <UiEmptyState v-else-if="state === 'error'" icon="ph:cloud-warning" :title="tg('common.loadFailed')" :text="tg('common.loadFailedText')" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="run(true)">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>

    <div v-else-if="empty && scope === 'catalog' && words >= 2" class="results">
      <SearchDescribeRow :text="q" />
      <SearchLyricsResults v-if="lyricLike" :text="q" />
      <SearchSoundResults :text="q" />
    </div>

    <UiEmptyState v-else-if="empty" icon="ph:magnifying-glass" :title="scope === 'library' ? t('nothingMine') : t('nothing')" :text="t('nothingFor', { q })">
      <UiButton v-if="scope === 'library'" variant="secondary" @click="libraryModel = false">{{ t('searchEverywhere') }}</UiButton>
    </UiEmptyState>

    <div v-else-if="results" class="results" :class="{ 'is-stale': state === 'loading' }" @click.capture="onResultClick">
      <!-- Нашлось только похожее: одна строка, без рамок -->
      <p v-if="results.suggestion" class="hint">
        {{ t('didYouMean') }}
        <NuxtLink :to="suggestionLink" class="hint__link">{{ results.suggestion }}</NuxtLink>
      </p>
      <!-- Всё -->
      <template v-if="type === 'all'">
        <template v-if="ui.isPhone">
          <SearchItemRow
            v-if="top"
            size="lg"
            class="results__top"
            :title="topTitle"
            :sub="topSub"
            :image="topImage"
            :round="top.kind === 'artist'"
            :to="topLink"
            @open="openTop"
          />
          <section v-if="phoneTracks.length" class="block" :aria-label="t('tracks')">
            <header class="block__head">
              <h2 class="block__title">{{ t('tracks') }}</h2>
              <NuxtLink v-if="results.tracks.length > 6" :to="filterLink('tracks')" class="block__all">{{ tg('common.all') }}</NuxtLink>
            </header>
            <MusicTrackList :tracks="phoneTracks" variant="compact" plain :hook="scope === 'catalog'" :context="ctx" :label="t('tracks')" />
          </section>
        </template>
        <div v-else class="lead">
          <section v-if="top" class="topres" :aria-label="t('top')">
            <header class="block__head"><h2 class="block__title">{{ t('top') }}</h2></header>
            <div class="topres__card">
              <NuxtLink v-if="topLink" :to="topLink" class="topres__link" :aria-label="topTitle" @click="openTop" />
              <button v-else type="button" class="topres__link" :aria-label="topTitle" @click="openTop" />
              <MusicCoverArt :image="topImage" size="medium" :round="top.kind === 'artist'" class="topres__art" :fallback-icon="top.kind === 'artist' ? 'ph:user' : undefined" />
              <div class="topres__name">{{ topTitle }}</div>
              <div class="topres__meta ellipsis">{{ topSub }}<template v-if="top.kind === 'artist' && top.item.fans"> · {{ formatCount(top.item.fans, 'listener', lang) }}</template></div>
              <MusicPlayButton class="topres__play" :playing="topPlaying" :size="48" @click="playTop" />
            </div>
          </section>
          <section v-if="results.tracks.length" class="lead__tracks" :aria-label="t('tracks')">
            <header class="block__head">
              <h2 class="block__title">{{ t('tracks') }}</h2>
              <UiButton v-if="results.tracks.length > 5" :to="filterLink('tracks')" variant="ghost" size="sm">{{ tg('common.all') }}</UiButton>
            </header>
            <MusicTrackList :tracks="results.tracks.slice(0, 5)" variant="compact" plain :hook="scope === 'catalog'" :context="ctx" :label="t('tracks')" />
          </section>
        </div>

        <SearchLyricsResults v-if="lyricLike && scope === 'catalog'" :text="q" />
        <SearchDescribeRow v-if="descriptive" :text="q" />
        <SearchSoundResults v-if="descriptive" :text="q" compact :all-link="soundLink" />

        <MusicShelf v-if="results.artists.length" :title="t('artists')" :cols="6" phone-width="104px" :to="results.artists.length > 6 ? filterLink('artists') : undefined">
          <template v-if="ui.isPhone">
            <SearchAvatar v-for="a in results.artists" :key="a.id" :artist="a" />
          </template>
          <template v-else>
            <MusicArtistCard v-for="a in results.artists" :key="a.id" :artist="a" />
          </template>
        </MusicShelf>
        <MusicShelf v-if="results.albums.length" :title="t('albums')" :cols="6" phone-width="140px" :to="results.albums.length > 6 ? filterLink('albums') : undefined">
          <MusicAlbumCard v-for="a in results.albums" :key="a.id" :album="a" :meta="`${a.artist.name}${a.releaseDate ? ` · ${formatYear(a.releaseDate)}` : ''}`" />
        </MusicShelf>
        <MusicShelf v-if="playlists.length" :title="t('playlists')" :cols="6" phone-width="140px">
          <MusicMixCard v-for="p in playlists" :key="p.id" :playlist="p" :meta="formatCount(p.tracksCount, 'track', lang)" />
        </MusicShelf>
      </template>

      <!-- Треки -->
      <template v-else-if="type === 'tracks'">
        <div v-if="results.tracks.length" class="results__list">
          <MusicTrackList :tracks="results.tracks" variant="wide" plain :hook="scope === 'catalog'" :context="ctx" :label="t('tracks')" />
        </div>
        <UiEmptyState v-else compact icon="ph:music-notes-simple" :title="t('noneOf.tracks')" />
      </template>

      <!-- Артисты -->
      <template v-else-if="type === 'artists'">
        <div v-if="ui.isPhone && results.artists.length" class="results__rows">
          <SearchItemRow v-for="a in results.artists" :key="a.id" :title="a.name" :sub="tg('record.artist')" :image="a.image" round :to="`/artist/${a.id}`" />
        </div>
        <div v-else-if="results.artists.length" class="results__grid">
          <MusicArtistCard v-for="a in results.artists" :key="a.id" :artist="a" />
        </div>
        <UiEmptyState v-else compact icon="ph:user" :title="t('noneOf.artists')" />
      </template>

      <!-- Альбомы -->
      <template v-else-if="type === 'albums'">
        <div v-if="ui.isPhone && results.albums.length" class="results__rows">
          <SearchItemRow v-for="a in results.albums" :key="a.id" :title="a.title" :sub="albumMeta(a)" :image="a.image" :to="`/album/${a.id}`" />
        </div>
        <div v-else-if="results.albums.length" class="results__grid">
          <MusicAlbumCard v-for="a in results.albums" :key="a.id" :album="a" />
        </div>
        <UiEmptyState v-else compact icon="ph:vinyl-record" :title="t('noneOf.albums')" />
      </template>

      <!-- Плейлисты -->
      <template v-else>
        <div v-if="ui.isPhone && playlists.length" class="results__rows">
          <SearchItemRow
            v-for="p in playlists"
            :key="p.id"
            :title="p.title"
            :sub="formatCount(p.tracksCount, 'track', lang)"
            :playlist="{ id: p.id, title: p.title, covers: p.covers }"
            :to="`/playlist/${p.id}`"
          />
        </div>
        <div v-else-if="playlists.length" class="results__grid">
          <MusicMixCard v-for="p in playlists" :key="p.id" :playlist="p" :meta="formatCount(p.tracksCount, 'track', lang)" />
        </div>
        <UiEmptyState v-else compact icon="ph:playlist" :title="t('noneOf.playlists')" />
      </template>
    </div>
  </div>
</template>

<style scoped>
.search { display: flex; flex-direction: column; gap: 32px; padding-top: 16px; }
.search__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.search__filters { margin-bottom: -14px; }
.search__idle { display: flex; flex-direction: column; gap: 40px; }

.block__head { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 36px; margin-bottom: 8px; }
.block__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.2; }
.block__all { display: inline-flex; align-items: center; min-height: 44px; padding: 0 2px 0 12px; font-size: var(--fs-body); font-weight: 550; color: var(--text-2); }
.block__all:hover { color: var(--text-1); }

.results { display: flex; flex-direction: column; gap: 36px; transition: opacity var(--dur-2); }
.results.is-stale { opacity: 0.6; }
.results__grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(168px, 1fr)); gap: 28px 24px; }
.results__rows { display: flex; flex-direction: column; }
.hint { margin: -12px 0 -16px; font-size: var(--fs-body); color: var(--text-2); }
.hint__link { font-weight: 600; color: var(--text-1); }
.hint__link:hover { text-decoration: underline; text-underline-offset: 3px; }

.lead { display: grid; grid-template-columns: minmax(280px, 1fr) minmax(0, 1.7fr); gap: 32px; }
.lead .block__head { margin-bottom: 12px; }
.lead__tracks .block__head { margin-bottom: 4px; }
.topres__card { position: relative; display: flex; flex-direction: column; gap: 4px; padding: 20px; border-radius: var(--r-4); background: var(--fill-1); transition: background-color var(--dur-1); }
.topres__card:hover { background: var(--fill-2); }
.topres__link { position: absolute; inset: 0; z-index: 1; padding: 0; border: 0; border-radius: inherit; background: none; outline: none; }
.topres__link:focus-visible { box-shadow: inset 0 0 0 2px var(--focus); }
.topres__art { width: 104px; margin-bottom: 16px; box-shadow: var(--shadow-1); }
.topres__art:not(.art--round) { border-radius: var(--r-2); }
.topres__name { overflow: hidden; padding-right: 64px; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.2; text-overflow: ellipsis; white-space: nowrap; }
.topres__meta { padding-right: 64px; font-size: var(--fs-small); color: var(--text-2); }
.topres__play { position: absolute; right: 20px; bottom: 20px; z-index: 2; opacity: 0; transform: translateY(6px); transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.topres__card:hover .topres__play, .topres__card:focus-within .topres__play { opacity: 1; transform: none; }
@media (pointer: coarse) and (min-width: 700px) {
  .topres__play { opacity: 1; transform: none; }
  .topres__card:hover { background: var(--fill-1); }
}

.search__skel { display: grid; grid-template-columns: minmax(280px, 1fr) minmax(0, 1.7fr); gap: 32px; }
.search__skel-top { border-radius: var(--r-4); }
.search__skel-rows { display: flex; flex-direction: column; gap: 16px; padding-top: 44px; }

/* Планшет под пальцем: поле во всю колонку, закреплено под строкой состояния */
.tsbar { position: sticky; top: var(--safe-top); z-index: 15; display: flex; align-items: center; gap: 8px; margin: 0 -32px; padding: 8px 32px 12px; transition: background-color var(--dur-2), box-shadow var(--dur-2); }
.tsbar::before { content: ''; position: absolute; left: 0; right: 0; bottom: 100%; height: var(--safe-top); background: inherit; }
.tsbar.is-solid { background: color-mix(in srgb, var(--ink-1) 88%, transparent); -webkit-backdrop-filter: blur(20px) saturate(1.3); backdrop-filter: blur(20px) saturate(1.3); box-shadow: 0 1px 0 var(--hair); }
.tsbar__field { flex: 1; }
.tsbar :deep(.sf) { background: var(--fill-2); box-shadow: none; }
.tsbar :deep(.sf:focus-within) { background: var(--fill-3); box-shadow: none; }
.tsbar__mic { flex: none; --size: 48px; color: var(--text-2); }

@media (max-width: 1099px) {
  .lead, .search__skel { grid-template-columns: minmax(0, 1fr); }
}

/* Телефон */
@media (max-width: 699px) {
  .search { gap: 0; padding-top: 0; }
  .search__titlewrap { display: grid; grid-template-rows: 1fr; padding: 0 16px; transition: grid-template-rows var(--dur-3) var(--ease-out), opacity var(--dur-2); }
  .search__titlewrap.is-collapsed { grid-template-rows: 0fr; opacity: 0; }
  .search__titleclip { overflow: hidden; min-height: 0; }
  .search__titlewrap .search__title { padding: 12px 0 4px; font-size: 30px; line-height: 1.2; }

  .sbar { position: sticky; top: var(--safe-top); z-index: 15; display: flex; flex-direction: column; gap: 12px; padding: 8px 16px 12px; transition: background-color var(--dur-2), box-shadow var(--dur-2); }
  .sbar::before { content: ''; position: absolute; left: 0; right: 0; bottom: 100%; height: var(--safe-top); background: inherit; }
  .sbar.is-solid { background: color-mix(in srgb, var(--ink-1) 88%, transparent); -webkit-backdrop-filter: blur(20px) saturate(1.3); backdrop-filter: blur(20px) saturate(1.3); box-shadow: 0 1px 0 var(--hair); }
  .sbar__row { display: flex; align-items: center; gap: 12px; }
  .sbar__field { flex: 1; }
  .sbar :deep(.sf) { height: 40px; padding-left: 12px; border-radius: 10px; background: var(--fill-2); box-shadow: none; }
  .sbar :deep(.sf:focus-within) { background: var(--fill-3); box-shadow: none; }
  .sbar :deep(.sf__input) { font-size: 16px; }
  .sbar :deep(.sf__icon) { font-size: 18px; }
  .sbar__mic { flex: none; --size: 44px; margin-right: -8px; color: var(--text-2); }
  .sbar__cancel { min-height: 44px; flex: none; padding: 0; border: 0; background: none; font: inherit; font-size: 16px; font-weight: 500; color: var(--text-1); animation: fade-in var(--dur-2) var(--ease-out); }
  .sbar__filters { margin-inline: -16px; padding-inline: 16px; }
  /* Правый отступ ленты: padding в конце прокрутки не учитывается, последний чип прилипал бы к краю */
  .sbar__filters::after { content: ''; width: 8px; flex: none; }

  .search__idle { gap: 32px; padding: 16px 16px 0; }
  .search__artists { margin: 0 -16px; }
  .search__artists :deep(.shelf__title) { font-size: 20px; }

  .search__skel { display: flex; flex-direction: column; gap: 0; padding: 8px 16px 0; }
  .skrow { display: flex; align-items: center; gap: 12px; min-height: 64px; }
  .skrow :deep(.skrow__art) { width: 48px; height: 48px; flex: none; border-radius: var(--r-1); }
  .skrow :deep(.skrow__art--round) { width: 56px; height: 56px; border-radius: 50%; }
  .skrow__lines { display: flex; flex: 1; flex-direction: column; gap: 8px; }

  .results { gap: 28px; padding-top: 8px; }
  .results__top { margin: 0 8px; }
  .block { padding: 0 8px; }
  .block__head { padding: 0 8px; margin-bottom: 0; }
  .block__title { font-size: 20px; }
  .results :deep(.shelf__title) { font-size: 20px; }
  .results :deep(.shelf__head) { align-items: center; }
  .results__rows { padding: 0 8px; }
  .results__list { padding: 0 6px; }
  .hint { display: flex; flex-wrap: wrap; align-items: center; gap: 0 6px; min-height: 44px; margin: -8px 0 -16px; padding: 0 16px; }
  .hint__link { display: inline-flex; align-items: center; min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Поиск",
    "cancel": "Отменить",
    "resultsFor": "Результаты: «{q}»",
    "yourArtists": "Ваши артисты",
    "nothing": "Ничего не нашлось",
    "nothingMine": "В медиатеке такого нет",
    "nothingFor": "По запросу «{q}»",
    "searchEverywhere": "Искать везде",
    "top": "Лучшее совпадение",
    "didYouMean": "Возможно, вы искали",
    "track": "Трек",
    "tracks": "Треки",
    "artists": "Артисты",
    "albums": "Альбомы и синглы",
    "playlists": "Плейлисты",
    "noneOf": {
      "tracks": "Треков не нашлось",
      "artists": "Артистов не нашлось",
      "albums": "Альбомов не нашлось",
      "playlists": "Плейлистов с таким названием нет"
    }
  },
  "en": {
    "title": "Search",
    "cancel": "Cancel",
    "resultsFor": "Results for “{q}”",
    "yourArtists": "Your artists",
    "nothing": "Nothing found",
    "nothingMine": "Not in your library",
    "nothingFor": "For “{q}”",
    "searchEverywhere": "Search everywhere",
    "top": "Top result",
    "didYouMean": "Did you mean",
    "track": "Song",
    "tracks": "Songs",
    "artists": "Artists",
    "albums": "Albums and singles",
    "playlists": "Playlists",
    "noneOf": {
      "tracks": "No songs found",
      "artists": "No artists found",
      "albums": "No albums found",
      "playlists": "No playlists with that name"
    }
  }
}
</i18n>
