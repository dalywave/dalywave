<script setup lang="ts">
// «Между устройствами».
// «Где заиграет выбранная музыка» (stores/connect.ts, crates/server/src/choice.rs) — настройки этого
// устройства, хранятся на сервере и видны в «Где играть» на любом устройстве: где играть музыку,
// выбранную здесь, пока она играет на другом, и «Медиаплеер» — компьютер у колонок, которым управляют
// с других устройств.
// «Подхват на другом устройстве» (stores/handoff.ts): наушники ушли с компьютера на телефон — музыка
// за ними. Настройка общая для устройств слушателя; строки — только в приложениях: вкладка сайта
// своих наушников не знает. По умолчанию — «спрашивать»: ложный перенос останавливает музыку там,
// где её слушали, а вопрос стоит одного нажатия
import type { PlaybackDevice } from '~/bridge/types'

const bridge = useBridge()
const account = useAccountSettings()
const connect = useConnectStore()
const ui = useUiStore()
const { t } = useScopedI18n('components.settings.Handoff')
const { t: tg } = useI18n()

const on = computed({
  get: () => account.settings.handoff !== 'off',
  set: (v: boolean) => void account.update({ handoff: v ? 'suggest' : 'off' }),
})
const ask = computed({
  get: () => account.settings.handoff !== 'auto',
  set: (v: boolean) => void account.update({ handoff: v ? 'suggest' : 'auto' }),
})

type Choice = NonNullable<PlaybackDevice['choice']>
// Устройство в списке сервера появляется после связи: до неё строк нет
const self = computed(() => (connect.self && !connect.self.capabilities.remote ? connect.self : null))
const choiceOptions = computed(() => [
  { value: 'auto' as const, label: tg('devices.choiceAuto') },
  { value: 'here' as const, label: tg('devices.choiceSelf') },
  { value: 'there' as const, label: tg('devices.choiceActive') },
])
const choice = computed<Choice>({
  get: () => self.value?.choice ?? 'auto',
  set: (v) => {
    if (self.value) void connect.updateDevice(self.value.id, { choice: v })
  },
})
const choiceHint = computed(() => ({ auto: t('autoHint'), here: t('hereHint'), there: t('thereHint') })[choice.value])
const media = computed({
  get: () => !!self.value?.mediaPlayer,
  set: (v: boolean) => {
    if (self.value) void connect.updateDevice(self.value.id, { mediaPlayer: v })
  },
})
</script>

<template>
  <SettingsGroup v-if="self || bridge.kind === 'tauri'" :title="t('group')">
    <template v-if="self">
      <SettingsRow :title="tg('devices.choiceHere')" :description="choiceHint" :stack="ui.isPhone">
        <UiSegmentedControl v-model="choice" :options="choiceOptions" :label="tg('devices.choiceHere')" :block="ui.isPhone" />
      </SettingsRow>
      <SettingsRow :title="tg('devices.mediaPlayer')" :description="tg('devices.mediaPlayerHint')">
        <UiSwitch v-model="media" :label="tg('devices.mediaPlayer')" hide-label />
      </SettingsRow>
    </template>
    <template v-if="bridge.kind === 'tauri'">
      <SettingsRow :title="t('title')" :description="t('desc')">
        <UiSwitch v-model="on" :label="t('title')" hide-label />
      </SettingsRow>
      <SettingsRow v-if="on" :title="t('ask')" :description="ask ? t('askOn') : t('askOff')">
        <UiSwitch v-model="ask" :label="t('ask')" hide-label />
      </SettingsRow>
    </template>
  </SettingsGroup>
</template>
