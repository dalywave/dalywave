<script setup lang="ts">
// Настройки «Друзья»: показывать ли друзьям, что я слушаю, и приватная сессия одним нажатием
// (на 6 часов или до выключения). Страница настроек и раздел «Друзья»
const props = withDefaults(defineProps<{ title?: string }>(), { title: undefined })

const friends = useFriendsStore()
const priv = usePrivateSession()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

onMounted(() => {
  friends.init()
  void friends.loadPrivacy()
})

const share = computed(() => friends.privacy?.shareListening ?? true)
const startItems = computed(() => [
  { label: tg('friends.private6h'), icon: 'ph:timer', onSelect: () => void friends.setPrivate('hours6') },
  { label: tg('friends.privateUntilOff'), icon: 'ph:infinity', onSelect: () => void friends.setPrivate('until_off') },
])
const privateDesc = computed(() => (priv.on.value ? [tg('friends.privateHint'), priv.until.value].filter(Boolean).join(' · ') : tg('friends.privateHint')))
</script>

<template>
  <SettingsGroup :title="props.title ?? tg('friends.title')">
    <SettingsRow :title="tg('friends.share')" :description="share ? undefined : t('shareOff')">
      <UiSwitch :model-value="share" :label="tg('friends.share')" hide-label :disabled="!friends.privacy" @update:model-value="friends.setShare" />
    </SettingsRow>
    <SettingsRow :title="tg('friends.private')" :description="privateDesc">
      <UiButton v-if="priv.on.value" size="sm" variant="secondary" icon="ph:eye" @click="friends.setPrivate('off')">{{ tg('friends.privateStop') }}</UiButton>
      <UiMenu v-else :items="startItems" align="end">
        <UiButton size="sm" variant="secondary" icon="ph:eye-slash" icon-right="ph:caret-down" :disabled="!friends.privacy">{{ t('start') }}</UiButton>
      </UiMenu>
    </SettingsRow>
  </SettingsGroup>
</template>

<i18n lang="json">
{
  "ru": {
    "shareOff": "Друзья видят только ваше имя",
    "start": "Включить"
  },
  "en": {
    "shareOff": "Friends only see your name",
    "start": "Turn on"
  }
}
</i18n>
