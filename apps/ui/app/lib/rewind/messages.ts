// Слова при числах в историях «Итогов года» (components/rewind/Story*.vue). Сами тексты историй —
// в словарях i18n/locales, раздел lib.rewind; подключает их useRewindText().

type Forms = { ru: [string, string, string], uk: [string, string, string], en: [string, string] }

/** Слова при числах: по-русски и по-украински три формы (1, 2–4, 5+), по-английски две */
export const REWIND_FORMS = {
  minutesOfMusic: { ru: ['минута музыки', 'минуты музыки', 'минут музыки'], uk: ['хвилина музики', 'хвилини музики', 'хвилин музики'], en: ['minute of music', 'minutes of music'] },
  daysWithMusic: { ru: ['день с музыкой', 'дня с музыкой', 'дней с музыкой'], uk: ['день із музикою', 'дні з музикою', 'днів із музикою'], en: ['day with music', 'days with music'] },
  inARow: { ru: ['день подряд', 'дня подряд', 'дней подряд'], uk: ['день поспіль', 'дні поспіль', 'днів поспіль'], en: ['day in a row', 'days in a row'] },
  newArtists: { ru: ['новый артист', 'новых артиста', 'новых артистов'], uk: ['новий виконавець', 'нові виконавці', 'нових виконавців'], en: ['new artist', 'new artists'] },
  minutes: { ru: ['минута', 'минуты', 'минут'], uk: ['хвилина', 'хвилини', 'хвилин'], en: ['minute', 'minutes'] },
  plays: { ru: ['прослушивание', 'прослушивания', 'прослушиваний'], uk: ['прослуховування', 'прослуховування', 'прослуховувань'], en: ['play', 'plays'] },
  times: { ru: ['раз', 'раза', 'раз'], uk: ['раз', 'рази', 'разів'], en: ['play', 'plays'] },
  days: { ru: ['день', 'дня', 'дней'], uk: ['день', 'дні', 'днів'], en: ['day', 'days'] },
  hours: { ru: ['час', 'часа', 'часов'], uk: ['година', 'години', 'годин'], en: ['hour', 'hours'] },
} satisfies Record<string, Forms>

export type RewindForm = keyof typeof REWIND_FORMS
