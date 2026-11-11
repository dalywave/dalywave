<script setup lang="ts">
import type { ImageSet } from '~/api/types'

// Строка поиска: обложка, название, подпись. Вся строка — ссылка или кнопка; справа — своё действие
const props = withDefaults(defineProps<{
  title: string
  sub?: string
  image?: ImageSet | null
  /** Плейлист: мозаика или собранная обложка */
  playlist?: { id: string, title: string, covers: ImageSet[] }
  /** Значок вместо обложки (недавний запрос) */
  icon?: string
  round?: boolean
  to?: string
  size?: 'md' | 'lg'
}>(), { size: 'md' })

const emit = defineEmits<{ open: [] }>()
const fallback = computed(() => (props.round ? 'ph:user' : 'ph:music-notes-simple'))
</script>

<template>
  <div class="sir" :class="`sir--${size}`">
    <NuxtLink v-if="to" :to="to" class="sir__hit" :aria-label="sub ? `${title}, ${sub}` : title" @click="emit('open')" />
    <button v-else type="button" class="sir__hit" :aria-label="sub ? `${title}, ${sub}` : title" @click="emit('open')" />
    <span v-if="icon" class="sir__icon" aria-hidden="true"><Icon :name="icon" /></span>
    <MusicPlaylistCover v-else-if="playlist" :id="playlist.id" class="sir__art" :title="playlist.title" :covers="playlist.covers" :show-title="false" />
    <MusicCoverArt v-else class="sir__art" :image="image" size="small" :round="round" :fallback-icon="fallback" />
    <span class="sir__main">
      <span class="sir__title ellipsis">{{ title }}</span>
      <span v-if="sub" class="sir__sub ellipsis">{{ sub }}</span>
    </span>
    <span v-if="$slots.end" class="sir__end"><slot name="end" /></span>
  </div>
</template>

<style scoped>
.sir {
  --art: 48px;
  position: relative;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 64px;
  padding: 8px;
  border-radius: var(--r-3);
  transition: background-color var(--dur-1);
}
.sir--lg { --art: 56px; min-height: 72px; }
.sir:hover { background: var(--fill-1); }
.sir:active { background: var(--fill-2); }
.sir:has(.sir__hit:focus-visible) { box-shadow: inset 0 0 0 2px var(--focus); }
.sir__hit { position: absolute; inset: 0; z-index: 1; padding: 0; border: 0; border-radius: inherit; background: none; outline: none; cursor: default; }
.sir__art, .sir__icon { width: var(--art); height: var(--art); flex: none; }
.sir__art:not(.art--round) { border-radius: var(--r-1); }
.sir__icon { display: grid; place-items: center; border-radius: var(--r-1); background: var(--fill-2); color: var(--text-2); font-size: 22px; }
.sir__main { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.sir__title { font-size: 15px; font-weight: 600; color: var(--text-1); }
.sir--lg .sir__title { font-size: 16px; }
.sir__sub { font-size: var(--fs-small); color: var(--text-2); }
.sir__end { position: relative; z-index: 2; display: flex; align-items: center; flex: none; }
@media (max-width: 699px) {
  .sir:hover { background: none; }
  .sir:active { background: var(--fill-1); }
}
</style>
