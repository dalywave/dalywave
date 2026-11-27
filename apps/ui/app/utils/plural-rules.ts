// Формы для чисел у славянских языков: «{n} трек | {n} трека | {n} треков» (one | few | many).
// Правило одно для русского и украинского; язык с такими же формами добавляется в SLAVIC
const rules = new Intl.PluralRules('ru-RU')

export function slavicPlural(n: number, choices: number): number {
  if (choices < 3) return Math.abs(n) === 1 ? 0 : 1
  const rule = rules.select(Math.abs(n))
  return rule === 'one' ? 0 : rule === 'few' ? 1 : 2
}

export const SLAVIC = ['ru', 'uk'] as const
