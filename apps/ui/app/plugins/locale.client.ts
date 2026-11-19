import { shellInfo } from '~/bridge/tauri/shell'
import { setContentLocale } from '~/utils/mixTitles'

// Язык интерфейса, выбранный в настройках, — до первой отрисовки. Не выбран — по языку системы:
// русский для русского, украинского, белорусского и казахского, иначе английский (ревьюер App Store,
// чужие серверы). На wave.daly.rest без выбора — русский
function savedLocale(): string | null {
  try {
    return localStorage.getItem('dw.locale')
  }
  catch {
    return null
  }
}

const OWNER_HOST = 'wave.daly.rest'
const RUSSIAN = /^(ru|uk|be|kk)\b/i

function serverHost(): string {
  try {
    const shell = shellInfo()
    const override = sessionStorage.getItem('dalywave.shell.server')
    // Сервер ещё не выбран (первый запуск сборки для App Store): адрес по умолчанию ни о чём не говорит
    if (shell && !override && shell.serverChosen === false) return ''
    const url = override ?? shell?.serverUrl ?? location.origin
    return new URL(url).host
  }
  catch {
    return location.host
  }
}

function systemLocale(): 'ru' | 'en' {
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language]
  return langs.some(l => RUSSIAN.test(l)) ? 'ru' : 'en'
}

export default defineNuxtPlugin(async (nuxtApp) => {
  const saved = savedLocale()
  const locale = saved === 'en' || saved === 'ru'
    ? saved
    : serverHost() === OWNER_HOST ? 'ru' : systemLocale()
  if (locale !== nuxtApp.$i18n.locale.value) await nuxtApp.$i18n.setLocale(locale)
  document.documentElement.lang = locale
  setContentLocale(locale)
  // Меню в трее и строке меню — на том же языке
  if (!saved) useBridge().setLocale(locale)
})
