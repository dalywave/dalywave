// Переводы для утилит без Nuxt (format.ts, notifications.ts, wave-options.ts, mixTitles.ts): они зовут
// tr(), а настоящий t() общего экземпляра vue-i18n подставляет plugins/i18n-runtime.ts, в тестах —
// tests/setup.ts. Слова берутся из словарей i18n/locales; ключа нет на языке — английский

export interface TrOptions {
  /** Язык; не задан — текущий язык интерфейса */
  locale?: string
  /** Число для форм «трек | трека | треков» */
  n?: number
  /** Подстановки {name} */
  named?: Record<string, unknown>
}

export type Translator = (key: string, options: TrOptions) => string

// До подключения словарей — ключ как есть: видно, что перевод не подключён
let translator: Translator = key => key

export function setTranslator(fn: Translator) {
  translator = fn
}

export function tr(key: string, options: TrOptions = {}): string {
  return translator(key, options)
}

type ComposerT = (key: string, named: Record<string, unknown>, options: { locale?: string, plural?: number }) => string

/** Translator из экземпляра vue-i18n (composer или i18n.global): `t(key, named, { locale, plural })` */
export function composerTranslator(i18n: { t: unknown }): Translator {
  const t = i18n.t as ComposerT
  return (key, { locale, n, named }) => {
    const options: { locale?: string, plural?: number } = {}
    if (locale) options.locale = locale
    if (n !== undefined) options.plural = n
    return t(key, named ?? {}, options)
  }
}
