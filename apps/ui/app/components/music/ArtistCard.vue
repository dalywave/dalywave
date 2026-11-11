<script setup lang="ts">
import type { ArtistSummary } from '~/api/types'

// Карточка артиста: круглое фото, имя, подпись; «играть» — лучшие треки
const props = withDefaults(defineProps<{
  artist: ArtistSummary
  meta?: string
  forceHover?: boolean
}>(), {})

const { t } = useI18n()
const lang = useLang()
const library = useLibraryStore()
const { playArtist, isPlayingContext, loading } = usePlayActions()
const sub = computed(() => {
  if (props.meta) return props.meta
  if (library.isFollowed(props.artist)) return t('artist.following')
  return props.artist.fans ? formatCount(props.artist.fans, 'listener', lang.value) : t('record.artist')
})
</script>

<template>
  <div class="card card--artist" :class="{ 'is-hover': forceHover }">
    <NuxtLink :to="`/artist/${artist.id}`" class="card__link" :aria-label="artist.name" />
    <div class="card__art">
      <MusicCoverArt :image="artist.image" size="medium" :alt="artist.name" round fallback-icon="ph:user" />
      <MusicPlayButton class="card__play" :playing="isPlayingContext('artist', artist.id)" :loading="loading === `artist:${artist.id}`" :size="44" :label="`${t('common.play')}: ${artist.name}`" @click="playArtist(artist.id)" />
    </div>
    <div class="card__body">
      <div class="card__title">{{ artist.name }}</div>
      <div class="card__sub">{{ sub }}</div>
    </div>
  </div>
</template>

<style>
.card--artist { align-items: center; text-align: center; }
.card--artist .card__art, .card--artist .card__art .art { width: 100%; border-radius: 50%; }
.card--artist .card__link { border-radius: 50%; }
.card--artist .card__body { align-items: center; width: 100%; }
.card--artist .card__title { justify-content: center; }
.card--artist .card__play { right: 4px; bottom: 4px; }
</style>
