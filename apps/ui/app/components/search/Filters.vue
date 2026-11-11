<script lang="ts">
export type SearchType = 'all' | 'tracks' | 'artists' | 'albums' | 'playlists' | 'sound'
</script>

<script setup lang="ts">
// Фильтры результатов: что показать и где искать. Ряд прокручивается вбок на узком экране
const type = defineModel<SearchType>('type', { required: true })
const library = defineModel<boolean>('library', { required: true })
const { t } = useI18n({ useScope: 'local' })
const TYPES: SearchType[] = ['all', 'tracks', 'artists', 'albums', 'playlists']

// «По звучанию» — последний в ряду: выбранный, он не должен прятаться за краем узкого экрана
const soundChip = ref<HTMLElement | null>(null)
function revealSound() {
  const chip = soundChip.value
  const row = chip?.parentElement
  if (type.value !== 'sound' || !chip || !row) return
  row.scrollLeft = Math.max(0, chip.offsetLeft + chip.offsetWidth - row.clientWidth + 16)
}
onMounted(revealSound)
watch(type, () => void nextTick(revealSound))
</script>

<template>
  <div v-edge-fade class="filters no-scrollbar" role="toolbar" :aria-label="t('label')">
    <button type="button" class="fchip fchip--scope" :class="{ 'is-on': library }" :aria-pressed="library" @click="library = !library">
      <Icon :name="library ? 'ph:check-bold' : 'ph:books'" class="fchip__icon" />
      {{ t('library') }}
    </button>
    <span class="filters__sep" aria-hidden="true" />
    <button
      v-for="x in TYPES"
      :key="x"
      type="button"
      class="fchip"
      :class="{ 'is-on': type === x }"
      :aria-pressed="type === x"
      @click="type = x"
    >
      {{ t(`type.${x}`) }}
    </button>
    <span class="filters__sep" aria-hidden="true" />
    <!-- Поиск словами по звучанию: «спокойный инструментал для работы» -->
    <button ref="soundChip" type="button" class="fchip fchip--sound" :class="{ 'is-on': type === 'sound' }" :aria-pressed="type === 'sound'" @click="type = type === 'sound' ? 'all' : 'sound'">
      <Icon name="ph:waveform" class="fchip__icon" />
      {{ t('type.sound') }}
    </button>
  </div>
</template>

<style scoped>
.filters { display: flex; align-items: center; gap: 8px; min-width: 0; margin-block: -6px; padding-block: 6px; overflow-x: auto; overscroll-behavior-x: contain; }
.filters__sep { width: 1px; height: 20px; flex: none; margin: 0 2px; background: var(--line); }
.fchip {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  flex: none;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-2);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 550;
  color: var(--text-1);
  white-space: nowrap;
  transition: background-color var(--dur-1), color var(--dur-1);
}
/* Зона нажатия 44 pt при высоте чипа 32 */
.fchip::before { content: ''; position: absolute; inset: -6px -2px; }
.fchip:hover { background: var(--fill-3); }
.fchip:active { transform: scale(0.97); }
.fchip:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.fchip.is-on { background: var(--text-1); color: var(--on-light); }
.fchip--scope { padding-left: 11px; }
/* «В медиатеке» — переключатель, а не выбор вида: обводка вместо заливки */
.fchip--scope.is-on { background: transparent; color: var(--text-1); box-shadow: inset 0 0 0 1.5px var(--text-1); }
.fchip__icon { font-size: 15px; }
.fchip--sound { padding-left: 11px; }
.fchip--sound .fchip__icon { color: var(--accent); }
.fchip--sound.is-on .fchip__icon { color: inherit; }
</style>

<i18n lang="json">
{
  "ru": {
    "label": "Фильтры",
    "library": "В медиатеке",
    "type": { "all": "Всё", "tracks": "Треки", "artists": "Артисты", "albums": "Альбомы", "playlists": "Плейлисты", "sound": "По звучанию" }
  },
  "en": {
    "label": "Filters",
    "library": "In library",
    "type": { "all": "All", "tracks": "Songs", "artists": "Artists", "albums": "Albums", "playlists": "Playlists", "sound": "By sound" }
  }
}
</i18n>
