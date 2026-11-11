<script setup lang="ts">
import type { ArtistSummary } from '~/api/types'

// Круглое фото артиста с именем — ряд «Ваши артисты»
defineProps<{ artist: Pick<ArtistSummary, 'id' | 'name' | 'image'> }>()
</script>

<template>
  <NuxtLink :to="`/artist/${artist.id}`" class="avatar">
    <MusicCoverArt :image="artist.image" size="medium" round :alt="artist.name" fallback-icon="ph:user" class="avatar__img" />
    <span class="avatar__name ellipsis">{{ artist.name }}</span>
  </NuxtLink>
</template>

<style scoped>
.avatar { display: flex; flex-direction: column; align-items: center; gap: 8px; min-width: 0; border-radius: var(--r-3); text-align: center; outline: none; }
.avatar__img { width: 100%; transition: transform var(--dur-2) var(--ease-out); }
.avatar:hover .avatar__img { transform: scale(1.03); }
.avatar:active .avatar__img { transform: scale(0.97); }
.avatar:focus-visible .avatar__img { box-shadow: 0 0 0 2px var(--focus); }
.avatar__name { max-width: 100%; font-size: var(--fs-small); font-weight: 600; color: var(--text-1); }
</style>
