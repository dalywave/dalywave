// Раздел компонента в общих словарях (i18n/locales/<язык>.json): ключ ищется сначала в разделе
// (`pages.settings` + `title`), затем среди общих (`common.save`). Без Nuxt — для тестов

type Translate = (key: string, ...args: unknown[]) => string

interface Composer {
  t: unknown
  te: unknown
}

export function scopedTranslator(i18n: Composer, ns: string): { t: Translate, te: (key: string) => boolean } {
  const own = (key: string) => `${ns}.${key}`
  // Ключ компонента без перевода на текущий язык берётся из запасного (английского)
  const exists = i18n.te as (key: string, locale?: string) => boolean
  const has = (key: string) => exists(own(key)) || exists(own(key), 'en')
  const t: Translate = (key, ...args) => (i18n.t as Translate)(has(key) ? own(key) : key, ...args)
  const te = (key: string) => has(key) || exists(key)
  return { t, te }
}
