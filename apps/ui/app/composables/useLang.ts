import type { Locale } from '~/utils/format'

/**
 * Текущий язык интерфейса для функций форматирования.
 * Берём общий i18n: useI18n() в компоненте со своим блоком <i18n> вернул бы локальный словарь
 */
export function useLang() {
  const i18n = useNuxtApp().$i18n
  return computed<Locale>(() => (i18n.locale.value === 'en' || i18n.locale.value === 'uk' ? i18n.locale.value : 'ru'))
}
