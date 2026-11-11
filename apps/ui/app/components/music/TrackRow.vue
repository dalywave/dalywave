<script setup lang="ts">
import type { Track } from '~/api/types'

// Строка трека — одна для подборок, очереди, альбомов и поиска.
// Под названием — артист и почему трек здесь; справа — состояние на сервере и длительность
const props = withDefaults(defineProps<{
  track: Track
  index?: number
  variant?: 'default' | 'compact' | 'queue' | 'album' | 'wide'
  reason?: string
  /** Задать «играет» снаружи (галерея); иначе — по плееру */
  playing?: boolean
  queueUid?: string
  forceHover?: boolean
}>(), { variant: 'default' })

const emit = defineEmits<{ play: [] }>()
const player = usePlayerStore()
const library = useLibraryStore()
const { menuFor } = useTrackActions()
const { t } = useI18n()

const isCurrent = computed(() => props.playing ?? player.current?.track.id === props.track.id)
const isPlaying = computed(() => isCurrent.value && (props.playing ?? player.playing))
const liked = computed(() => library.isLiked(props.track))
const status = computed(() => library.statusOf(props.track))
const remote = computed(() => status.value === 'none')
const unavailable = computed(() => status.value === 'unavailable')
const artists = computed(() => props.track.artists.map(a => a.name).join(', '))
// «Remix», «(Live)» — приглушённо после названия, в скобках
const version = computed(() => {
  const v = props.track.version?.trim()
  if (!v) return ''
  return v.startsWith('(') || v.startsWith('[') ? v : `(${v})`
})
const menuOpen = ref(false)
const items = computed(() => (menuOpen.value ? menuFor(props.track, { inQueue: props.queueUid }) : []))

function onRowClick(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('button, a')) return
  if (!unavailable.value) emit('play')
}

function onKey(e: KeyboardEvent) {
  if (e.key === 'Enter' && !unavailable.value) emit('play')
}
</script>

