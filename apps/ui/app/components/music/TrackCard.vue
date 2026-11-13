<script setup lang="ts">
import type { Track } from '~/api/types'
import type { HookOrigin } from '~/stores/hookPreview'

// Карточка трека для полок («Открытия недели», «Радар новинок», «Забытые любимые»): обложка альбома,
// название, артист. Нажатие — играть полку с этого трека (решает родитель), правый клик и «⋯» — меню трека
const props = withDefaults(defineProps<{
  track: Track
  badge?: 'new' | null
  /** Подпись вместо артиста */
  meta?: string
  /** «Послушать припев»: долгое нажатие пальцем на карточку или удержание ▶ мышью */
  hook?: boolean
  /** Откуда отрывок — «Слушать» в нём включает полку с этого трека */
  origin?: () => HookOrigin
}>(), { badge: null })
const emit = defineEmits<{ play: [] }>()

const { t } = useI18n()
const player = usePlayerStore()
const { menuFor } = useTrackActions()
const isCurrent = computed(() => player.current?.track.id === props.track.id)
const sub = computed(() => props.meta ?? props.track.artists.map(a => a.name).join(', '))
const menu = computed(() => menuFor(props.track))
const press = useHookPress(() => props.track, () => props.origin?.() ?? null)
const hookStore = useHookPreviewStore()
const hooked = computed(() => hookStore.track?.id === props.track.id)
function onDown(e: PointerEvent, mouse = false) {
  if (props.hook) press.down(e, mouse)
}
</script>

<template>
  <UiContextMenu :items="menu">
    <div
      class="card tcard"
      :class="{ 'is-current': isCurrent, 'is-hookable': hook, 'is-hooked': hooked }"
      @click.capture="press.click"
      @pointerdown="onDown"
      @pointermove="press.move"
      @pointerup="press.up"
      @pointercancel="press.up"
      @pointerleave="press.up"
      @contextmenu="press.context"
    >
      <button type="button" class="card__link" :aria-label="`${t('common.play')}: ${track.title} — ${sub}`" @click="emit('play')" />
      <div class="card__art">
        <MusicCoverArt :image="track.album?.image ?? null" size="medium" :alt="track.album?.title ?? track.title" />
        <UiBadge v-if="badge === 'new'" variant="new" class="card__badge">{{ t('common.new') }}</UiBadge>
        <MusicPlayButton class="card__play" :playing="isCurrent && player.playing" :size="44" :label="`${t('common.play')}: ${track.title}`" @pointerdown="onDown($event, true)" @click="isCurrent ? player.toggle() : emit('play')" />
      </div>
      <div class="card__body">
        <div class="card__title">{{ track.title }}<UiBadge v-if="track.explicit" variant="explicit" class="card__e" /></div>
        <div class="card__sub">{{ sub }}</div>
      </div>
    </div>
  </UiContextMenu>
</template>

<style scoped>
.tcard .card__link { border: 0; padding: 0; background: transparent; cursor: pointer; }
.tcard .card__link:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.tcard.is-current .card__title { color: var(--accent); }
.tcard.is-hooked .card__title { color: var(--accent); }
@media (pointer: coarse) {
  .tcard.is-hookable { -webkit-touch-callout: none; user-select: none; }
}
</style>
