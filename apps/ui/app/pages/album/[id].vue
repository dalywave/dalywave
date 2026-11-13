<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Альбом: обложка, кто и когда, треки по порядку
const route = useRoute()
const bridge = useBridge()
const library = useLibraryStore()
const shareMenu = useShare()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad(() => bridge.catalog.album(id.value), () => id.value)
const { playAlbum, isContext, isPlayingContext, loading } = usePlayActions()

useHead({ title: () => data.value?.album.title ?? t('title') })

const album = computed(() => data.value?.album ?? null)
const saved = computed(() => (album.value ? library.isSaved(album.value) : false))
const playing = computed(() => !!album.value && isPlayingContext('album', album.value.id))
const multiDisc = computed(() => new Set(data.value?.tracks.map(tr => tr.discNumber)).size > 1)
const discs = computed(() => {
  const d = data.value
  if (!d) return []
  const map = new Map<number, typeof d.tracks>()
  for (const tr of d.tracks) map.set(tr.discNumber, [...(map.get(tr.discNumber) ?? []), tr])
  return [...map.entries()].sort((a, b) => a[0] - b[0])
})
const releaseDate = computed(() => {
  const d = album.value?.releaseDate
  if (!d) return null
  const date = new Intl.DateTimeFormat(lang.value === 'en' ? 'en-GB' : 'ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }).format(new Date(d))
  return t(new Date(d) > new Date() ? 'upcoming' : 'released', { date })
})

function play() {
  if (album.value) void playAlbum(album.value.id)
}

const moreItems = computed<MenuEntry[]>(() => {
  const a = album.value
  if (!a) return []
  return [
    { label: t('toArtist'), icon: 'ph:user', onSelect: () => navigateTo(`/artist/${a.artist.id}`) },
    shareMenu.entry('album', a.id, a.title),
  ]
})

</script>

<template>
  <div class="album">
    <template v-if="state !== 'ready'">
      <MusicPhoneNav />
      <MusicPageState :state="state" @retry="load" />
    </template>
    <template v-else-if="data && album">
      <MusicPageHeader :kicker="`${tg(`record.${album.recordType}`)}${album.releaseDate ? ` · ${formatYear(album.releaseDate)}` : ''}`" :title="album.title">
        <template #art>
          <MusicCoverArt :image="album.image" size="large" :alt="album.title" eager />
        </template>
        <template #sub>
          <NuxtLink :to="`/artist/${album.artist.id}`" class="album__artist ellipsis">{{ album.artist.name }}</NuxtLink>
          <UiBadge v-if="album.explicit" variant="explicit" />
        </template>
        <template v-if="data.tracks.length" #meta>
          <span>{{ formatCount(data.tracks.length, 'track', lang) }}</span>
          <span>{{ formatLongDuration(data.totalDurationMs, lang) }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :loading="loading === `album:${album.id}`" :disabled="!data.tracks.length" @click="play">
            {{ playing ? tg('common.pause') : isContext('album', album.id) ? t('resume') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" icon="ph:shuffle" :disabled="!data.tracks.length" @click="playAlbum(album.id, { shuffle: true })">{{ t('shuffle') }}</UiButton>
        </template>
        <template #tools>
          <UiIconButton
            :icon="saved ? 'ph:check-circle-fill' : 'ph:plus-circle'"
            :label="saved ? t('unsave') : t('save')"
            :tone="saved ? 'liked' : 'default'"
            :pressed="saved"
            size="lg"
            @click="library.setSaved(album, !saved)"
          />
          <MusicDownloadButton kind="album" :ref-id="album.id" :title="album.title" />
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <section v-for="[disc, tracks] in discs" :key="disc" class="album__disc">
        <h2 v-if="multiDisc" class="album__disc-title"><Icon name="ph:disc" />{{ t('disc', { n: disc }) }}</h2>
        <MusicTrackList :tracks="tracks" variant="album" hook :context="{ type: 'album', id: album.id, title: album.title }" :label="album.title" />
      </section>
      <p v-if="!data.tracks.length" class="album__empty">{{ t('noTracks') }}</p>

      <footer class="album__foot">
        <span v-if="releaseDate">{{ releaseDate }}</span>
        <span v-if="data.label">℗ {{ data.label }}</span>
        <span v-if="data.genres.length">{{ data.genres.join(', ') }}</span>
        <ShellDevMockNote :sections="['catalog']" what="альбом" />
      </footer>
    </template>
  </div>
</template>

<style scoped>
.album { display: flex; flex-direction: column; gap: 28px; }
.album__artist { min-width: 0; }
.album__disc { display: flex; flex-direction: column; gap: 4px; }
.album__disc-title { display: flex; align-items: center; gap: 8px; margin: 8px 10px 4px; font-size: var(--fs-small); font-weight: 650; color: var(--text-2); }
.album__empty { color: var(--text-3); }
.album__foot { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; padding: 0 10px; font-size: var(--fs-caption); color: var(--text-3); }
@media (max-width: 699px) {
  .album { gap: 20px; }
  .album__disc { padding: 0 8px; }
  .album__foot { padding: 0 18px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Альбом",
    "resume": "Продолжить",
    "shuffle": "Перемешать",
    "save": "Добавить в медиатеку",
    "unsave": "Убрать из медиатеки",
    "toArtist": "Перейти к артисту",
    "disc": "Диск {n}",
    "noTracks": "Треков пока нет",
    "released": "Вышел {date}",
    "upcoming": "Выйдет {date}"
  },
  "en": {
    "title": "Album",
    "resume": "Resume",
    "shuffle": "Shuffle",
    "save": "Add to library",
    "unsave": "Remove from library",
    "toArtist": "Go to artist",
    "disc": "Disc {n}",
    "noTracks": "No tracks yet",
    "released": "Released {date}",
    "upcoming": "Out {date}"
  }
}
</i18n>
