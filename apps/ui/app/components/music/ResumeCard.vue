<script setup lang="ts">
import type { ShelfItem } from '~/api/types'

// Плитка «Продолжить»: обложка, название, что дальше, полоска прогресса
const props = withDefaults(defineProps<{
  item: ShelfItem
  /** Доля прослушанного 0…1; null — без полоски */
  progress?: number | null
  meta?: string
  forceHover?: boolean
}>(), { progress: null })

const { t } = useI18n()
const lang = useLang()
const player = usePlayerStore()
const { playAlbum, playArtist, playPlaylist, isContext, isPlayingContext, loading } = usePlayActions()

const view = computed(() => {
  const it = props.item
  switch (it.type) {
    case 'album':
      return { id: it.id, title: it.title, to: `/album/${it.id}`, image: it.image, round: false, ctx: 'album' as const, sub: `${it.artist.name} · ${t(`record.${it.recordType}`)}` }
    case 'artist':
      return { id: it.id, title: t('home.artistRadio', { name: it.name }), to: `/artist/${it.id}`, image: it.image, round: true, ctx: 'artist' as const, sub: t('home.waveByArtist') }
    case 'playlist':
      return { id: it.id, title: it.title, to: `/playlist/${it.id}`, image: null, round: false, ctx: 'playlist' as const, sub: `${t('record.playlist')} · ${formatCount(it.tracksCount, 'track', lang.value)}` }
    default:
      return { id: it.id, title: it.title, to: it.album ? `/album/${it.album.id}` : '/', image: it.album?.image ?? null, round: false, ctx: null, sub: it.artists.map(a => a.name).join(', ') }
  }
})

const current = computed(() => (view.value.ctx ? isContext(view.value.ctx, view.value.id) : player.current?.track.id === view.value.id))
const playing = computed(() => (view.value.ctx ? isPlayingContext(view.value.ctx, view.value.id) : current.value && player.playing))
const busy = computed(() => !!view.value.ctx && loading.value === `${view.value.ctx}:${view.value.id}`)

function play() {
  const it = props.item
  if (it.type === 'album') return playAlbum(it.id)
  if (it.type === 'artist') return playArtist(it.id)
  if (it.type === 'playlist') return playPlaylist(it.id)
  if (current.value) return player.toggle()
  return player.playTracks([it], 0, { type: 'library', id: null, title: it.title })
}
</script>

<template>
  <div class="resume" :class="{ 'is-current': current, 'is-hover': forceHover }">
    <NuxtLink :to="view.to" class="resume__link" :aria-label="view.title" />
    <MusicPlaylistCover v-if="item.type === 'playlist'" :id="item.id" :title="item.title" :covers="item.covers" :kind="item.kind" :show-title="false" class="resume__art" />
    <MusicCoverArt v-else :image="view.image" size="small" :round="view.round" :alt="view.title" class="resume__art" :fallback-icon="view.round ? 'ph:user' : undefined" />
    <div class="resume__text">
      <span class="resume__title">{{ view.title }}</span>
      <span class="resume__meta">{{ meta ?? view.sub }}</span>
      <span v-if="progress != null" class="resume__bar" aria-hidden="true"><i :style="{ width: `${Math.round(progress * 100)}%` }" /></span>
    </div>
    <MusicPlayButton class="resume__play" :playing="playing" :loading="busy" :size="36" :label="`${t('common.play')}: ${view.title}`" @click="play" />
  </div>
</template>

<style scoped>
.resume {
  position: relative;
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr) auto;
  align-items: center;
  gap: 14px;
  min-width: 0;
  padding: 8px 12px 8px 8px;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  transition: background-color var(--dur-1), box-shadow var(--dur-1);
}
.resume:hover, .resume.is-hover { background: var(--fill-2); box-shadow: inset 0 0 0 1px var(--line); }
.resume__link { position: absolute; inset: 0; z-index: 1; border-radius: inherit; }
.resume__art { width: 56px; border-radius: 10px; }
.resume__art.art--round { border-radius: 50%; }
.resume__text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.resume__title { overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.resume__meta { overflow: hidden; font-size: var(--fs-small); color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.resume__bar { width: min(120px, 100%); height: 3px; margin-top: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-3); }
.resume__bar i { display: block; height: 100%; border-radius: inherit; background: var(--text-2); }
.resume__play { position: relative; z-index: 2; opacity: 0; transform: scale(0.9); transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.resume:hover .resume__play, .resume.is-hover .resume__play, .resume:focus-within .resume__play { opacity: 1; transform: none; }
.resume.is-current .resume__title { color: var(--accent); }
.resume.is-current .resume__play { opacity: 1; transform: none; }
@media (max-width: 699px) {
  .resume { grid-template-columns: 56px minmax(0, 1fr); }
  .resume__play { display: none; }
}
</style>
