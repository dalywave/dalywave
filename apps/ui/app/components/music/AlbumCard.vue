<script setup lang="ts">
import type { AlbumSummary } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Карточка альбома: обложка поднимается при наведении, справа внизу — «играть».
// Правый щелчок (под пальцем — долгое нажатие) — меню альбома
const props = withDefaults(defineProps<{
  album: AlbumSummary
  /** Подпись под названием; по умолчанию — артист и год */
  meta?: string
  badge?: 'new' | null
  forceHover?: boolean
}>(), { badge: null })

const { t } = useI18n()
const { playAlbum, isPlayingContext, loading } = usePlayActions()
const sub = computed(() => props.meta ?? [props.album.artist.name, formatYear(props.album.releaseDate)].filter(Boolean).join(' · '))
const playing = computed(() => isPlayingContext('album', props.album.id))
const shareMenu = useShare()
const menu = ref<MenuEntry[]>([])
function onMenu() {
  const a = props.album
  menu.value = [
    { label: t('common.play'), icon: 'ph:play', onSelect: () => void playAlbum(a.id) },
    { label: t('common.shuffle'), icon: 'ph:shuffle', onSelect: () => void playAlbum(a.id, { shuffle: true }) },
    { kind: 'separator' },
    { label: t('track.goArtist'), icon: 'ph:user', onSelect: () => navigateTo(`/artist/${a.artist.id}`) },
    shareMenu.entry('album', a.id, a.title),
  ]
}
</script>

<template>
  <UiContextMenu :items="menu" @open="onMenu">
  <div class="card" :class="{ 'is-hover': forceHover }">
    <NuxtLink :to="`/album/${album.id}`" class="card__link" :aria-label="`${album.title} — ${album.artist.name}`" />
    <div class="card__art">
      <MusicCoverArt :image="album.image" size="medium" :alt="album.title" />
      <UiBadge v-if="badge === 'new'" variant="new" class="card__badge">{{ t('common.new') }}</UiBadge>
      <MusicPlayButton class="card__play" :playing="playing" :loading="loading === `album:${album.id}`" :size="44" :label="`${t('common.play')}: ${album.title}`" @click="playAlbum(album.id)" />
    </div>
    <div class="card__body">
      <div class="card__title"><span class="card__name">{{ album.title }}</span><UiBadge v-if="album.explicit" variant="explicit" class="card__e" /></div>
      <div class="card__sub">{{ sub }}</div>
    </div>
  </div>
  </UiContextMenu>
</template>

<style>
.card { position: relative; display: flex; flex-direction: column; gap: 10px; min-width: 0; }
.card__link { position: absolute; inset: 0; z-index: 1; border-radius: var(--r-3); }
.card__art { position: relative; border-radius: var(--r-3); transition: transform var(--dur-3) var(--ease-out), box-shadow var(--dur-3); }
.card__art .art { width: 100%; border-radius: var(--r-3); }
.card:hover .card__art, .card.is-hover .card__art { transform: translateY(-3px); box-shadow: 0 18px 36px -18px rgba(0, 0, 0, 0.9); }
.card__play { position: absolute; right: 10px; bottom: 10px; z-index: 2; opacity: 0; transform: translateY(6px) scale(0.92); box-shadow: 0 8px 20px -6px rgba(0, 0, 0, 0.7); transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.card:hover .card__play, .card.is-hover .card__play, .card:focus-within .card__play { opacity: 1; transform: none; }
.card__badge { position: absolute; left: 10px; top: 10px; z-index: 2; }
.card__body { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding: 0 2px; }
.card__title { display: flex; align-items: center; gap: 6px; min-width: 0; overflow: hidden; font-size: var(--fs-body); font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.card__e { flex: none; }
.card__name { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
.card__sub { overflow: hidden; font-size: var(--fs-small); color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.card__sub a { position: relative; z-index: 2; }
.card__sub a:hover { color: var(--text-1); }
@media (max-width: 699px) {
  .card__play { display: none; }
}
/* Планшет под пальцем: наведения нет — «играть» видна всегда, тихим стеклянным кругом,
   обложка не подпрыгивает от залипшего :hover */
@media (pointer: coarse) and (min-width: 700px) {
  .card { -webkit-touch-callout: none; user-select: none; }
  .card .card__art .card__play {
    opacity: 1;
    transform: none;
    background: rgba(8, 10, 14, 0.55);
    color: #fff;
    -webkit-backdrop-filter: blur(14px);
    backdrop-filter: blur(14px);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
  }
  .card:hover .card__art { transform: none; box-shadow: none; }
  .card:active .card__art { transform: scale(0.98); }
}
</style>
