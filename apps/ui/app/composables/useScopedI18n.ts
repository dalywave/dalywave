/**
 * Переводы компонента из общих словарей (i18n/locales/<язык>.json): ключ ищется сначала в разделе
 * компонента (`pages.settings` + `title`), затем среди общих (`common.save`). Раздел — путь файла
 * от app/ через точку, `[id]` → `_id`
 */
export function useScopedI18n(ns: string) {
  const i18n = useNuxtApp().$i18n
  const own = (key: string) => `${ns}.${key}`
  // Ключ компонента без перевода на текущий язык берётся из запасного (английского)
  const has = (key: string) => i18n.te(own(key)) || i18n.te(own(key), 'en')
  const t = ((key: string, ...args: unknown[]) =>
    (i18n.t as (k: string, ...a: unknown[]) => string)(has(key) ? own(key) : key, ...args)) as typeof i18n.t
  const te = (key: string) => has(key) || i18n.te(key)
  return { t, te, locale: i18n.locale }
}
