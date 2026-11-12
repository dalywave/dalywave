<script setup lang="ts">
// Кнопка «Распознать» рядом с полем поиска. Только там, где есть ShazamKit (приложение для
// iPhone, iPad и Mac); на сайте и в Windows её нет
withDefaults(defineProps<{ size?: 'sm' | 'md' }>(), { size: 'md' })

const rec = useRecognizeStore()
const { t } = useI18n({ useScope: 'local' })
</script>

<template>
  <UiIconButton v-if="rec.available" icon="ph:microphone" :label="t('label')" :size="size" side="bottom" class="rbtn" @click="rec.start()" />
</template>

<i18n lang="json">
{
  "ru": { "label": "Распознать песню" },
  "en": { "label": "Recognize a song" }
}
</i18n>
