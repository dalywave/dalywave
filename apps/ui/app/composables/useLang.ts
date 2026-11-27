import type { Locale } from '~/utils/format'

/**
 * Текущий язык интерфейса для функций форматирования.
 * Берём общий экземпляр i18n — его язык один на всё приложение
 */
export function useLang() {
  const i18n = useNuxtApp().$i18n
  return computed<Locale>(() => i18n.locale.value)
}
