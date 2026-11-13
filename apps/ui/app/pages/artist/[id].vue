<script setup lang="ts">
import type { AlbumSummary } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Артист: фото, подписка, популярное, релизы по видам, похожие
const route = useRoute()
const bridge = useBridge()
const library = useLibraryStore()
const shareMenu = useShare()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad(() => bridge.catalog.artist(id.value), () => id.value)
const { playArtist, isContext, isPlayingContext, loading } = usePlayActions()
useHead({ title: () => data.value?.artist.name ?? t('title') })

const artist = computed(() => data.value?.artist ?? null)
const followed = computed(() => (artist.value ? library.isFollowed(artist.value) : false))
const playing = computed(() => !!artist.value && isPlayingContext('artist', artist.value.id))
const showAllTop = ref(false)
const top = computed(() => (data.value ? data.value.topTracks.slice(0, showAllTop.value ? 10 : 5) : []))
const releaseCount = computed(() => {
  const r = data.value?.releases
  return r ? r.albums.length + r.eps.length + r.singles.length + r.compilations.length : 0
})
const shelves = computed(() => {
  const r = data.value?.releases
  if (!r) return []
  return [
    { key: 'albums', title: t('kinds.albumsTitle'), items: r.albums },
    { key: 'eps', title: t('kinds.epsTitle'), items: [...r.eps, ...r.singles] },
    { key: 'compilations', title: t('kinds.compilationsTitle'), items: r.compilations },
  ].filter(s => s.items.length)
})

function releaseMeta(a: AlbumSummary) {
  return [formatYear(a.releaseDate), tg(`record.${a.recordType}`)].filter(Boolean).join(' · ')
}

function toggleFollow() {
  if (artist.value) void library.setFollowed(artist.value, !followed.value)
}

const moreItems = computed<MenuEntry[]>(() => {
  const a = artist.value
  if (!a) return []
  return [
    ...(data.value?.topTracks.length ? [{ label: t('shuffle'), icon: 'ph:shuffle', onSelect: () => playArtist(a.id, { shuffle: true }) }] : []),
    shareMenu.entry('artist', a.id, a.name),
  ]
})

</script>

<template>
  <div class="artist">
    <template v-if="state !== 'ready'">
      <MusicPhoneNav />
      <MusicPageState :state="state" round @retry="load" />
    </template>
    <template v-else-if="data && artist">
      <MusicPageHeader :kicker="t('title')" :title="artist.name" round>
        <template #art>
          <MusicCoverArt :image="artist.image" size="large" :alt="artist.name" round eager fallback-icon="ph:user" />
        </template>
        <template v-if="artist.fans || releaseCount" #meta>
          <span v-if="artist.fans">{{ formatCount(artist.fans, 'listener', lang) }}</span>
          <span v-if="releaseCount">{{ formatCount(releaseCount, 'release', lang) }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :loading="loading === `artist:${artist.id}`" :disabled="!data.topTracks.length" @click="playArtist(artist.id)">
            {{ playing ? tg('common.pause') : isContext('artist', artist.id) ? t('resume') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" :icon="followed ? 'ph:check' : 'ph:plus'" :class="{ 'is-following': followed }" :aria-pressed="followed" @click="toggleFollow">
            {{ followed ? t('following') : t('follow') }}
          </UiButton>
        </template>
        <template #tools>
          <!-- Артиста из подписок можно держать на устройстве целиком -->
          <MusicDownloadButton v-if="followed" kind="artist" :ref-id="artist.id" :title="artist.name" />
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <section v-if="data.topTracks.length" class="artist__top">
        <header class="artist__head">
          <h2 class="artist__h2">{{ t('popular') }}</h2>
          <UiButton v-if="data.topTracks.length > 5" variant="ghost" size="sm" @click="showAllTop = !showAllTop">{{ showAllTop ? t('less') : t('more') }}</UiButton>
        </header>
        <MusicTrackList :tracks="top" hook :context="{ type: 'artist', id: artist.id, title: artist.name }" :label="t('popular')" />
      </section>

      <section v-if="data.upcoming?.length" class="artist__soon" :aria-label="t('soon')">
        <header class="artist__head">
          <h2 class="artist__h2">{{ t('soon') }}</h2>
        </header>
        <div class="artist__soon-list">
          <MusicUpcomingRelease v-for="r in data.upcoming" :key="r.album.id" :release="r" />
        </div>
      </section>

      <MusicShelf v-for="s in shelves" :key="s.key" :title="s.title" :cols="5">
        <MusicAlbumCard v-for="a in s.items" :key="a.id" :album="a" :meta="releaseMeta(a)" />
      </MusicShelf>

      <MusicArtistAbout :artist-id="artist.id" />

      <MusicShelf v-if="data.related.length" :title="t('related')" :cols="6" phone-width="120px">
        <MusicArtistCard v-for="a in data.related" :key="a.id" :artist="a" />
      </MusicShelf>

      <UiEmptyState v-if="!data.topTracks.length && !shelves.length" icon="ph:vinyl-record" :title="t('empty')" compact />

      <ShellDevMockNote :sections="['catalog']" what="артист" class="artist__note" />
    </template>
  </div>
</template>

<style scoped>
.artist { display: flex; flex-direction: column; gap: 40px; }
.is-following { box-shadow: inset 0 0 0 1px var(--accent-line); }
.is-following :deep(.iconify) { color: var(--accent); }
.artist__top { display: flex; flex-direction: column; gap: 8px; }
.artist__soon { display: flex; flex-direction: column; gap: 8px; }
.artist__soon-list { display: flex; flex-direction: column; }
.artist__head { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; }
.artist__h2 { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.artist__note { align-self: center; }
@media (max-width: 699px) {
  .artist { gap: 32px; }
  .artist__top { padding: 0 8px; }
  .artist__soon { padding: 0 8px; }
  .artist__soon-list { padding: 0 8px; }
  .artist__head { padding: 0 8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Артист",
    "resume": "Продолжить",
    "shuffle": "Перемешать",
    "follow": "Подписаться",
    "following": "Вы подписаны",
    "popular": "Популярные треки",
    "more": "Показать 10",
    "less": "Свернуть",
    "related": "Похожие артисты",
    "soon": "Скоро",
    "empty": "Релизов пока нет",
    "kinds": {
      "albumsTitle": "Альбомы",
      "epsTitle": "EP и синглы",
      "compilationsTitle": "Сборники"
    }
  },
  "en": {
    "title": "Artist",
    "resume": "Resume",
    "shuffle": "Shuffle",
    "follow": "Follow",
    "following": "Following",
    "popular": "Popular tracks",
    "more": "Show 10",
    "less": "Show less",
    "related": "Similar artists",
    "soon": "Coming soon",
    "empty": "No releases yet",
    "kinds": {
      "albumsTitle": "Albums",
      "epsTitle": "EPs and singles",
      "compilationsTitle": "Compilations"
    }
  }
}
</i18n>
