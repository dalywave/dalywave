<script setup lang="ts">
import type { HistoryItem } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Строка «Недавно прослушанного»: обложка, трек, артист и откуда играл; справа — время и «⋯».
// Нажатие на строку — слушать
const props = defineProps<{
  item: HistoryItem
  /** «из «Микс дня 2»», «Моя волна»; нет — только артист */
  context: string | null
  /** Пункты места: «Убрать из истории», «Перейти к источнику» */
  extra: () => MenuEntry[]
}>()
const emit = defineEmits<{ play: [] }>()

const player = usePlayerStore()
const lang = useLang()
const { menuFor } = useTrackActions()
const { t } = useI18n()

const track = computed(() => props.item.track)
const isCurrent = computed(() => player.current?.track.id === track.value.id)
const artists = computed(() => track.value.artists.map(a => a.name).join(', '))
const time = computed(() => new Intl.DateTimeFormat(intlLocale(lang.value, 'en-GB'), { hour: '2-digit', minute: '2-digit' }).format(new Date(props.item.startedAt)))
const menuOpen = ref(false)
const items = computed(() => (menuOpen.value ? menuFor(track.value, { extra: props.extra() }) : []))

function onClick(e: MouseEvent) {
  if ((e.target as HTMLElement).closest('button, a')) return
  emit('play')
}
</script>

<template>
  <div
    class="hr"
    :class="{ 'is-current': isCurrent, 'is-menu': menuOpen }"
    role="row"
    tabindex="0"
    @click="onClick"
    @keydown.enter="emit('play')"
  >
    <div class="hr__art" role="cell">
      <MusicCoverArt :image="track.album?.image" size="small" :alt="track.album?.title ?? ''" radius="6px" />
      <span v-if="isCurrent" class="hr__eq" :class="{ 'is-paused': !player.playing }" aria-hidden="true"><i /><i /><i /></span>
    </div>
    <div class="hr__main" role="cell">
      <div class="hr__title">
        <span class="ellipsis">{{ track.title }}</span>
        <UiBadge v-if="track.explicit" variant="explicit" />
      </div>
      <div class="hr__sub ellipsis">
        <NuxtLink v-if="track.artists[0]" :to="`/artist/${track.artists[0].id}`" class="hr__artist" @click.stop>{{ artists }}</NuxtLink>
        <template v-if="context"><span class="hr__sep">·</span><span>{{ context }}</span></template>
      </div>
    </div>
    <span class="hr__time tnum" role="cell">{{ time }}</span>
    <UiMenu v-model:open="menuOpen" :items="items">
      <UiIconButton class="hr__more" icon="ph:dots-three" :label="t('common.more')" size="sm" :tooltip="false" @click.stop />
    </UiMenu>
  </div>
</template>

<style scoped>
.hr {
  display: grid;
  grid-template-columns: 40px minmax(0, 1fr) auto auto;
  align-items: center;
  column-gap: 12px;
  min-height: 56px;
  padding: 8px;
  border-radius: var(--r-3);
  cursor: default;
  transition: background-color var(--dur-1);
}
.hr:hover, .hr.is-menu { background: var(--fill-1); }
.hr:active { background: var(--fill-2); }
.hr:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.hr__art { position: relative; width: 40px; }
.hr__eq { position: absolute; inset: 0; display: flex; align-items: flex-end; justify-content: center; gap: 2px; padding-bottom: 12px; border-radius: 6px; background: rgba(0, 0, 0, 0.45); }
.hr__eq i { width: 3px; height: 12px; border-radius: 1px; background: var(--accent); transform-origin: bottom; animation: hr-eq 0.9s ease-in-out infinite; }
.hr__eq i:nth-child(2) { animation-delay: -0.3s; }
.hr__eq i:nth-child(3) { animation-delay: -0.6s; }
.hr__eq.is-paused i { animation: none; transform: scaleY(0.35); }
@keyframes hr-eq { 0%, 100% { transform: scaleY(0.35); } 50% { transform: scaleY(1); } }
.hr__main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.hr__title { display: flex; align-items: center; gap: 6px; min-width: 0; font-weight: 550; }
.hr.is-current .hr__title { color: var(--accent); }
.hr__sub { font-size: var(--fs-small); color: var(--text-2); }
.hr__artist:hover { color: var(--text-1); text-decoration: underline; text-underline-offset: 3px; }
.hr__sep { margin: 0 6px; color: var(--text-3); }
.hr__time { font-size: var(--fs-small); color: var(--text-3); }
.hr__more { color: var(--text-2); }
.tnum { font-variant-numeric: tabular-nums; }
@media (hover: hover) {
  .hr__more { opacity: 0; }
  .hr:hover .hr__more, .hr.is-menu .hr__more, .hr__more:focus-visible { opacity: 1; }
}
@media (max-width: 699px) {
  .hr { grid-template-columns: 48px minmax(0, 1fr) auto auto; min-height: 64px; column-gap: 12px; padding: 8px 0; border-radius: 0; }
  .hr__art { width: 48px; }
  .hr__more { --size: 44px; }
}
</style>
