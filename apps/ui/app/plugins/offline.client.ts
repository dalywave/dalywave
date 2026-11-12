// Приложение: состояние загрузок на устройство — сразу при запуске, дальше событиями оболочки
export default defineNuxtPlugin(() => {
  if (isMiniWindow()) return
  useOfflineStore().start()
})
