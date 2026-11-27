import { scopedTranslator } from '~/utils/i18n-scope'

/**
 * Переводы компонента из общих словарей (i18n/locales/<язык>.json): ключ ищется сначала в разделе
 * компонента (`pages.settings` + `title`), затем среди общих (`common.save`). Раздел — путь файла
 * от app/ через точку, `[id]` → `_id`
 */
export function useScopedI18n(ns: string) {
  const i18n = useNuxtApp().$i18n
  return { ...scopedTranslator(i18n, ns), locale: i18n.locale }
}
