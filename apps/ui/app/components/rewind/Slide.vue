<script setup lang="ts">
import type { ImageSet, Rewind, Track } from '~/api/types'
import type { SlideColors } from '~/utils/rewind-colors'

// Один слайд «Итогов года». Поле — плоский цвет обложки, крупные цифры и простые фигуры.
// Размеры — в долях слайда (cqw/cqh): одинаково в рамке телефона на компьютере и на весь экран телефона
export type SlideKind = 'intro' | 'minutes' | 'days' | 'first' | 'tracks' | 'toy' | 'artists' | 'albums' | 'sound' | 'discoveries' | 'personality' | 'summary' | 'empty'

const props = defineProps<{
  kind: SlideKind
  r: Rewind
  colors: SlideColors
  active: boolean
  /** Плейлист сохраняется или картинка готовится */
  busy?: 'share' | 'playlist' | null
}>()
const emit = defineEmits<{ share: [], playlist: [] }>()

const bridge = useBridge()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const img = (set: ImageSet | null | undefined, size: 'small' | 'medium' | 'large' = 'large') => (set?.[size] ? bridge.resolveUrl(set[size]) : null)
const cover = (tr: Track | null | undefined, size: 'small' | 'medium' | 'large' = 'large') => img(tr?.album?.image, size)
const artistsOf = (tr: Track) => tr.artists.filter(a => a.role !== 'featured').map(a => a.name).join(', ') || tr.artists.map(a => a.name).join(', ')
const num = (n: number) => formatNumber(n, lang.value)
const pct = (share: number) => `${Math.round(share * 100)}${NBSP}%`
const locale = computed(() => (lang.value === 'en' ? 'en-GB' : 'ru-RU'))
const day = (iso: string) => new Intl.DateTimeFormat(locale.value, { day: 'numeric', month: 'long', timeZone: 'UTC' }).format(new Date(`${iso.slice(0, 10)}T12:00:00Z`))
const dayShort = (iso: string) => new Intl.DateTimeFormat(locale.value, { day: 'numeric', month: 'short', timeZone: 'UTC' }).format(new Date(`${iso.slice(0, 10)}T12:00:00Z`))
const monthName = (m: number) => new Intl.DateTimeFormat(locale.value, { month: 'long', timeZone: 'UTC' }).format(new Date(Date.UTC(2026, m - 1, 15)))
const timeOf = (iso: string) => new Intl.DateTimeFormat(locale.value, { hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
const times = (n: number) => (lang.value === 'en' ? `${num(n)} ${n === 1 ? 'play' : 'plays'}` : `${num(n)}${NBSP}${plural(n, ['раз', 'раза', 'раз'])}`)
const ru = (n: number, forms: [string, string, string]) => plural(n, forms)

const style = computed(() => ({ '--bg': props.colors.bg, '--ink': props.colors.ink, '--soft': props.colors.soft, '--accent': props.colors.accent }))
const inProgress = computed(() => !props.r.complete)

// ---------- Минуты и месяцы ----------
const maxMonth = computed(() => Math.max(1, ...props.r.months.map(m => m.minutes)))
const peakMonth = computed(() => props.r.months.reduce((a, b) => (b.minutes > a.minutes ? b : a), props.r.months[0]!))
const minutesWord = computed(() => (lang.value === 'en' ? 'minutes of music' : `${ru(props.r.minutes, ['минута', 'минуты', 'минут'])} музыки`))
const minutesSub = computed(() => {
  const d = Math.floor(props.r.minutes / 1440)
  const h = Math.round(props.r.minutes / 60)
  if (lang.value === 'en') return d >= 1 ? t('nonstopDays', { n: d }) : t('nonstopHours', { n: h })
  return d >= 1 ? t('nonstopDays', { n: d, w: ru(d, ['сутки', 'суток', 'суток']) }) : t('nonstopHours', { n: h, w: ru(h, ['час', 'часа', 'часов']) })
})
const MONTH_IN = ['январе', 'феврале', 'марте', 'апреле', 'мае', 'июне', 'июле', 'августе', 'сентябре', 'октябре', 'ноябре', 'декабре']
const monthIn = (m: number) => (lang.value === 'en' ? `in ${monthName(m)}` : `в${NBSP}${MONTH_IN[m - 1]}`)

// ---------- Дни ----------
const daysWord = computed(() => (lang.value === 'en' ? (props.r.days === 1 ? 'day with music' : 'days with music') : `${ru(props.r.days, ['день', 'дня', 'дней'])} с музыкой`))
const streakWord = computed(() => {
  const n = props.r.streak?.days ?? 0
  return lang.value === 'en' ? (n === 1 ? 'day in a row' : 'days in a row') : `${ru(n, ['день', 'дня', 'дней'])} подряд`
})

// ---------- Топы ----------
const tracks = computed(() => props.r.topTracks)
const artists = computed(() => props.r.topArtists)
const albums = computed(() => props.r.topAlbums)
const toy = computed(() => props.r.trackOfYear)

// ---------- Звучание ----------
const maxGenre = computed(() => Math.max(0.01, ...props.r.genres.map(g => g.share)))
const genreShare = (s: number) => s / props.r.genres.reduce((a, g) => a + g.share, 0)
const moods = computed(() => props.r.moods.slice(0, 3))

// ---------- Открытия ----------
const newWord = computed(() => (lang.value === 'en' ? (props.r.newArtists === 1 ? 'new artist' : 'new artists') : `${ru(props.r.newArtists, ['новый артист', 'новых артиста', 'новых артистов'])}`))

// ---------- Характер ----------
const persona = computed(() => props.r.personality)
const traits = computed(() => [
  { id: 'night', share: persona.value.nightShare },
  { id: 'novelty', share: persona.value.noveltyShare },
  { id: 'repeat', share: persona.value.repeatShare },
  { id: 'wave', share: persona.value.waveShare },
])
const personaText = computed(() => {
  const p = persona.value
  const share = { night_owl: p.nightShare, early_bird: p.morningShare, explorer: p.noveltyShare, loyal: p.repeatShare, wave_rider: p.waveShare }[p.id] ?? 0
  return t(`persona.${p.id}.text`, { share: pct(share) })
})
</script>

<template>
  <section class="sl" :class="[`sl--${kind}`, { 'is-active': active }]" :style="style">
    <!-- Начало -->
    <template v-if="kind === 'intro'">
      <div class="intro">
        <div class="intro__covers" aria-hidden="true">
          <img v-for="(a, i) in albums.slice(0, 3)" :key="a.album.id" :src="img(a.album.image, 'medium') ?? ''" alt="" class="intro__cover in" :style="{ '--d': `${120 + i * 90}ms`, '--i': i }">
        </div>
        <div class="intro__text">
          <div class="kicker in" style="--d: 40ms">{{ inProgress ? t('soFar') : t('rewind') }}</div>
          <div class="intro__year tnum in" style="--d: 80ms">{{ r.year }}</div>
          <p class="lead in" style="--d: 360ms">{{ inProgress ? t('introSoFar') : t('intro') }}</p>
        </div>
      </div>
    </template>

    <!-- Минуты и кривая по месяцам -->
    <template v-else-if="kind === 'minutes'">
      <div class="kicker in" style="--d: 0ms">{{ t('youListened') }}</div>
      <div class="num tnum in" style="--d: 80ms">{{ num(r.minutes) }}</div>
      <div class="title in" style="--d: 160ms">{{ minutesWord }}</div>
      <p class="lead in" style="--d: 240ms">{{ minutesSub }}</p>
      <div class="months" role="img" :aria-label="t('monthsLabel')">
        <div v-for="m in r.months" :key="m.month" class="months__col">
          <span class="months__bar" :class="{ 'is-peak': m.month === peakMonth.month && m.minutes > 0 }" :style="{ '--h': `${Math.max(3, (m.minutes / maxMonth) * 100)}%`, '--d': `${300 + m.month * 40}ms` }" />
          <span class="months__label">{{ monthName(m.month).charAt(0).toUpperCase() }}</span>
        </div>
      </div>
      <p v-if="peakMonth.minutes > 0" class="foot in" style="--d: 900ms">{{ t('peakMonth', { month: monthIn(peakMonth.month), n: num(peakMonth.minutes) }) }}</p>
    </template>

    <!-- Дни, серия, самый музыкальный день -->
    <template v-else-if="kind === 'days'">
      <div class="kicker in" style="--d: 0ms">{{ t('rhythm') }}</div>
      <div class="num tnum in" style="--d: 80ms">{{ num(r.days) }}</div>
      <div class="title in" style="--d: 160ms">{{ daysWord }}</div>
      <p class="lead in" style="--d: 220ms">{{ t('ofDays', { n: num(r.daysInRange) }) }}</p>
      <div class="facts">
        <div v-if="r.streak && r.streak.days > 1" class="fact in" style="--d: 360ms">
          <div class="fact__big tnum">{{ num(r.streak.days) }}</div>
          <div class="fact__body">
            <div class="fact__title">{{ streakWord }}</div>
            <div class="fact__sub">{{ dayShort(r.streak.from) }} — {{ dayShort(r.streak.to) }}</div>
          </div>
        </div>
        <div v-if="r.topDay" class="fact in" style="--d: 480ms">
          <img v-if="cover(r.topDay.track, 'small')" :src="cover(r.topDay.track, 'small')!" alt="" class="fact__cover">
          <div class="fact__body">
            <div class="fact__title">{{ t('topDay', { day: day(r.topDay.date) }) }}</div>
            <div class="fact__sub">
              {{ t('topDayStats', { min: num(r.topDay.minutes), plays: formatCount(r.topDay.plays, 'track', lang) }) }}<template v-if="r.topDay.track && r.topDay.trackPlays > 1"> · {{ t('topDayTrack', { title: r.topDay.track.title, n: times(r.topDay.trackPlays) }) }}</template>
            </div>
          </div>
        </div>
      </div>
    </template>

    <!-- Первый трек года -->
    <template v-else-if="kind === 'first' && r.firstTrack">
      <div class="kicker in" style="--d: 0ms">{{ t('firstKicker', { year: r.year }) }}</div>
      <div class="hero">
        <div class="hero__shape in" style="--d: 60ms" aria-hidden="true" />
        <img v-if="cover(r.firstTrack.track)" :src="cover(r.firstTrack.track)!" alt="" class="hero__cover in" style="--d: 140ms">
      </div>
      <div class="name in" style="--d: 260ms">{{ r.firstTrack.track.title }}</div>
      <div class="by in" style="--d: 320ms">{{ artistsOf(r.firstTrack.track) }}</div>
      <p class="foot in" style="--d: 420ms">{{ t('firstAt', { day: day(r.firstTrack.at), time: timeOf(r.firstTrack.at) }) }}</p>
    </template>

    <!-- Топ-5 треков -->
    <template v-else-if="kind === 'tracks'">
      <div class="kicker in" style="--d: 0ms">{{ t('topTracks') }}</div>
      <ol class="rank">
        <li v-for="(x, i) in tracks" :key="x.track.id" class="rank__row in" :class="{ 'rank__row--top': i === 0 }" :style="{ '--d': `${100 + i * 90}ms` }">
          <span class="rank__n tnum">{{ i + 1 }}</span>
          <img v-if="cover(x.track, i === 0 ? 'medium' : 'small')" :src="cover(x.track, i === 0 ? 'medium' : 'small')!" alt="" class="rank__img">
          <span class="rank__body">
            <span class="rank__title ellipsis">{{ x.track.title }}</span>
            <span class="rank__sub ellipsis">{{ artistsOf(x.track) }}</span>
          </span>
          <span class="rank__plays tnum">{{ times(x.plays) }}</span>
        </li>
      </ol>
    </template>

    <!-- Трек года -->
    <template v-else-if="kind === 'toy' && toy">
      <div class="kicker in" style="--d: 0ms">{{ t('toyKicker') }}</div>
      <div class="hero hero--big">
        <div class="hero__shape hero__shape--square in" style="--d: 60ms" aria-hidden="true" />
        <img v-if="cover(toy.track)" :src="cover(toy.track)!" alt="" class="hero__cover in" style="--d: 140ms">
      </div>
      <div class="name name--big in" style="--d: 260ms">{{ toy.track.title }}</div>
      <div class="by in" style="--d: 320ms">{{ artistsOf(toy.track) }}</div>
      <div class="chips in" style="--d: 420ms">
        <span class="chip tnum">{{ times(toy.plays) }}</span>
        <span class="chip">{{ t('since', { day: day(toy.firstPlayed) }) }}</span>
        <span class="chip">{{ t('peak', { month: monthName(toy.peakMonth) }) }}</span>
      </div>
    </template>

    <!-- Артисты -->
    <template v-else-if="kind === 'artists' && artists[0]">
      <div class="kicker in" style="--d: 0ms">{{ t('topArtist') }}</div>
      <div class="hero">
        <div class="hero__shape in" style="--d: 60ms" aria-hidden="true" />
        <img v-if="img(artists[0].artist.image)" :src="img(artists[0].artist.image)!" alt="" class="hero__cover hero__cover--round in" style="--d: 140ms">
      </div>
      <div class="name name--big in" style="--d: 240ms">{{ artists[0].artist.name }}</div>
      <div class="by tnum in" style="--d: 300ms">{{ times(artists[0].plays) }} · {{ formatLongDuration(artists[0].minutes * 60000, lang) }}</div>
      <ol class="mini">
        <li v-for="(a, i) in artists.slice(1)" :key="a.artist.id" class="mini__row in" :style="{ '--d': `${400 + i * 70}ms` }">
          <span class="mini__n tnum">{{ i + 2 }}</span>
          <img v-if="img(a.artist.image, 'small')" :src="img(a.artist.image, 'small')!" alt="" class="mini__img mini__img--round">
          <span class="mini__name ellipsis">{{ a.artist.name }}</span>
          <span class="mini__plays tnum">{{ num(a.plays) }}</span>
        </li>
      </ol>
    </template>

    <!-- Альбомы -->
    <template v-else-if="kind === 'albums' && albums[0]">
      <div class="kicker in" style="--d: 0ms">{{ t('topAlbums') }}</div>
      <div class="alb">
        <img v-if="img(albums[0].album.image)" :src="img(albums[0].album.image)!" alt="" class="alb__first in" style="--d: 100ms">
        <div class="alb__meta in" style="--d: 220ms">
          <div class="name">{{ albums[0].album.title }}</div>
          <div class="by tnum">{{ albums[0].album.artist.name }} · {{ times(albums[0].plays) }}</div>
        </div>
        <div class="alb__rest">
          <div v-for="(a, i) in albums.slice(1)" :key="a.album.id" class="alb__item in" :style="{ '--d': `${320 + i * 70}ms` }">
            <img v-if="img(a.album.image, 'medium')" :src="img(a.album.image, 'medium')!" alt="" class="alb__img">
            <span class="alb__n tnum">{{ i + 2 }}</span>
          </div>
        </div>
      </div>
    </template>

    <!-- Звучание: жанры и настроение -->
    <template v-else-if="kind === 'sound'">
      <div class="kicker in" style="--d: 0ms">{{ t('soundKicker') }}</div>
      <template v-if="r.genres.length">
        <div class="title title--lead in" style="--d: 80ms">{{ r.genres[0]!.name }}</div>
        <ul class="bars">
          <li v-for="(g, i) in r.genres" :key="g.id" class="bars__row in" :style="{ '--d': `${180 + i * 80}ms` }">
            <span class="bars__head">
              <span class="bars__name ellipsis">{{ g.name }}</span>
              <span class="bars__pct tnum">{{ pct(genreShare(g.share)) }}</span>
            </span>
            <span class="bars__track"><i :style="{ '--w': `${(g.share / maxGenre) * 100}%`, '--d': `${260 + i * 80}ms` }" /></span>
          </li>
        </ul>
        <div v-if="moods.length" class="moods in" style="--d: 640ms">
          <div class="moods__label">{{ t('moodLabel') }}</div>
          <div class="chips">
            <span v-for="m in moods" :key="m.id" class="chip">{{ t(`mood.${m.id}`) }} <b class="tnum">{{ pct(m.score) }}</b></span>
          </div>
        </div>
      </template>
      <div v-else class="thin in" style="--d: 80ms">
        <div class="title">{{ t('soundEmpty') }}</div>
        <p class="lead">{{ t('soundEmptyText') }}</p>
      </div>
    </template>

    <!-- Открытия -->
    <template v-else-if="kind === 'discoveries'">
      <div class="kicker in" style="--d: 0ms">{{ t('discKicker') }}</div>
      <template v-if="r.newArtists > 0">
        <div class="num tnum in" style="--d: 80ms">{{ num(r.newArtists) }}</div>
        <div class="title in" style="--d: 160ms">{{ newWord }}</div>
        <ol v-if="r.discoveries.length" class="mini mini--disc">
          <li v-for="(d, i) in r.discoveries" :key="d.artist.id" class="mini__row in" :style="{ '--d': `${280 + i * 80}ms` }">
            <img v-if="img(d.artist.image, 'small')" :src="img(d.artist.image, 'small')!" alt="" class="mini__img mini__img--round">
            <span class="mini__body">
              <span class="mini__name ellipsis">{{ d.artist.name }}</span>
              <span class="mini__sub">{{ t('discSince', { day: dayShort(d.firstHeard) }) }} · {{ times(d.plays) }}</span>
            </span>
          </li>
        </ol>
        <p v-else class="lead in" style="--d: 260ms">{{ t('discNoFav') }}</p>
      </template>
      <div v-else class="thin in" style="--d: 80ms">
        <div class="title title--lead">{{ t('discEmpty') }}</div>
        <p class="lead">{{ t('discEmptyText') }}</p>
      </div>
    </template>

    <!-- Характер -->
    <template v-else-if="kind === 'personality'">
      <div class="kicker in" style="--d: 0ms">{{ t('personaKicker') }}</div>
      <div class="persona">
        <div class="persona__shape in" :class="`persona__shape--${persona.id}`" style="--d: 60ms" aria-hidden="true" />
        <div class="title title--persona in" style="--d: 180ms">{{ t(`persona.${persona.id}.title`) }}</div>
        <p class="lead in" style="--d: 280ms">{{ personaText }}</p>
      </div>
      <ul class="traits">
        <li v-for="(x, i) in traits" :key="x.id" class="traits__row in" :style="{ '--d': `${380 + i * 70}ms` }">
          <span class="traits__name">{{ t(`trait.${x.id}`) }}</span>
          <span class="traits__track"><i :style="{ '--w': `${Math.max(2, x.share * 100)}%`, '--d': `${460 + i * 70}ms` }" /></span>
          <span class="traits__pct tnum">{{ pct(x.share) }}</span>
        </li>
      </ul>
    </template>

    <!-- Итог: им же можно поделиться картинкой -->
    <template v-else-if="kind === 'summary'">
      <div class="sum">
        <div class="sum__head in" style="--d: 0ms">
          <img v-if="img(artists[0]?.artist.image, 'medium')" :src="img(artists[0]?.artist.image, 'medium')!" alt="" class="sum__photo">
          <div>
            <div class="kicker">{{ inProgress ? t('soFar') : t('rewind') }}</div>
            <div class="sum__year tnum">{{ r.year }}</div>
          </div>
        </div>
        <div class="sum__cols in" style="--d: 120ms">
          <div>
            <div class="sum__label">{{ t('sumArtists') }}</div>
            <ol class="sum__list">
              <li v-for="(a, i) in artists" :key="a.artist.id"><span class="tnum">{{ i + 1 }}</span><span class="ellipsis">{{ a.artist.name }}</span></li>
            </ol>
          </div>
          <div>
            <div class="sum__label">{{ t('sumTracks') }}</div>
            <ol class="sum__list">
              <li v-for="(x, i) in tracks" :key="x.track.id"><span class="tnum">{{ i + 1 }}</span><span class="ellipsis">{{ x.track.title }}</span></li>
            </ol>
          </div>
        </div>
        <div class="sum__stats in" style="--d: 220ms">
          <div><div class="sum__label">{{ t('sumMinutes') }}</div><div class="sum__value tnum">{{ num(r.minutes) }}</div></div>
          <div v-if="r.genres[0]"><div class="sum__label">{{ t('sumGenre') }}</div><div class="sum__value ellipsis">{{ r.genres[0].name }}</div></div>
        </div>
        <div class="sum__persona in" style="--d: 300ms">
          <div class="sum__label">{{ t('personaKicker') }}</div>
          <div class="sum__value">{{ t(`persona.${persona.id}.title`) }}</div>
        </div>
        <div class="sum__actions in" style="--d: 380ms" @pointerdown.stop @pointerup.stop @click.stop>
          <button type="button" class="act act--main" :disabled="busy === 'share'" @click="emit('share')">
            <Icon :name="busy === 'share' ? 'ph:circle-notch' : 'ph:upload-simple'" :class="{ spin: busy === 'share' }" />{{ t('share') }}
          </button>
          <button type="button" class="act" :disabled="busy === 'playlist'" @click="emit('playlist')">
            <Icon :name="busy === 'playlist' ? 'ph:circle-notch' : r.playlistId ? 'ph:playlist' : 'ph:plus'" :class="{ spin: busy === 'playlist' }" />{{ r.playlistId ? t('openPlaylist') : t('savePlaylist') }}
          </button>
        </div>
      </div>
    </template>

    <!-- Год без прослушиваний -->
    <template v-else>
      <div class="thin thin--center">
        <div class="intro__year tnum in" style="--d: 0ms">{{ r.year }}</div>
        <div class="title in" style="--d: 120ms">{{ t('emptyTitle') }}</div>
        <p class="lead in" style="--d: 200ms">{{ t('emptyText', { year: r.year }) }}</p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.sl {
  position: absolute;
  inset: 0;
  container-type: size;
  display: flex;
  flex-direction: column;
  padding: calc(var(--rw-top, 0px) + 13cqw) 7cqw calc(var(--rw-bottom, 0px) + 8cqw);
  overflow: hidden;
  background: var(--bg);
  color: var(--ink);
  font-family: var(--font);
}

/* Появление: снизу вверх, по очереди. Без движения — сразу на месте */
.in { opacity: 0; }
.is-active .in { animation: rise 640ms var(--ease-out) both; animation-delay: var(--d, 0ms); }
@keyframes rise {
  from { opacity: 0; transform: translateY(4cqh); }
  to { opacity: 1; transform: none; }
}
@media (prefers-reduced-motion: reduce) {
  .in, .is-active .in { opacity: 1; animation: none; }
}

.kicker { font-size: 4.1cqw; font-weight: 700; letter-spacing: -0.01em; color: var(--soft); }
.num { margin-top: 4cqh; font-size: 30cqw; font-weight: 850; line-height: 0.86; letter-spacing: -0.055em; }
.title { margin-top: 1.4cqh; font-size: 8.6cqw; font-weight: 800; line-height: 1.02; letter-spacing: -0.035em; text-wrap: balance; }
.title--lead { margin-top: 3cqh; font-size: 12cqw; letter-spacing: -0.045em; }
.lead { margin-top: 2cqh; max-width: 82cqw; font-size: 4.6cqw; font-weight: 500; line-height: 1.3; color: var(--soft); text-wrap: pretty; }
.foot { margin-top: auto; font-size: 4.2cqw; font-weight: 600; line-height: 1.3; color: var(--soft); }
.name { margin-top: 3.4cqh; font-size: 7.4cqw; font-weight: 800; line-height: 1.05; letter-spacing: -0.03em; text-wrap: balance; overflow-wrap: anywhere; }
.name--big { font-size: 9.6cqw; letter-spacing: -0.04em; }
.by { margin-top: 0.8cqh; font-size: 4.6cqw; font-weight: 600; color: var(--soft); }
.ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
.chips { display: flex; flex-wrap: wrap; gap: 2cqw; margin-top: 3cqh; }
.chip { display: inline-flex; align-items: baseline; gap: 1.4cqw; padding: 1.8cqw 3.4cqw; border-radius: 999px; background: var(--ink); color: var(--bg); font-size: 3.8cqw; font-weight: 700; }
.chip b { font-weight: 800; }

/* Начало */
.intro { display: flex; flex: 1; flex-direction: column; justify-content: space-between; }
.intro__covers { position: relative; height: 62cqw; margin: auto 0; }
.intro__cover {
  position: absolute;
  top: 0;
  top: calc(var(--i) * 4cqw);
  left: calc(var(--i) * 18cqw);
  width: 50cqw;
  aspect-ratio: 1;
  object-fit: cover;
  border-radius: 2cqw;
  transform: rotate(calc((var(--i) - 1) * 5deg));
}
.is-active .intro__cover { animation-name: drop; }
@keyframes drop {
  from { opacity: 0; transform: translateY(-6cqh) rotate(calc((var(--i) - 1) * 5deg)); }
  to { opacity: 1; transform: rotate(calc((var(--i) - 1) * 5deg)); }
}
.intro__year { font-size: 36cqw; font-weight: 850; line-height: 0.84; letter-spacing: -0.06em; margin-top: 1cqh; }

/* Месяцы */
.months { display: grid; grid-template-columns: repeat(12, 1fr); gap: 1.4cqw; height: 26cqh; margin-top: auto; padding-top: 4cqh; }
.months__col { display: flex; flex-direction: column; align-items: center; gap: 1.2cqh; min-height: 0; }
.months__bar { flex: 1; position: relative; width: 100%; }
.months__bar::after {
  content: '';
  position: absolute;
  inset: auto 0 0;
  height: var(--h);
  border-radius: 1.4cqw;
  background: var(--accent);
  transform-origin: bottom;
}
.months__bar.is-peak::after { background: var(--ink); }
.is-active .months__bar::after { animation: grow 720ms var(--ease-out) both; animation-delay: var(--d); }
@keyframes grow { from { transform: scaleY(0); } to { transform: none; } }
.months__label { font-size: 3.2cqw; font-weight: 700; color: var(--soft); }
.sl--minutes .foot { margin-top: 2.4cqh; }

/* Факты: серия и самый музыкальный день */
.facts { display: flex; flex-direction: column; gap: 3cqh; margin-top: auto; }
.fact { display: flex; align-items: center; gap: 4cqw; padding-top: 3cqh; border-top: 2px solid color-mix(in srgb, var(--ink) 22%, transparent); }
.fact__big { min-width: 22cqw; font-size: 15cqw; font-weight: 850; line-height: 0.9; letter-spacing: -0.05em; color: var(--accent); }
.fact__cover { width: 22cqw; aspect-ratio: 1; object-fit: cover; border-radius: 1.6cqw; flex: none; }
.fact__body { min-width: 0; }
.fact__title { font-size: 5.2cqw; font-weight: 800; line-height: 1.12; letter-spacing: -0.02em; }
.fact__sub { margin-top: 0.6cqh; font-size: 3.9cqw; font-weight: 600; line-height: 1.3; color: var(--soft); }

/* Большая обложка с фигурой цвета обложки за ней */
.hero { position: relative; width: 66cqw; aspect-ratio: 1; margin: 6cqh auto 0; }
.hero--big { width: 76cqw; margin-top: 4cqh; }
.hero__shape { position: absolute; inset: -7cqw -9cqw auto auto; width: 50cqw; aspect-ratio: 1; border-radius: 50%; background: var(--accent); }
.hero__shape--square { inset: auto auto -6cqw -8cqw; border-radius: 4cqw; transform: rotate(-8deg); }
.hero__cover { position: relative; width: 100%; height: 100%; object-fit: cover; border-radius: 2.4cqw; }
.hero__cover--round { border-radius: 50%; }
.is-active .hero__shape { animation: pop 820ms var(--ease-spring) both; animation-delay: var(--d); }
@keyframes pop { from { opacity: 0; transform: scale(0.4); } to { opacity: 1; } }
.sl--toy .name, .sl--first .name, .sl--artists .name { text-align: left; }

/* Топ треков */
.rank { display: flex; flex-direction: column; gap: 2.6cqh; margin-top: 5cqh; list-style: none; }
.rank__row { display: grid; grid-template-columns: 6cqw 15cqw 1fr auto; align-items: center; gap: 3.4cqw; }
.rank__row--top { grid-template-columns: 6cqw 30cqw 1fr; margin-bottom: 1.6cqh; }
.rank__row--top .rank__plays { grid-column: 3; grid-row: 2; margin-top: -1cqh; justify-self: start; }
.rank__row--top .rank__body { align-self: end; }
.rank__n { font-size: 5cqw; font-weight: 800; color: var(--soft); }
.rank__row--top .rank__n { font-size: 9cqw; color: var(--ink); align-self: start; }
.rank__img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 1.4cqw; }
.rank__row--top .rank__img { grid-row: span 2; }
.rank__body { display: flex; flex-direction: column; min-width: 0; }
.rank__title { font-size: 4.6cqw; font-weight: 800; letter-spacing: -0.015em; }
.rank__row--top .rank__title { font-size: 7cqw; line-height: 1.05; white-space: normal; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
.rank__sub { margin-top: 0.3cqh; font-size: 3.7cqw; font-weight: 600; color: var(--soft); }
.rank__plays { font-size: 3.8cqw; font-weight: 700; color: var(--soft); }
.rank__row--top .rank__plays { padding: 1.2cqw 3cqw; border-radius: 999px; background: var(--ink); color: var(--bg); }

/* Короткие списки: артисты 2–5, открытия */
.mini { display: flex; flex-direction: column; gap: 1.8cqh; margin-top: auto; list-style: none; }
.mini--disc { gap: 2.4cqh; margin-top: 4cqh; }
.mini__row { display: flex; align-items: center; gap: 3.4cqw; min-width: 0; }
.mini__n { width: 5cqw; font-size: 4.2cqw; font-weight: 800; color: var(--soft); }
.mini__img { width: 11cqw; aspect-ratio: 1; object-fit: cover; flex: none; border-radius: 1.2cqw; }
.mini--disc .mini__img { width: 14cqw; }
.mini__img--round { border-radius: 50%; }
.mini__body { display: flex; flex-direction: column; min-width: 0; }
.mini__name { flex: 1; font-size: 4.6cqw; font-weight: 800; }
.mini__sub { margin-top: 0.3cqh; font-size: 3.6cqw; font-weight: 600; color: var(--soft); }
.mini__plays { font-size: 3.8cqw; font-weight: 700; color: var(--soft); }
.sl--artists .hero { width: 56cqw; margin-top: 4cqh; }

/* Альбомы */
.alb { display: flex; flex: 1; flex-direction: column; margin-top: 4cqh; }
.alb__first { width: 72cqw; aspect-ratio: 1; object-fit: cover; border-radius: 2.4cqw; }
.alb__meta .name { margin-top: 2.4cqh; }
.alb__rest { display: grid; grid-template-columns: repeat(4, 1fr); gap: 2.4cqw; margin-top: auto; }
.alb__item { position: relative; }
.alb__img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 1.4cqw; display: block; }
.alb__n { position: absolute; left: 1.4cqw; bottom: 1cqw; padding: 0.4cqw 1.6cqw; border-radius: 999px; background: var(--bg); color: var(--ink); font-size: 3.2cqw; font-weight: 800; }

/* Жанры: полосы */
.bars { display: flex; flex-direction: column; gap: 2.2cqh; margin-top: 4cqh; list-style: none; }
.bars__head { display: flex; justify-content: space-between; gap: 3cqw; font-size: 4.4cqw; font-weight: 700; }
.bars__pct { color: var(--soft); }
.bars__track { display: block; height: 2.6cqw; margin-top: 1cqh; border-radius: 999px; background: color-mix(in srgb, var(--ink) 14%, transparent); overflow: hidden; }
.bars__track i, .traits__track i { display: block; width: var(--w); height: 100%; border-radius: inherit; background: var(--accent); transform-origin: left; }
.bars__row:first-child .bars__track i { background: var(--ink); }
.is-active .bars__track i, .is-active .traits__track i { animation: widen 760ms var(--ease-out) both; animation-delay: var(--d); }
@keyframes widen { from { transform: scaleX(0); } to { transform: none; } }
.moods { margin-top: auto; }
.moods__label { font-size: 3.8cqw; font-weight: 700; color: var(--soft); }
.moods .chips { margin-top: 1.4cqh; }

/* Мало данных — честно и спокойно */
.thin { margin-top: auto; margin-bottom: auto; }
.thin--center { display: flex; flex: 1; flex-direction: column; justify-content: center; }

/* Характер: фигура под характер */
.persona { position: relative; margin-top: auto; }
.persona__shape { width: 46cqw; aspect-ratio: 1; margin-bottom: 4cqh; border-radius: 50%; background: var(--accent); }
.persona__shape--night_owl { background: var(--ink); -webkit-mask: radial-gradient(circle at 84% 38%, transparent 49.5%, #000 50.5%); mask: radial-gradient(circle at 84% 38%, transparent 49.5%, #000 50.5%); }
.persona__shape--early_bird { border-radius: 50% 50% 0 0; aspect-ratio: 2 / 1; height: auto; }
.persona__shape--explorer { border-radius: 0; transform: rotate(45deg) scale(0.78); }
.persona__shape--loyal { border-radius: 50%; box-shadow: 0 0 0 4cqw var(--bg), 0 0 0 7cqw var(--accent); width: 24cqw; margin: 6cqw 0 calc(4cqh + 6cqw) 6cqw; }
.persona__shape--wave_rider { aspect-ratio: 3 / 1; border-radius: 999px; }
.title--persona { font-size: 12cqw; letter-spacing: -0.045em; line-height: 0.98; }
.traits { display: flex; flex-direction: column; gap: 1.8cqh; margin-top: 5cqh; list-style: none; }
.traits__row { display: grid; grid-template-columns: 24cqw 1fr 13cqw; align-items: center; gap: 3cqw; font-size: 3.9cqw; font-weight: 700; }
.traits__track { display: block; height: 2.2cqw; border-radius: 999px; background: color-mix(in srgb, var(--ink) 14%, transparent); overflow: hidden; }
.traits__pct { text-align: right; color: var(--soft); }

/* Итог */
.sum { display: flex; flex: 1; flex-direction: column; gap: 4.2cqh; }
.sum__head { display: flex; align-items: flex-end; gap: 5cqw; }
.sum__photo { width: 34cqw; aspect-ratio: 1; object-fit: cover; border-radius: 2cqw; }
.sum__year { font-size: 18cqw; font-weight: 850; line-height: 0.86; letter-spacing: -0.055em; }
.sum__cols { display: grid; grid-template-columns: 1fr 1fr; gap: 5cqw; }
.sum__label { font-size: 3.5cqw; font-weight: 700; color: var(--soft); }
.sum__list { display: flex; flex-direction: column; gap: 0.9cqh; margin-top: 1.2cqh; list-style: none; }
.sum__list li { display: flex; gap: 2cqw; min-width: 0; font-size: 4.6cqw; font-weight: 800; letter-spacing: -0.01em; }
.sum__list li span:first-child { width: 3.6cqw; flex: none; color: var(--soft); }
.sum__stats { display: grid; grid-template-columns: 1fr 1fr; gap: 5cqw; }
.sum__value { margin-top: 0.6cqh; font-size: 7cqw; font-weight: 850; letter-spacing: -0.035em; line-height: 1.05; }
.sum__actions { display: flex; gap: 2.6cqw; margin-top: auto; }
.act {
  display: inline-flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 2cqw;
  min-height: 44px;
  padding: 3.4cqw 3cqw;
  border-radius: 999px;
  border: 2px solid var(--ink);
  background: transparent;
  color: var(--ink);
  font: inherit;
  font-size: 4cqw;
  font-weight: 800;
  cursor: pointer;
  transition: transform var(--dur-1) var(--ease-out), opacity var(--dur-1);
}
.act--main { background: var(--ink); color: var(--bg); }
.act:active { transform: scale(0.97); }
.act:disabled { opacity: 0.6; cursor: default; }
.act:focus-visible { outline: 3px solid var(--accent); outline-offset: 2px; }
.spin { animation: spin 0.9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
</style>

<i18n lang="json">
{
  "ru": {
    "rewind": "Итоги года",
    "soFar": "Итоги по сей день",
    "intro": "Твой год в музыке",
    "introSoFar": "Год ещё идёт — вот что уже набралось",
    "youListened": "Музыка за год",
    "nonstopDays": "Это {n} {w} без перерыва",
    "nonstopHours": "Это {n} {w} без перерыва",
    "monthsLabel": "Минуты по месяцам",
    "peakMonth": "Больше всего — {month}: {n} мин",
    "rhythm": "Ритм года",
    "ofDays": "из {n} дней",
    "topDay": "Самый музыкальный день — {day}",
    "topDayStats": "{min} мин · {plays}",
    "topDayTrack": "«{title}» — {n}",
    "firstKicker": "Твой {year} начался с",
    "firstAt": "{day} в {time}",
    "topTracks": "Треки года",
    "toyKicker": "Твой трек года",
    "since": "с {day}",
    "peak": "пик — {month}",
    "topArtist": "Артист года",
    "topAlbums": "Альбомы года",
    "soundKicker": "Звучание года",
    "moodLabel": "Настроение",
    "mood": { "happy": "радость", "sad": "грусть", "aggressive": "драйв", "relaxed": "спокойствие", "party": "праздник" },
    "soundEmpty": "Звучание ещё разбирается",
    "soundEmptyText": "Когда треки года пройдут анализ звука, здесь появятся жанры и настроение",
    "discKicker": "Открытия года",
    "discSince": "с {day}",
    "discNoFav": "Новые имена были — но любимыми пока не стали",
    "discEmpty": "Без новых имён",
    "discEmptyText": "Весь год — с теми, кого уже любишь. Новое найдётся в «Открытиях недели»",
    "personaKicker": "Твой характер",
    "persona": {
      "night_owl": { "title": "Ночной слушатель", "text": "Музыка звучит, когда город спит: {share} времени — после 23:00" },
      "early_bird": { "title": "Ранняя пташка", "text": "Утро начинается с музыки: {share} времени — до полудня" },
      "explorer": { "title": "Исследователь", "text": "{share} треков года — новые для тебя" },
      "loyal": { "title": "Верный слушатель", "text": "Ты знаешь, что любишь: {share} прослушиваний — любимое снова" },
      "wave_rider": { "title": "На волне", "text": "{share} времени звучала «Моя волна»" },
      "balanced": { "title": "Меломан", "text": "Утро и ночь, новое и любимое — всего понемногу" }
    },
    "trait": { "night": "Ночью", "novelty": "Новое", "repeat": "Повторы", "wave": "Волна" },
    "sumArtists": "Артисты",
    "sumTracks": "Треки",
    "sumMinutes": "Минут",
    "sumGenre": "Жанр",
    "share": "Поделиться",
    "savePlaylist": "Плейлист",
    "openPlaylist": "Открыть плейлист",
    "emptyTitle": "Пока тихо",
    "emptyText": "Включи что-нибудь — итоги {year} соберутся из прослушиваний"
  },
  "en": {
    "rewind": "Year in review",
    "soFar": "Your year so far",
    "intro": "Your year in music",
    "introSoFar": "The year isn’t over — here’s what you have so far",
    "youListened": "Your year in music",
    "nonstopDays": "That’s {n} days non-stop",
    "nonstopHours": "That’s {n} hours non-stop",
    "monthsLabel": "Minutes by month",
    "peakMonth": "Most of all {month}: {n} min",
    "rhythm": "Your rhythm",
    "ofDays": "out of {n} days",
    "topDay": "Your biggest day — {day}",
    "topDayStats": "{min} min · {plays}",
    "topDayTrack": "“{title}” — {n}",
    "firstKicker": "Your {year} began with",
    "firstAt": "{day} at {time}",
    "topTracks": "Top tracks",
    "toyKicker": "Your track of the year",
    "since": "since {day}",
    "peak": "peak — {month}",
    "topArtist": "Artist of the year",
    "topAlbums": "Top albums",
    "soundKicker": "Your sound",
    "moodLabel": "Mood",
    "mood": { "happy": "happy", "sad": "sad", "aggressive": "drive", "relaxed": "calm", "party": "party" },
    "soundEmpty": "Still listening closely",
    "soundEmptyText": "Genres and mood appear once this year’s tracks are analysed",
    "discKicker": "Discoveries",
    "discSince": "since {day}",
    "discNoFav": "New names came along — none became a favourite yet",
    "discEmpty": "No new names",
    "discEmptyText": "A whole year with the artists you love. Discover Weekly has new ones waiting",
    "personaKicker": "Your listening type",
    "persona": {
      "night_owl": { "title": "Night owl", "text": "Music plays while the city sleeps: {share} of your time after 11 pm" },
      "early_bird": { "title": "Early bird", "text": "Mornings start with music: {share} of your time before noon" },
      "explorer": { "title": "Explorer", "text": "{share} of this year’s tracks were new to you" },
      "loyal": { "title": "Loyal listener", "text": "You know what you love: {share} of plays were favourites again" },
      "wave_rider": { "title": "Wave rider", "text": "{share} of your time went to My Wave" },
      "balanced": { "title": "Music lover", "text": "Mornings and nights, new and familiar — a bit of everything" }
    },
    "trait": { "night": "Night", "novelty": "New", "repeat": "Repeats", "wave": "Wave" },
    "sumArtists": "Artists",
    "sumTracks": "Tracks",
    "sumMinutes": "Minutes",
    "sumGenre": "Genre",
    "share": "Share",
    "savePlaylist": "Playlist",
    "openPlaylist": "Open playlist",
    "emptyTitle": "Quiet so far",
    "emptyText": "Play something — your {year} review builds from what you listen to"
  }
}
</i18n>
