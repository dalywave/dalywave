import type { ImageSet, Rewind, Track } from '~/api/types'
import { REWIND_FORMS, REWIND_MESSAGES, type RewindForm } from '~/lib/rewind/messages'

const MONTH_IN = ['январе', 'феврале', 'марте', 'апреле', 'мае', 'июне', 'июле', 'августе', 'сентябре', 'октябре', 'ноябре', 'декабре']
const MONTH_IN_UK = ['січні', 'лютому', 'березні', 'квітні', 'травні', 'червні', 'липні', 'серпні', 'вересні', 'жовтні', 'листопаді', 'грудні']

/**
 * Тексты, числа и картинки историй «Итогов года» — общие для трёх стилей. Это единственный
 * вызов useI18n в компоненте стиля: второй локальный словарь vue-i18n в компоненте не поддерживает
 */
export function useRewindText() {
  const { t } = useI18n({ useScope: 'local', messages: REWIND_MESSAGES })
  const bridge = useBridge()
  const lang = useLang()
  const en = computed(() => lang.value === 'en')
  const uk = computed(() => lang.value === 'uk')
  const locale = computed(() => intlLocale(lang.value, 'en-GB'))

  const img = (set: ImageSet | null | undefined, size: 'small' | 'medium' | 'large' = 'large') => (set?.[size] ? bridge.resolveUrl(set[size]) : null)
  const cover = (tr: Track | null | undefined, size: 'small' | 'medium' | 'large' = 'large') => img(tr?.album?.image, size)
  const artistsOf = (tr: Track) => tr.artists.filter(a => a.role !== 'featured').map(a => a.name).join(', ') || tr.artists.map(a => a.name).join(', ')
  const num = (n: number) => formatNumber(n, lang.value)
  const pct = (share: number) => `${Math.round(share * 100)}${NBSP}%`
  /** Слово при числе: «12 480 минут», «3 дня подряд» — без самого числа */
  const word = (key: RewindForm, n: number) => {
    const f = REWIND_FORMS[key]
    return en.value ? f.en[Math.abs(n) === 1 ? 0 : 1]! : plural(n, uk.value ? f.uk : f.ru)
  }
  /** Число со словом через неразрывный пробел */
  const count = (key: RewindForm, n: number) => `${num(n)}${NBSP}${word(key, n)}`
  const date = (iso: string) => new Date(`${iso.slice(0, 10)}T12:00:00Z`)
  const day = (iso: string) => new Intl.DateTimeFormat(locale.value, { day: 'numeric', month: 'long', timeZone: 'UTC' }).format(date(iso))
  const dayShort = (iso: string) => new Intl.DateTimeFormat(locale.value, { day: 'numeric', month: 'short', timeZone: 'UTC' }).format(date(iso))
  const monthName = (m: number) => new Intl.DateTimeFormat(locale.value, { month: 'long', timeZone: 'UTC' }).format(new Date(Date.UTC(2026, m - 1, 15)))
  const monthLetter = (m: number) => monthName(m).charAt(0).toUpperCase()
  /** «в марте» / «у березні» / «in March» */
  const monthIn = (m: number) => (en.value ? `in ${monthName(m)}` : uk.value ? `у${NBSP}${MONTH_IN_UK[m - 1]}` : `в${NBSP}${MONTH_IN[m - 1]}`)
  const timeOf = (iso: string) => new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
  const hour = (h: number) => `${String(h).padStart(2, '0')}:00`
  /** Минуты одним куском времени: «8 дней и 16 часов» */
  const nonstop = (minutes: number) => {
    const d = Math.floor(minutes / 1440)
    const h = Math.floor((minutes % 1440) / 60)
    const and = en.value ? ' and ' : uk.value ? ' і ' : ' и '
    if (d >= 1) return h ? `${count('days', d)}${and}${count('hours', h)}` : count('days', d)
    return count('hours', Math.max(1, Math.round(minutes / 60)))
  }
  const personaTitle = (r: Rewind) => t(`persona.${r.personality.id}.title`)
  const personaText = (r: Rewind) => {
    const p = r.personality
    const share = { night_owl: p.nightShare, early_bird: p.morningShare, explorer: p.noveltyShare, loyal: p.repeatShare, wave_rider: p.waveShare }[p.id] ?? 0
    return t(`persona.${p.id}.text`, { share: pct(share) })
  }
  const peakMonth = (r: Rewind) => r.months.reduce((a, b) => (b.minutes > a.minutes ? b : a), r.months[0]!)
  /** Имя для обращения: первое слово имени аккаунта */
  const auth = useAuthStore()
  const firstName = computed(() => auth.user?.name?.trim().split(/\s+/)[0] ?? '')
  const intro = (r: Rewind) => (!r.complete ? t('introSoFar') : firstName.value ? t('intro', { name: firstName.value }) : t('introAnon'))

  return { t, lang, en, img, cover, artistsOf, num, pct, word, count, day, dayShort, monthName, monthLetter, monthIn, timeOf, hour, nonstop, personaTitle, personaText, peakMonth, firstName, intro }
}

export type RewindText = ReturnType<typeof useRewindText>
