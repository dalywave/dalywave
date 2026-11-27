import { shellInfo } from '~/bridge/tauri/shell'
import type { Locale } from '~/utils/format'
import { setContentLocale } from '~/utils/mixTitles'

// Язык интерфейса, выбранный в настройках, — до первой отрисовки. Не выбран — по языку системы:
// украинский для украинского, русский для русского, белорусского и казахского, иначе английский
// (ревьюер App Store, чужие серверы). На wave.daly.rest без выбора — русский
function savedLocale(): string | null {
  try {
    return localStorage.getItem('dw.locale')
  }
  catch {
    return null
  }
}

const OWNER_HOST = 'wave.daly.rest'
const UKRAINIAN = /^uk\b/i
const RUSSIAN = /^(ru|be|kk)\b/i

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

function systemLocale(): Locale {
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language]
  // Первый из знакомых языков по порядку предпочтений системы
  for (const l of langs) {
    if (UKRAINIAN.test(l)) return 'uk'
    if (RUSSIAN.test(l)) return 'ru'
  }
  return 'en'
}

export default defineNuxtPlugin(async (nuxtApp) => {
  const saved = savedLocale()
  const locale = saved === 'en' || saved === 'ru' || saved === 'uk'
    ? saved
    : serverHost() === OWNER_HOST ? 'ru' : systemLocale()
  if (locale !== nuxtApp.$i18n.locale.value) await nuxtApp.$i18n.setLocale(locale)
  document.documentElement.lang = locale
  setContentLocale(locale)
  // Меню в трее и строке меню — на том же языке
  if (!saved) useBridge().setLocale(locale)
})
