<script setup lang="ts">
// Кнопка «Распознать» рядом с полем поиска. Только там, где есть ShazamKit (приложение для
// iPhone, iPad и Mac); на сайте и в Windows её нет
withDefaults(defineProps<{ size?: 'sm' | 'md' }>(), { size: 'md' })

const rec = useRecognizeStore()
// Узнанная песня ищется в каталоге Deezer: без него кнопки нет
const server = useServerStore()
const { t } = useI18n({ useScope: 'local' })
</script>

<template>
  <UiIconButton v-if="rec.available && server.deezer" icon="ph:microphone" :label="t('label')" :size="size" side="bottom" class="rbtn" @click="rec.start()" />
</template>

<i18n lang="json">
{
  "ru": { "label": "Распознать песню" },
  "uk": { "label": "Розпізнати пісню" },
  "en": { "label": "Recognize a song" }
}
</i18n>
