import { shellInfo } from '~/bridge/tauri/shell'
import type { Locale } from '~/utils/format'
import { setContentLocale } from '~/utils/mixTitles'

// Язык интерфейса, выбранный в настройках, — до первой отрисовки. Не выбран — по языку системы:
// первый из языков системы, на который переведён интерфейс (i18n.locales в nuxt.config.ts);
// для белорусского и казахского — русский, иначе английский (ревьюер App Store, чужие серверы).
// На wave.daly.rest без выбора — русский
function savedLocale(): string | null {
  try {
    return localStorage.getItem('dw.locale')
  }
  catch {
    return null
  }
}

const OWNER_HOST = 'wave.daly.rest'
/** Языки системы, для которых интерфейс — русский */
const AS_RUSSIAN = new Set(['be', 'kk'])

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

function systemLocale(available: readonly Locale[]): Locale {
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language]
  const find = (code: string) => available.find(c => c.toLowerCase() === code)
  // Первый из знакомых языков по порядку предпочтений системы: «pt-BR» — сначала целиком, потом «pt»
  for (const l of langs) {
    const tag = l.toLowerCase().replace('_', '-')
    const primary = tag.split('-')[0] ?? ''
    const code = find(tag) ?? find(primary) ?? (AS_RUSSIAN.has(primary) ? find('ru') : undefined)
    if (code) return code
  }
  return find('en') ?? available[0]!
}

export default defineNuxtPlugin(async (nuxtApp) => {
  const i18n = nuxtApp.$i18n
  const available = i18n.localeCodes.value
  const saved = savedLocale()
  const chosen = available.find(c => c === saved)
  const locale = chosen ?? (serverHost() === OWNER_HOST ? 'ru' : systemLocale(available))
  if (locale !== i18n.locale.value) await i18n.setLocale(locale)
  document.documentElement.lang = locale
  setContentLocale(locale)
  // Меню в трее и строке меню — на том же языке
  if (!chosen) useBridge().setLocale(locale)
})
