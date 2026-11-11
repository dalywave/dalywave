// Приложение: состояние загрузок на устройство — сразу при запуске, дальше событиями оболочки
export default defineNuxtPlugin(() => {
  useOfflineStore().start()
})
