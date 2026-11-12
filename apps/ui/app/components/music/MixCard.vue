<script setup lang="ts">
import type { PlaylistSummary } from '~/api/types'

// Карточка плейлиста или микса: настоящие обложки, как в медиатеке и боковой панели (собранная —
// только когда обложек нет), название, сколько треков
const props = withDefaults(defineProps<{
  playlist: PlaylistSummary
  meta?: string
  forceHover?: boolean
}>(), {})

const { t } = useI18n()
const lang = useLang()
const { playPlaylist, isPlayingContext, loading } = usePlayActions()
const sub = computed(() => props.meta ?? props.playlist.description ?? '')
const count = computed(() => formatCount(props.playlist.tracksCount, 'track', lang.value))
</script>

<template>
  <div class="card" :class="{ 'is-hover': forceHover }">
    <NuxtLink :to="`/playlist/${playlist.id}`" class="card__link" :aria-label="playlist.title" />
    <div class="card__art">
      <MusicPlaylistCover :id="playlist.id" :title="playlist.title" :covers="playlist.covers" :kind="playlist.kind" />
      <MusicPlayButton class="card__play" :playing="isPlayingContext('playlist', playlist.id)" :loading="loading === `playlist:${playlist.id}`" :size="44" :label="`${t('common.play')}: ${playlist.title}`" @click="playPlaylist(playlist.id)" />
    </div>
    <div class="card__body">
      <div class="card__title">{{ playlist.title }}</div>
      <div class="card__sub">{{ sub || count }}</div>
    </div>
  </div>
</template>
