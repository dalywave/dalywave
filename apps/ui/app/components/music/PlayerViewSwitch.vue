<script setup lang="ts">
import type { PlayerMap } from '~/composables/usePlayerMap'
import type { PlayerView } from '~/stores/prefs'

// «Обложка / Как подобран» (usePlayerMap): на компьютере — переключатель вверху сцены,
// на телефоне и iPad — кнопка в ряду под управлением
withDefaults(defineProps<{ map: PlayerMap, icon?: boolean }>(), { icon: false })
const { t } = useI18n({ useScope: 'local' })

const options = computed(() => [
  { value: 'cover' as const, label: t('cover'), icon: 'ph:vinyl-record' },
  { value: 'map' as const, label: t('map'), icon: 'ph:graph' },
])
// Выбор — настройка устройства (prefs.playerView): карта у трека — уже по ней
const prefs = usePrefsStore()
const model = computed<PlayerView>({
  get: () => prefs.playerView,
  set: v => (prefs.playerView = v),
})
</script>

<template>
  <UiIconButton
    v-if="icon"
    icon="ph:graph"
    :label="map.on ? t('showCover') : t('map')"
    size="lg"
    :tooltip="false"
    :active="map.on"
    :pressed="map.on"
    @click="model = map.on ? 'cover' : 'map'"
  />
  <UiSegmentedControl v-else v-model="model" :options="options" :label="t('label')" size="sm" />
</template>

<i18n lang="json">
{
  "ru": {
    "label": "Что показывать",
    "cover": "Обложка",
    "map": "Как подобран",
    "showCover": "Показать обложку"
  },
  "uk": {
    "label": "Що показувати",
    "cover": "Обкладинка",
    "map": "Як підібрано",
    "showCover": "Показати обкладинку"
  },
  "en": {
    "label": "What to show",
    "cover": "Cover",
    "map": "How it was picked",
    "showCover": "Show cover"
  }
}
</i18n>