<template>
  <div
    class="tr"
    :class="[`tr--${variant}`, { 'is-current': isCurrent, 'is-remote': remote, 'is-unavailable': unavailable, 'is-hover': forceHover, 'is-menu': menuOpen }]"
    role="row"
    tabindex="0"
    :aria-current="isCurrent ? 'true' : undefined"
    @click="onRowClick"
    @keydown="onKey"
  >
    <div v-if="variant !== 'compact' && variant !== 'queue'" class="tr__lead" role="cell">
      <span class="tr__idx tnum">{{ index ?? '' }}</span>
      <span class="tr__eq" :class="{ 'is-paused': !isPlaying }" aria-hidden="true"><i /><i /><i /></span>
      <button type="button" class="tr__play" :aria-label="`${t('common.play')}: ${track.title}`" :disabled="unavailable" @click="emit('play')">
        <Icon :name="isPlaying ? 'ph:pause-fill' : 'ph:play-fill'" />
      </button>
    </div>
    <div v-if="variant !== 'album'" class="tr__art" role="cell">
      <MusicCoverArt :image="track.album?.image" size="small" :alt="track.album?.title ?? ''" radius="6px" />
      <span v-if="variant === 'queue' || variant === 'compact'" class="tr__art-eq" :class="{ 'is-on': isCurrent, 'is-paused': !isPlaying }" aria-hidden="true"><i /><i /><i /></span>
    </div>
    <div class="tr__main" role="cell">
      <div class="tr__title">
        <span class="ellipsis">{{ track.title }}<span v-if="version" class="tr__version">{{ version }}</span></span>
        <UiBadge v-if="track.explicit" variant="explicit" />
      </div>
      <div class="tr__sub ellipsis">
        <NuxtLink v-if="track.artists[0]" :to="`/artist/${track.artists[0].id}`" class="tr__artist" @click.stop>{{ artists }}</NuxtLink>
        <template v-if="unavailable"><span class="tr__sep">·</span><span class="tr__note">{{ t('track.status.unavailable') }}</span></template>
        <template v-else-if="reason"><span class="tr__sep">·</span><span class="tr__reason">{{ typographRu(reason) }}</span></template>
      </div>
    </div>
    <div v-if="variant === 'wide'" class="tr__album ellipsis" role="cell">
      <NuxtLink v-if="track.album" :to="`/album/${track.album.id}`" @click.stop>{{ track.album.title }}</NuxtLink>
    </div>
    <div class="tr__end" role="cell">
      <!-- Место под состояние есть всегда: колонки в широком списке не гуляют от строки к строке -->
      <span class="tr__slot">
        <UiTooltip v-if="status === 'none'" :text="t('track.status.none')">
          <span class="tr__state tr__state--cloud" role="img" :aria-label="t('track.status.none')"><Icon name="ph:cloud-arrow-down" /></span>
        </UiTooltip>
        <UiTooltip v-else-if="status === 'queued' || status === 'acquiring'" :text="t(`track.status.${status}`)">
          <span class="tr__state"><UiProgressRing :state="status === 'queued' ? 'queued' : 'progress'" :value="0.42" :size="18" :label="t(`track.status.${status}`)" /></span>
        </UiTooltip>
        <UiTooltip v-else-if="status === 'disputed' || status === 'failed'" :text="t(`track.status.${status}`)">
          <span class="tr__state tr__state--warn" role="img" :aria-label="t(`track.status.${status}`)"><Icon name="ph:warning-circle" /></span>
        </UiTooltip>
      </span>
      <UiIconButton
        class="tr__like"
        :class="{ 'is-liked': liked }"
        :icon="liked ? 'ph:heart-fill' : 'ph:heart'"
        :label="liked ? t('player.unlike') : t('player.like')"
        :pressed="liked"
        :tone="liked ? 'liked' : 'default'"
        size="sm"
        @click.stop="player.toggleLike(track)"
      />
      <span class="tr__dur tnum">{{ formatDuration(track.durationMs) }}</span>
      <UiMenu v-model:open="menuOpen" :items="items">
        <UiIconButton class="tr__more" icon="ph:dots-three" :label="t('common.more')" size="sm" :tooltip="false" @click.stop />
      </UiMenu>
    </div>
  </div>
</template>

<style scoped>
.tr {
  --cover: 40px;
  position: relative;
  display: grid;
  grid-template-columns: 28px var(--cover) minmax(0, 1fr) auto;
  align-items: center;
  column-gap: 12px;
  min-height: 56px;
  padding: 8px 8px 8px 10px;
  border-radius: var(--r-3);
  outline: none;
  cursor: default;
  container-type: inline-size;
  transition: background-color var(--dur-1);
}
.tr--wide { grid-template-columns: 28px var(--cover) minmax(0, 1.4fr) minmax(0, 1fr) auto; }
.tr--album { grid-template-columns: 28px minmax(0, 1fr) auto; min-height: 52px; }
.tr--compact, .tr--queue { grid-template-columns: var(--cover) minmax(0, 1fr) auto; padding-left: 8px; }
.tr--queue { cursor: grab; }
.tr:hover, .tr.is-hover, .tr:focus-visible, .tr.is-menu { background: var(--fill-1); }
.tr:focus-visible { box-shadow: inset 0 0 0 2px var(--focus); }

.tr__lead { position: relative; display: grid; place-items: center; width: 28px; height: 28px; color: var(--text-3); font-size: var(--fs-small); }
.tr__lead > * { grid-area: 1 / 1; transition: opacity var(--dur-1); }
.tr__eq, .tr__art-eq { display: inline-flex; align-items: flex-end; gap: 2px; width: 14px; height: 14px; color: var(--accent); opacity: 0; }
.tr__eq i, .tr__art-eq i { flex: 1; height: 100%; background: currentColor; border-radius: 1px; transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.tr__eq i:nth-child(2), .tr__art-eq i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.tr__eq i:nth-child(3), .tr__art-eq i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
.tr__eq.is-paused i, .tr__art-eq.is-paused i { animation-play-state: paused; }
.tr__play { display: grid; place-items: center; width: 28px; height: 28px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-1); font-size: 16px; opacity: 0; }
.tr:hover .tr__idx, .tr.is-hover .tr__idx, .tr:focus-within .tr__idx { opacity: 0; }
.tr:hover .tr__play, .tr.is-hover .tr__play, .tr:focus-within .tr__play { opacity: 1; }
.tr.is-current .tr__idx { opacity: 0; }
.tr.is-current .tr__eq { opacity: 1; }
.tr.is-current:hover .tr__eq, .tr.is-current.is-hover .tr__eq { opacity: 0; }

