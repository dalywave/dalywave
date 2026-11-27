// Перед тестами (bunfig.toml → [test] preload): экземпляр vue-i18n из всех словарей i18n/locales и правил
// форм — как в приложении (i18n/i18n.config.ts), чтобы утилиты (format.ts и соседи) брали слова из словарей
import { readdirSync, readFileSync } from 'node:fs'
import { createI18n } from 'vue-i18n'
import { composerTranslator, setTranslator } from '../app/utils/i18n-runtime'
import { SLAVIC, slavicPlural } from '../app/utils/plural-rules'

const dir = new URL('../i18n/locales/', import.meta.url)
const messages = Object.fromEntries(readdirSync(dir).filter(f => f.endsWith('.json'))
  .map(f => [f.slice(0, -5), JSON.parse(readFileSync(new URL(f, dir), 'utf8'))]))

const i18n = createI18n({
  legacy: false,
  locale: 'ru',
  fallbackLocale: 'en',
  // Язык без перевода в тестах — нарочно: запасной английский без предупреждений
  missingWarn: false,
  fallbackWarn: false,
  messages,
  pluralRules: Object.fromEntries(SLAVIC.map(l => [l, slavicPlural])),
})

setTranslator(composerTranslator(i18n.global))
