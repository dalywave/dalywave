// Язык интерфейса, выбранный в настройках, — до первой отрисовки
function savedLocale(): string | null {
  try {
    return localStorage.getItem('dw.locale')
  }
  catch {
    return null
  }
}

export default defineNuxtPlugin(async (nuxtApp) => {
  const saved = savedLocale()
  if (saved === 'en' || saved === 'ru') {
    await nuxtApp.$i18n.setLocale(saved)
    document.documentElement.lang = saved
  }
})
