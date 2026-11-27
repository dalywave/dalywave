<script setup lang="ts">
import type { ScreensaverBridge } from '~/bridge/types'

// «Заставка DalyWave» (приложение на Mac): живая волна, а когда играет музыка — цвет обложки и название
// в углу. «Установить» кладёт её в ~/Library/Screen Savers и открывает настройки заставки (screensaver.rs).
// В сборке без заставки (не `make app`) строки нет
const bridge = useBridge()
const toast = useToast()
const { t } = useScopedI18n('components.settings.Screensaver')

/** Только разработка: строку можно увидеть на сайте — `?screensaver=1` (снимки экранов) */
function devSaver(): ScreensaverBridge | null {
  if (!import.meta.dev || typeof location === 'undefined' || !new URLSearchParams(location.search).has('screensaver')) return null
  let installed = new URLSearchParams(location.search).get('screensaver') === 'installed'
  return {
    state: async () => ({ available: true, installed }),
    install: async () => ({ available: true, installed: (installed = true) }),
  }
}
const saver = bridge.screensaver ?? devSaver()
const state = ref<{ available: boolean, installed: boolean } | null>(null)
const busy = ref(false)

onMounted(async () => {
  if (saver) state.value = await saver.state().catch(() => null)
})

async function install() {
  if (!saver || busy.value) return
  busy.value = true
  try {
    state.value = await saver.install()
    toast.show({ text: t('done'), icon: 'ph:check-circle-fill' })
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <SettingsGroup v-if="state?.available" :title="t('group')">
    <SettingsRow :title="t('title')" :description="state.installed ? t('installed') : t('hint')">
      <UiButton size="sm" :loading="busy" @click="install">{{ state.installed ? t('open') : t('install') }}</UiButton>
    </SettingsRow>
  </SettingsGroup>
</template>
