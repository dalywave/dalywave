// Словари и разделы компонентов: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { createI18n } from 'vue-i18n'
import en from '../i18n/locales/en.json'
import ru from '../i18n/locales/ru.json'
import uk from '../i18n/locales/uk.json'
import { scopedTranslator } from '../app/utils/i18n-scope'
import { SLAVIC, slavicPlural } from '../app/utils/plural-rules'

type Dict = Record<string, unknown>
const LOCALES: Record<string, Dict> = { ru, uk, en }

function get(d: unknown, path: string): unknown {
  return path.split('.').reduce<unknown>((o, k) => (o && typeof o === 'object' ? (o as Dict)[k] : undefined), d)
}

function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap(e =>
    e.isDirectory() ? sources(join(dir, e.name)) : /\.(vue|ts)$/.test(e.name) ? [join(dir, e.name)] : [])
}

describe('scopedTranslator', () => {
  const i18n = createI18n({
    legacy: false,
    locale: 'uk',
    fallbackLocale: 'en',
    missingWarn: false,
    fallbackWarn: false,
    messages: {
      uk: { common: { save: 'Зберегти' }, pages: { demo: { title: 'Заголовок' } } },
      en: { common: { save: 'Save' }, pages: { demo: { title: 'Title', onlyEn: 'Only in English' } } },
    },
  })
  const { t, te } = scopedTranslator(i18n.global, 'pages.demo')

  test('ключ раздела, затем общий', () => {
    expect(t('title')).toBe('Заголовок')
    expect(t('common.save')).toBe('Зберегти')
    expect(te('title')).toBe(true)
    expect(te('nothing')).toBe(false)
  })

  test('ключ раздела без перевода — из английского', () => {
    expect(t('onlyEn')).toBe('Only in English')
  })
})

describe('формы для чисел', () => {
  const i18n = createI18n({
    legacy: false,
    locale: 'ru',
    fallbackLocale: 'en',
    messages: { ru, uk, en } as never,
    pluralRules: Object.fromEntries(SLAVIC.map(l => [l, slavicPlural])),
  })
  const t = (locale: string, n: number) => i18n.global.t('pages.admin.import.stat.likes', n, { locale })

  test('русский, украинский и английский', () => {
    expect([1, 3, 5, 11, 21, 22].map(n => t('ru', n))).toEqual(['лайк', 'лайка', 'лайков', 'лайков', 'лайк', 'лайка'])
    expect([1, 3, 5].map(n => t('uk', n))).toEqual(['лайк', 'лайки', 'лайків'])
    expect([1, 3].map(n => t('en', n))).toEqual(['like', 'likes'])
  })
})

describe('ключи в компонентах', () => {
  const app = new URL('../app/', import.meta.url).pathname
  const files = sources(app).map(f => ({ f: f.slice(app.length), src: readFileSync(f, 'utf8') }))
    .filter(x => /useScopedI18n\('/.test(x.src))

  test('каждый раздел есть во всех словарях', () => {
    expect(files.length).toBeGreaterThan(100)
    for (const { f, src } of files) {
      for (const [, ns] of src.matchAll(/useScopedI18n\('([^']+)'\)/g)) {
        for (const [lang, dict] of Object.entries(LOCALES)) expect([f, lang, typeof get(dict, ns!)]).toEqual([f, lang, 'object'])
      }
    }
  })

  test("каждый t('…') находится в разделе компонента или среди общих", () => {
    const missing: string[] = []
    for (const { f, src } of files) {
      const ns = /useScopedI18n\('([^']+)'\)/.exec(src)![1]!
      for (const m of src.matchAll(/(?<![\w.$])(?:t|tl)\(\s*(['"`])([A-Za-z0-9_.]+)(\$\{)?/g)) {
        const key = m[3] ? m[2]!.replace(/\.$/, '') : m[2]!
        for (const [lang, dict] of Object.entries(LOCALES)) {
          if (get(dict, `${ns}.${key}`) === undefined && get(dict, key) === undefined) missing.push(`${f}: ${lang} ${key}`)
        }
      }
    }
    expect(missing).toEqual([])
  })
})
