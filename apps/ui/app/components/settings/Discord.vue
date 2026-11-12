<script setup lang="ts">
// «Статус в Discord»: «Слушает DalyWave» с треком, обложкой и прогрессом. Только приложение для
// компьютера (локальный IPC Discord, apps/shell/src/discord.rs); на сайте и телефоне строки нет.
// Пока в сборке нет id приложения Discord (`available: false`) — тоже нет
interface DiscordState { available: boolean, enabled: boolean, connected: boolean }

const bridge = useBridge()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })

const state = ref<DiscordState | null>(null)
const busy = ref(false)

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(cmd, args)
}

onMounted(async () => {
  if (bridge.kind !== 'tauri' || !bridge.isDesktopApp) return
  state.value = await call<DiscordState>('discord_state').catch(() => null)
})

const enabled = computed({
  get: () => state.value?.enabled ?? false,
  set: (v: boolean) => {
    if (busy.value || !state.value) return
    busy.value = true
    const before = state.value
    state.value = { ...before, enabled: v }
    call<DiscordState>('discord_set_enabled', { enabled: v })
      .then(s => (state.value = s))
      .catch(() => {
        state.value = before
        toast.error(t('failed'))
      })
      .finally(() => (busy.value = false))
  },
})
</script>

<template>
  <SettingsGroup v-if="state?.available" :title="t('group')">
    <SettingsRow :title="t('title')">
      <UiSwitch v-model="enabled" :label="t('title')" hide-label :disabled="busy" />
    </SettingsRow>
  </SettingsGroup>
</template>

<i18n lang="json">
{
  "ru": {
    "group": "Интеграции",
    "title": "Статус в Discord",
    "failed": "Не получилось переключить"
  },
  "en": {
    "group": "Integrations",
    "title": "Discord status",
    "failed": "Couldn’t switch it"
  }
}
</i18n>
