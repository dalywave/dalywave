<script setup lang="ts">
import type { AlbumSummary, DownloadPolicy } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Артист: фото, подписка (что сервер скачивает сам), популярное, релизы по видам, похожие
const route = useRoute()
const bridge = useBridge()
const library = useLibraryStore()
const toast = useToast()
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
const policy = computed<DownloadPolicy | null>(() => data.value?.follow?.policy ?? null)
const policyText = computed(() => {
  const p = policy.value
  if (!p) return t('policyDefault')
  const parts = [p.albums && t('kinds.albums'), p.eps && t('kinds.eps'), p.singles && t('kinds.singles'), p.compilations && t('kinds.compilations'), p.live && t('kinds.live'), p.appearsOn && t('kinds.appearsOn')].filter(Boolean)
  return parts.length ? t('policy', { what: parts.join(', ') }) : t('policyNothing')
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
    { label: t('shuffle'), icon: 'ph:shuffle', onSelect: () => playArtist(a.id, { shuffle: true }) },
    { label: t('radio'), icon: 'ph:waveform', onSelect: () => toast.show({ text: t('radioSoon'), icon: 'ph:waveform' }) },
    { kind: 'separator' },
    { label: t('block'), icon: 'ph:prohibit', onSelect: () => toast.show({ text: t('blockSoon', { name: a.name }), icon: 'ph:prohibit' }) },
  ]
})
</script>

<template>
  <div class="artist">
    <MusicPageState v-if="state !== 'ready'" :state="state" round @retry="load" />
    <template v-else-if="data && artist">
      <MusicPageHeader :kicker="t('title')" :title="artist.name" round>
        <template #art>
          <MusicCoverArt :image="artist.image" size="large" :alt="artist.name" round ambient eager fallback-icon="ph:user" />
        </template>
        <template #meta>
          <span v-if="artist.fans">{{ formatCount(artist.fans, 'listener', lang) }}</span>
          <span v-if="artist.fans" class="dot">·</span>
          <span>{{ formatCount(releaseCount, 'release', lang) }}</span>
          <span class="dot">·</span>
          <span class="ready" :class="{ 'is-full': data.tracksOnServer >= data.tracksTotal && data.tracksTotal > 0 }">
            <Icon :name="data.tracksOnServer >= data.tracksTotal && data.tracksTotal > 0 ? 'ph:check-circle-fill' : 'ph:cloud-arrow-down'" />
            {{ t('onServer', { n: data.tracksOnServer, total: data.tracksTotal }) }}
          </span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :loading="loading === `artist:${artist.id}`" :disabled="!data.topTracks.length" @click="playArtist(artist.id)">
            {{ playing ? tg('common.pause') : isContext('artist', artist.id) ? t('resume') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" :icon="followed ? 'ph:bell-ringing-fill' : 'ph:bell-simple'" :class="{ 'is-following': followed }" @click="toggleFollow">
            {{ followed ? t('following') : t('follow') }}
          </UiButton>
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <p v-if="followed" class="artist__policy"><Icon name="ph:download-simple" />{{ policyText }}</p>

      <section v-if="data.topTracks.length" class="artist__top">
        <header class="artist__head">
          <h2 class="artist__h2">{{ t('popular') }}</h2>
          <UiButton v-if="data.topTracks.length > 5" variant="ghost" size="sm" @click="showAllTop = !showAllTop">{{ showAllTop ? t('less') : t('more') }}</UiButton>
        </header>
        <MusicTrackList :tracks="top" :context="{ type: 'artist', id: artist.id, title: artist.name }" :label="t('popular')" />
      </section>

      <MusicShelf v-for="s in shelves" :key="s.key" :title="s.title" :cols="5">
        <MusicAlbumCard v-for="a in s.items" :key="a.id" :album="a" :meta="releaseMeta(a)" />
      </MusicShelf>

      <MusicShelf v-if="data.related.length" :title="t('related')" :cols="6" phone-width="120px">
        <MusicArtistCard v-for="a in data.related" :key="a.id" :artist="a" />
      </MusicShelf>

      <ShellDevMockNote :sections="['catalog']" what="артист" class="artist__note" />
    </template>
  </div>
</template>

<style scoped>
.artist { display: flex; flex-direction: column; gap: 40px; }
.dot { color: var(--text-4); }
.ready { display: inline-flex; align-items: center; gap: 5px; }
.ready :deep(svg) { color: var(--text-3); }
.ready.is-full :deep(svg) { color: var(--success); }
.is-following { box-shadow: inset 0 0 0 1px var(--accent-line); }
.is-following :deep(svg) { color: var(--accent); }
.artist__policy { display: flex; align-items: center; gap: 8px; margin: -24px 0 0; font-size: var(--fs-small); color: var(--text-3); }
.artist__top { display: flex; flex-direction: column; gap: 8px; }
.artist__head { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; }
.artist__h2 { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.artist__note { align-self: center; }
@media (max-width: 699px) {
  .artist { gap: 32px; }
  .artist__policy { justify-content: center; margin: -16px 16px 0; text-align: center; }
  .artist__top { padding: 0 8px; }
  .artist__head { padding: 0 8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Артист",
    "resume": "Продолжить",
    "shuffle": "Перемешать",
    "radio": "Волна по артисту",
    "radioSoon": "Волна по артисту появится вместе с рекомендациями",
    "block": "Не рекомендовать артиста",
    "blockSoon": "Скрывать {name} из волны можно будет вместе с рекомендациями",
    "follow": "Подписаться",
    "following": "Вы подписаны",
    "onServer": "{n} из {total} треков на сервере",
    "popular": "Популярные треки",
    "more": "Показать 10",
    "less": "Свернуть",
    "related": "Похожие артисты",
    "policyDefault": "Сервер скачивает альбомы, EP и синглы — и новые, как только выходят",
    "policy": "Сервер скачивает: {what} — и новые, как только выходят",
    "policyNothing": "Подписка без скачивания: только уведомления о новинках",
    "kinds": {
      "albums": "альбомы",
      "eps": "EP",
      "singles": "синглы",
      "compilations": "сборники",
      "live": "концертные",
      "appearsOn": "участие в чужих релизах",
      "albumsTitle": "Альбомы",
      "epsTitle": "EP и синглы",
      "compilationsTitle": "Сборники"
    }
  },
  "en": {
    "title": "Artist",
    "resume": "Resume",
    "shuffle": "Shuffle",
    "radio": "Artist radio",
    "radioSoon": "Artist radio arrives with recommendations",
    "block": "Don't recommend this artist",
    "blockSoon": "Hiding {name} from the wave arrives with recommendations",
    "follow": "Follow",
    "following": "Following",
    "onServer": "{n} of {total} tracks on the server",
    "popular": "Popular tracks",
    "more": "Show 10",
    "less": "Show less",
    "related": "Similar artists",
    "policyDefault": "The server downloads albums, EPs and singles — and new ones as they come out",
    "policy": "The server downloads: {what} — and new ones as they come out",
    "policyNothing": "Following without downloads: release notifications only",
    "kinds": {
      "albums": "albums",
      "eps": "EPs",
      "singles": "singles",
      "compilations": "compilations",
      "live": "live",
      "appearsOn": "appearances",
      "albumsTitle": "Albums",
      "epsTitle": "EPs and singles",
      "compilationsTitle": "Compilations"
    }
  }
}
</i18n>
