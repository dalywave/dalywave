// Тема оформления: выбор в настройках (или тема системы при «как в системе») → data-theme на
// корне, theme-color, и тема окна приложения — «светофор» и фон окна на macOS, Mica и заголовок на
// Windows, строка состояния и фон под WebView на iPhone и iPad. До первой отрисовки тему уже
// поставил скрипт в <head> (utils/theme.ts), здесь — дальнейшие смены
export default defineNuxtPlugin(() => {
  const prefs = usePrefsStore()
  const bridge = useBridge()

  const mq = matchMedia('(prefers-color-scheme: light)')
  const systemLight = ref(mq.matches)
  mq.addEventListener('change', e => (systemLight.value = e.matches))

  const theme = computed(() => resolveTheme(prefs.theme, systemLight.value))
  watch(theme, t => applyTheme(t), { immediate: true })
  // Окну — выбор как есть: при «как в системе» оно само следит за системой
  watch(() => prefs.theme, p => bridge.window.setTheme(p === 'system' ? null : p), { immediate: true })
})