.tr__art { position: relative; width: var(--cover); height: var(--cover); }
.tr__art-eq { position: absolute; inset: 0; display: grid; place-items: center; width: auto; height: auto; padding: 13px 12px; border-radius: 6px; background: rgba(4, 5, 7, 0.58); }
.tr__art-eq { display: none; }
.tr__art-eq.is-on { display: flex; opacity: 1; }

.tr__main { min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.tr__title { display: flex; align-items: center; gap: 6px; min-width: 0; font-weight: 550; font-size: var(--fs-body); }
.tr__version { margin-left: 0.3em; color: var(--text-3); font-weight: 450; }
.tr.is-current .tr__title { color: var(--accent); }
.tr__sub { font-size: var(--fs-small); color: var(--text-2); }
.tr__artist:hover { color: var(--text-1); text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.tr__sep { margin: 0 5px; color: var(--text-4); }
.tr__reason, .tr__note { color: var(--text-3); }
.tr__album { font-size: var(--fs-small); color: var(--text-2); }
.tr__album a:hover { color: var(--text-1); }

.tr__end { display: flex; align-items: center; gap: 4px; }
.tr__slot { display: inline-grid; place-items: center; width: 24px; height: 24px; flex: none; }
.tr__state { display: inline-grid; place-items: center; width: 22px; height: 22px; color: var(--text-3); font-size: 17px; flex: none; }
.tr__state--warn { color: var(--warning); }
.tr__dur { width: 40px; text-align: right; font-size: var(--fs-small); color: var(--text-3); }
.tr :deep(.tr__like), .tr :deep(.tr__more) { opacity: 0; }
.tr:hover :deep(.tr__like), .tr.is-hover :deep(.tr__like), .tr:focus-within :deep(.tr__like), .tr.is-menu :deep(.tr__like),
.tr:hover :deep(.tr__more), .tr.is-hover :deep(.tr__more), .tr:focus-within :deep(.tr__more), .tr.is-menu :deep(.tr__more),
.tr :deep(.tr__like.is-liked) { opacity: 1; }

.tr.is-remote :deep(.art) { filter: saturate(0.45) brightness(0.82); }
.tr.is-remote .tr__title { color: var(--text-2); }
.tr.is-unavailable { opacity: 0.45; }

/* Очередь и узкие колонки: без сердца; «ещё» заменяет длительность при наведении */
.tr--queue :deep(.tr__like) { display: none; }
.tr--queue :deep(.tr__more) { display: none; opacity: 1; }
.tr--queue:hover :deep(.tr__more), .tr--queue:focus-within :deep(.tr__more), .tr--queue.is-menu :deep(.tr__more) { display: inline-grid; }
.tr--queue:hover .tr__dur, .tr--queue:focus-within .tr__dur, .tr--queue.is-menu .tr__dur { display: none; }

@container (max-width: 520px) {
  .tr__note { display: none; }
}

@media (max-width: 699px) {
  .tr { min-height: 60px; }
  .tr--wide { grid-template-columns: var(--cover) minmax(0, 1fr) auto; }
  .tr--wide .tr__lead, .tr--wide .tr__album { display: none; }
  .tr :deep(.tr__like), .tr__dur { display: none; }
  .tr :deep(.tr__more) { opacity: 1; --size: 44px; }
}
</style>
