<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'
import { colorsFromPalette } from '~/utils/cover-colors'

// Альбом: обложка, кто и когда, сколько уже на сервере, треки по порядку
const route = useRoute()
const bridge = useBridge()
const library = useLibraryStore()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad(() => bridge.catalog.album(id.value), () => id.value)
const { playAlbum, isContext, isPlayingContext, loading } = usePlayActions()

useHead({ title: () => data.value?.album.title ?? t('title') })

const album = computed(() => data.value?.album ?? null)
const glow = computed(() => colorsFromPalette(album.value?.palette)?.vivid ?? null)
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
  return d ? new Intl.DateTimeFormat(lang.value === 'en' ? 'en-GB' : 'ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }).format(new Date(d)) : null
})

function play() {
  if (album.value) void playAlbum(album.value.id)
}

const moreItems = computed<MenuEntry[]>(() => {
  const a = album.value
  if (!a) return []
  return [
    { label: t('shuffle'), icon: 'ph:shuffle', onSelect: () => playAlbum(a.id, { shuffle: true }) },
    { label: tg('track.radio'), icon: 'ph:waveform', onSelect: () => toast.show({ text: t('radioSoon'), icon: 'ph:waveform' }) },
    { kind: 'separator' },
    { label: t('toArtist'), icon: 'ph:user', onSelect: () => navigateTo(`/artist/${a.artist.id}`) },
    { label: t('copyLink'), icon: 'ph:link', onSelect: () => copyLink() },
  ]
})

async function copyLink() {
  try {
    await navigator.clipboard.writeText(location.href)
    toast.show({ text: t('linkCopied'), icon: 'ph:link' })
  }
  catch {
    toast.error(t('linkFailed'))
  }
}
</script>

<template>
  <div class="album">
    <MusicPageState v-if="state !== 'ready'" :state="state" @retry="load" />
    <template v-else-if="data && album">
      <MusicPageHeader :kicker="`${tg(`record.${album.recordType}`)}${album.releaseDate ? ` · ${formatYear(album.releaseDate)}` : ''}`" :title="album.title" :glow="glow">
        <template #art>
          <MusicCoverArt :image="album.image" size="large" :alt="album.title" ambient eager />
        </template>
        <template #sub>
          <NuxtLink :to="`/artist/${album.artist.id}`">{{ album.artist.name }}</NuxtLink>
          <UiBadge v-if="album.explicit" variant="explicit" />
        </template>
        <template #meta>
          <span>{{ formatCount(data.tracks.length, 'track', lang) }}</span>
          <span class="dot">·</span>
          <span>{{ formatLongDuration(data.totalDurationMs, lang) }}</span>
          <span class="dot">·</span>
          <span class="ready" :class="{ 'is-full': data.readyTracks >= data.tracks.length }">
            <Icon :name="data.readyTracks >= data.tracks.length ? 'ph:check-circle-fill' : 'ph:cloud-arrow-down'" />
            {{ data.readyTracks >= data.tracks.length ? t('allReady') : t('ready', { n: data.readyTracks, total: data.tracks.length }) }}
          </span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :loading="loading === `album:${album.id}`" @click="play">
            {{ playing ? tg('common.pause') : isContext('album', album.id) ? t('resume') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" icon="ph:shuffle" @click="playAlbum(album.id, { shuffle: true })">{{ t('shuffle') }}</UiButton>
          <UiIconButton
            :icon="saved ? 'ph:check-circle-fill' : 'ph:plus-circle'"
            :label="saved ? t('unsave') : t('save')"
            :tone="saved ? 'liked' : 'default'"
            :pressed="saved"
            size="lg"
            @click="library.setSaved(album, !saved)"
          />
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <section v-for="[disc, tracks] in discs" :key="disc" class="album__disc">
        <h2 v-if="multiDisc" class="album__disc-title"><Icon name="ph:disc" />{{ t('disc', { n: disc }) }}</h2>
        <MusicTrackList :tracks="tracks" variant="album" :context="{ type: 'album', id: album.id, title: album.title }" :label="album.title" />
      </section>
      <p v-if="!data.tracks.length" class="album__empty">{{ t('noTracks') }}</p>

      <footer class="album__foot">
        <span v-if="releaseDate">{{ t('released', { date: releaseDate }) }}</span>
        <span v-if="data.label">℗ {{ data.label }}</span>
        <span v-if="data.genres.length">{{ data.genres.join(', ') }}</span>
        <span v-if="data.upc">UPC {{ data.upc }}</span>
        <ShellDevMockNote :sections="['catalog']" what="альбом" />
      </footer>
    </template>
  </div>
</template>

<style scoped>
.album { display: flex; flex-direction: column; gap: 28px; }
.dot { color: var(--text-4); }
.ready { display: inline-flex; align-items: center; gap: 5px; }
.ready :deep(svg) { color: var(--text-3); }
.ready.is-full :deep(svg) { color: var(--success); }
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
    "copyLink": "Скопировать ссылку",
    "linkCopied": "Ссылка скопирована",
    "linkFailed": "Не получилось скопировать ссылку",
    "radioSoon": "Волна по альбому появится вместе с рекомендациями",
    "allReady": "Весь на сервере",
    "ready": "{n} из {total} на сервере — остальное скачается при запуске",
    "disc": "Диск {n}",
    "noTracks": "Треков пока нет: альбом анонсирован или сервер ещё не получил список",
    "released": "Вышел {date}"
  },
  "en": {
    "title": "Album",
    "resume": "Resume",
    "shuffle": "Shuffle",
    "save": "Add to library",
    "unsave": "Remove from library",
    "toArtist": "Go to artist",
    "copyLink": "Copy link",
    "linkCopied": "Link copied",
    "linkFailed": "Couldn't copy the link",
    "radioSoon": "Album radio arrives with recommendations",
    "allReady": "All on the server",
    "ready": "{n} of {total} on the server — the rest downloads when played",
    "disc": "Disc {n}",
    "noTracks": "No tracks yet: the album is announced or the server hasn't got the list",
    "released": "Released {date}"
  }
}
</i18n>
