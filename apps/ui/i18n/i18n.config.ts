import { SLAVIC, slavicPlural } from '../app/utils/plural-rules'

// Ключ без перевода на текущий язык показывается по-английски; формы для чисел — app/utils/plural-rules.ts
export default defineI18nConfig(() => ({
  fallbackLocale: 'en',
  pluralRules: Object.fromEntries(SLAVIC.map(l => [l, slavicPlural])),
}))
