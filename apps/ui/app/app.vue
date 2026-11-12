<script setup lang="ts">
import { TooltipProvider } from 'reka-ui'

// Корень: подсказки, уведомления и тихое свечение цвета обложки за интерфейсом
const bridge = useBridge()
const { t } = useI18n()

useHead({
  titleTemplate: title => (title ? `${title} — DalyWave` : 'DalyWave'),
  htmlAttrs: { 'data-platform': bridge.platform, 'data-app': bridge.isDesktopApp ? 'desktop' : 'web' },
})

const offline = ref(false)
onMounted(() => {
  offline.value = !navigator.onLine
  window.addEventListener('online', () => (offline.value = false))
  window.addEventListener('offline', () => (offline.value = true))
})
</script>

<template>
  <TooltipProvider :delay-duration="450" :skip-delay-duration="200">
    <div class="ambient" aria-hidden="true" />
    <NuxtLayout>
      <NuxtPage />
    </NuxtLayout>
    <div v-if="offline" class="offline" role="status"><Icon name="ph:wifi-slash" />{{ t('common.offline') }}</div>
    <UiToaster />
    <MusicPlaylistPicker />
    <MusicPlaylistSet />
    <MusicReportRecording />
    <MusicSharedLink />
    <UiConfirm />
    <UiPrompt />
  </TooltipProvider>
</template>

<style>
.offline {
  position: fixed;
  top: calc(var(--safe-top) + 8px);
  left: 50%;
  z-index: 260;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border-radius: var(--r-pill);
  background: var(--glass-strong);
  -webkit-backdrop-filter: blur(24px);
  backdrop-filter: blur(24px);
  box-shadow: var(--shadow-pop);
  font-size: var(--fs-caption);
  font-weight: 600;
  color: var(--warning);
  transform: translateX(-50%);
}
</style>
