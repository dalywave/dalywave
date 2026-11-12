<script lang="ts">
import type { ImageSet, PlaylistSummary } from '~/api/types'

export type LibraryKind = 'playlist' | 'album' | 'artist'

/** Элемент медиатеки: плейлист, сохранённый альбом или артист из подписок */
export interface LibraryItem {
  key: string
  kind: LibraryKind
  id: string
  title: string
  sub: string
  /** Короче — для узкой плитки в сетке на телефоне */
  short?: string
  pinned: boolean
  /** Когда попал в медиатеку — для сортировки и колонки «Добавлено» */
  added: string
  playlist?: PlaylistSummary
  image?: ImageSet | null
  explicit?: boolean
}
</script>

<script setup lang="ts">
// Плейлист, альбом или артист в медиатеке: плиткой в сетке или строкой списка.
// Обложки настоящие: у плейлиста — мозаика 2×2 из обложек треков, у артиста — круглое фото
const props = defineProps<{
  item: LibraryItem
  layout: 'grid' | 'list'
  /** Колонка «Добавлено» в списке на широком экране */
  date?: string
}>()

const { t } = useI18n()
const ui = useUiStore()
const { playAlbum, playArtist, playPlaylist, isPlayingContext, loading } = usePlayActions()

const to = computed(() => `/${props.item.kind}/${props.item.id}`)
const playing = computed(() => isPlayingContext(props.item.kind, props.item.id))
const busy = computed(() => loading.value === `${props.item.kind}:${props.item.id}`)
const round = computed(() => props.item.kind === 'artist')
const sub = computed(() => (ui.isPhone && props.layout === 'grid' ? props.item.short ?? props.item.sub : props.item.sub))

function play() {
  if (props.item.kind === 'album') return playAlbum(props.item.id)
  if (props.item.kind === 'artist') return playArtist(props.item.id)
  return playPlaylist(props.item.id)
}
</script>

<template>
  <div class="le" :class="[`le--${layout}`, { 'le--round': round, 'is-playing': playing }]" :data-key="item.key">
    <NuxtLink :to="to" class="le__link" :aria-label="item.title" />
    <div class="le__art">
      <MusicPlaylistCover
        v-if="item.playlist"
        :id="item.playlist.id"
        :title="item.playlist.title"
        :covers="item.playlist.covers"
        :image="item.playlist.image"
        :kind="item.playlist.kind"
        :show-title="layout === 'grid'"
        class="le__cover"
      />
      <MusicCoverArt
        v-else
        :image="item.image"
        :size="layout === 'grid' ? 'medium' : 'small'"
        :alt="item.title"
        :round="round"
        :fallback-icon="round ? 'ph:user' : 'ph:vinyl-record'"
        class="le__cover"
      />
      <MusicPlayButton
        v-if="!ui.isPhone"
        class="le__play"
        :playing="playing"
        :loading="busy"
        :size="layout === 'grid' ? 44 : 32"
        :label="`${t('common.play')}: ${item.title}`"
        @click="play"
      />
    </div>
    <div class="le__body">
      <div class="le__title">
        <span class="ellipsis">{{ item.title }}</span>
        <UiBadge v-if="item.explicit" variant="explicit" class="le__e" />
      </div>
      <div class="le__sub">
        <Icon v-if="item.pinned" name="ph:push-pin-fill" class="le__pin" />
        <span class="ellipsis">{{ sub }}</span>
      </div>
    </div>
    <span v-if="layout === 'list' && date" class="le__date tnum">{{ date }}</span>
  </div>
</template>

<style scoped>
.le { position: relative; display: flex; min-width: 0; -webkit-tap-highlight-color: transparent; }
.le__link { position: absolute; inset: 0; z-index: 1; border-radius: var(--r-3); outline: none; }
.le__link:focus-visible { box-shadow: 0 0 0 2px var(--focus); }
.le__art { position: relative; flex: none; }
.le__cover { width: 100%; }
.le__play { position: absolute; z-index: 2; opacity: 0; box-shadow: 0 8px 20px -6px rgba(0, 0, 0, 0.7); transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.le:hover .le__play, .le:focus-within .le__play, .le.is-playing .le__play { opacity: 1; transform: none; }
.le__body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.le__title { display: flex; align-items: center; gap: 6px; min-width: 0; font-weight: 600; color: var(--text-1); }
.le.is-playing .le__title { color: var(--accent); }
.le__e { flex: none; }
.le__sub { display: flex; align-items: center; gap: 5px; min-width: 0; font-size: var(--fs-small); color: var(--text-2); }
.le__pin { flex: none; font-size: 12px; color: var(--accent); }

/* Сетка: обложка сверху, подпись под ней */
.le--grid { flex-direction: column; gap: 10px; }
.le--grid .le__art { width: 100%; border-radius: var(--r-3); transition: transform var(--dur-3) var(--ease-out), box-shadow var(--dur-3); }
.le--grid .le__cover { border-radius: var(--r-3); }
.le--grid:hover .le__art { transform: translateY(-3px); box-shadow: 0 18px 36px -18px rgba(0, 0, 0, 0.9); }
.le--grid:active .le__art { transform: scale(0.98); }
.le--grid .le__play { right: 10px; bottom: 10px; transform: translateY(6px) scale(0.92); }
.le--grid .le__body { padding: 0 2px; }
.le--grid .le__title { font-size: var(--fs-body); }
.le--grid.le--round { align-items: center; text-align: center; }
.le--grid.le--round .le__art, .le--grid.le--round .le__cover, .le--grid.le--round .le__link { border-radius: 50%; }
.le--grid.le--round .le__link { bottom: auto; aspect-ratio: 1; }
.le--grid.le--round .le__body { align-items: center; width: 100%; }
.le--grid.le--round .le__title, .le--grid.le--round .le__sub { justify-content: center; }
.le--grid.le--round .le__play { right: 4px; bottom: 4px; }

/* Список: обложка слева, подписи, дата справа */
.le--list { align-items: center; gap: 14px; min-height: 64px; padding: 8px; margin: 0 -8px; border-radius: var(--r-3); transition: background-color var(--dur-1); }
.le--list:hover { background: var(--fill-1); }
.le--list:active { background: var(--fill-2); }
.le--list .le__art { width: 48px; }
.le--list .le__cover { border-radius: var(--r-2); }
.le--list.le--round .le__cover { border-radius: 50%; }
.le--list .le__play { inset: 0; margin: auto; transform: scale(0.9); }
.le--list .le__body { flex: 1; }
.le--list .le__title { font-size: var(--fs-body); }
.le__date { flex: none; width: 112px; font-size: var(--fs-small); color: var(--text-3); text-align: right; }

@media (max-width: 699px) {
  .le--grid { gap: 8px; }
  .le--grid .le__title { font-size: var(--fs-small); }
  .le--grid .le__sub { font-size: var(--fs-caption); }
  .le--list { gap: 12px; min-height: 72px; padding: 6px 8px; }
  .le--list .le__art { width: 60px; }
  .le--list .le__title { font-size: 15px; font-weight: 550; }
}
</style>
