<script setup lang="ts">
import type { ListFormat } from '~/composables/useExport'

// «Медиатека → Экспорт»: строка настроек и окно (на телефоне — лист снизу). Вся медиатека — архивом
// (CSV и M3U8 каждого списка и полный JSON) или одним JSON, по желанию с историей; любимые — одним файлом
const exporter = useExport()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const open = ref(false)
const format = ref<'zip' | 'json'>('zip')
const history = ref(false)
const busy = ref(false)

async function run(job: () => Promise<boolean>) {
  if (busy.value) return
  busy.value = true
  try {
    if (await job()) open.value = false
  }
  finally {
    busy.value = false
  }
}

const formats: ListFormat[] = ['csv', 'm3u8', 'json']
</script>

<template>
  <SettingsRow as="button" :title="t('row')" :description="t('rowDesc')" @click="open = true">
    <Icon name="ph:caret-right" class="ex__chev" aria-hidden="true" />
  </SettingsRow>

  <UiModal v-model:open="open" :title="t('title')" :width="440">
    <div class="ex">
      <div role="radiogroup" :aria-label="t('whole')" class="ex__group">
        <h3 class="ex__head">{{ t('whole') }}</h3>
        <SettingsRow as="button" radio :checked="format === 'zip'" :title="t('zip')" :description="t('zipDesc')" @click="format = 'zip'" />
        <SettingsRow as="button" radio :checked="format === 'json'" :title="t('json')" :description="t('jsonDesc')" @click="format = 'json'" />
        <SettingsRow :title="t('history')">
          <UiSwitch v-model="history" :label="t('history')" hide-label />
        </SettingsRow>
      </div>

      <div class="ex__group">
        <h3 class="ex__head">{{ tg('nav.liked') }}</h3>
        <div class="ex__chips">
          <UiButton v-for="f in formats" :key="f" :size="ui.isTouchUi ? 'md' : 'sm'" :disabled="busy" @click="run(() => exporter.liked(f))">
            {{ tg(`export.${f}`) }}
          </UiButton>
        </div>
        <p class="ex__note">{{ t('likedNote') }}</p>
      </div>
    </div>
    <template #footer>
      <UiButton variant="primary" :size="ui.isPhone ? 'lg' : 'md'" icon="ph:file-arrow-down" :loading="busy" class="ex__go" @click="run(() => exporter.library(format, history))">
        {{ t('download') }}
      </UiButton>
    </template>
  </UiModal>
</template>

<style scoped>
.ex__chev { font-size: 18px; color: var(--text-3); }
.ex { display: flex; flex-direction: column; gap: 24px; padding: 4px 20px 20px; }
.ex__group { display: flex; flex-direction: column; }
.ex__head { margin: 0 0 2px; font-size: var(--fs-small); font-weight: 600; color: var(--text-3); }
.ex__chips { display: flex; gap: 8px; padding-top: 8px; }
.ex__note { margin: 8px 0 0; font-size: var(--fs-caption); line-height: 1.45; color: var(--text-3); }
.ex__go { width: 100%; }
@media (min-width: 700px) {
  .ex__go { width: auto; margin-left: auto; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "row": "Экспорт",
    "rowDesc": "Медиатека в файл: CSV, M3U8, JSON",
    "title": "Экспорт медиатеки",
    "whole": "Вся медиатека",
    "zip": "Архив ZIP",
    "zipDesc": "Каждый плейлист в CSV и M3U8, всё — в JSON",
    "json": "Один файл JSON",
    "jsonDesc": "Лайки, плейлисты с папками, подписки",
    "history": "История прослушиваний",
    "likedNote": "CSV подходит для TuneMyMusic и Soundiiz, M3U8 — для плееров",
    "download": "Скачать"
  },
  "uk": {
    "row": "Експорт",
    "rowDesc": "Медіатека у файл: CSV, M3U8, JSON",
    "title": "Експорт медіатеки",
    "whole": "Уся медіатека",
    "zip": "Архів ZIP",
    "zipDesc": "Кожен плейлист у CSV і M3U8, усе — в JSON",
    "json": "Один файл JSON",
    "jsonDesc": "Вподобання, плейлисти з папками, підписки",
    "history": "Історія прослуховувань",
    "likedNote": "CSV підходить для TuneMyMusic і Soundiiz, M3U8 — для плеєрів",
    "download": "Завантажити"
  },
  "en": {
    "row": "Export",
    "rowDesc": "Your library as a file: CSV, M3U8, JSON",
    "title": "Export library",
    "whole": "Whole library",
    "zip": "ZIP archive",
    "zipDesc": "Every playlist as CSV and M3U8, everything as JSON",
    "json": "Single JSON file",
    "jsonDesc": "Likes, playlists with folders, follows",
    "history": "Listening history",
    "likedNote": "CSV works with TuneMyMusic and Soundiiz, M3U8 with media players",
    "download": "Download"
  }
}
</i18n>
