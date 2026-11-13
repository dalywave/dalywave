<script setup lang="ts">
// «Подхват на другом устройстве» (stores/handoff.ts): наушники ушли с компьютера на телефон — музыка
// за ними. Настройка общая для устройств слушателя; строка — только в приложениях: вкладка сайта
// своих наушников не знает. По умолчанию — «спрашивать»: ложный перенос останавливает музыку там,
// где её слушали, а вопрос стоит одного нажатия
const bridge = useBridge()
const account = useAccountSettings()
const { t } = useI18n({ useScope: 'local' })

const on = computed({
  get: () => account.settings.handoff !== 'off',
  set: (v: boolean) => void account.update({ handoff: v ? 'suggest' : 'off' }),
})
const ask = computed({
  get: () => account.settings.handoff !== 'auto',
  set: (v: boolean) => void account.update({ handoff: v ? 'suggest' : 'auto' }),
})
</script>

<template>
  <SettingsGroup v-if="bridge.kind === 'tauri'" :title="t('group')">
    <SettingsRow :title="t('title')" :description="t('desc')">
      <UiSwitch v-model="on" :label="t('title')" hide-label />
    </SettingsRow>
    <SettingsRow v-if="on" :title="t('ask')" :description="ask ? t('askOn') : t('askOff')">
      <UiSwitch v-model="ask" :label="t('ask')" hide-label />
    </SettingsRow>
  </SettingsGroup>
</template>

<i18n lang="json">
{
  "ru": {
    "group": "Между устройствами",
    "title": "Подхват на другом устройстве",
    "desc": "Музыка идёт за наушниками",
    "ask": "Спрашивать",
    "askOn": "«Продолжить здесь?» перед переносом",
    "askOff": "Музыка переходит сама"
  },
  "en": {
    "group": "Across devices",
    "title": "Continue on another device",
    "desc": "Music follows your headphones",
    "ask": "Ask first",
    "askOn": "“Continue here?” before moving",
    "askOff": "Music moves on its own"
  }
}
</i18n>
