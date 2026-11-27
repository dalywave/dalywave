// Формы для чисел у русского и украинского: «{n} трек | {n} трека | {n} треков» (one | few | many).
// Ключ без перевода на текущий язык показывается по-английски
const rules = new Intl.PluralRules('ru-RU')

function slavic(n: number, choices: number): number {
  if (choices < 3) return Math.abs(n) === 1 ? 0 : 1
  const rule = rules.select(Math.abs(n))
  return rule === 'one' ? 0 : rule === 'few' ? 1 : 2
}

export default defineI18nConfig(() => ({
  fallbackLocale: 'en',
  pluralRules: { ru: slavic, uk: slavic },
}))
