<script setup lang="ts">
import type { ImageSet, Rewind, RewindMore } from '~/api/types'
import { renderRecordCard } from '~/lib/rewind/share-record'
import { recordColors } from '~/utils/rewind-colors'
import type { RewindSlide } from '~/utils/rewind-slides'

// «Пластинка»: год как вещи, которые можно взять в руки. Конверт с пластинкой, кассета со счётчиком
// плёнки, полароиды, пластинка трека года под иглой, значок характера и чек итогов из принтера.
// Фон — тёмный в тон обложки, с зерном бумаги. Переход — плёнка протягивается на кадр, по краям
// мелькает перфорация (design/rewind/c.js, c.css)
const props = defineProps<{
  slides: RewindSlide[]
  index: number
  dir: 1 | -1
  r: Rewind
  more: RewindMore | null
  busy: 'share' | 'playlist' | 'more' | null
  frozen: boolean
}>()
const emit = defineEmits<{ share: [], playlist: [], wave: [], playMore: [], saveMore: [] }>()

const x = useRewindText()
const { t } = x

const slide = computed(() => props.slides[props.index] ?? props.slides[0]!)
const kind = computed(() => slide.value.kind)
const vars = computed(() => {
  const c = recordColors(slide.value.palette)
  return { '--bg': c.bg, '--obj': c.obj, '--obj-deep': c.objDeep }
})

const tracks = computed(() => props.r.topTracks)
const artists = computed(() => props.r.topArtists)
const albums = computed(() => props.r.topAlbums)
const toy = computed(() => props.r.trackOfYear)
const year = computed(() => props.r.year)
const pic = (set: ImageSet | null | undefined, size: 'small' | 'medium' | 'large' = 'large') => x.img(set, size)
const NB = NBSP
const trackCount = (n: number) => formatCount(n, 'track', x.lang.value)

// ---------- Начало: конверт и пластинка ----------
const sleeveCover = computed(() => x.cover(toy.value?.track ?? tracks.value[0]?.track) ?? pic(albums.value[0]?.album.image))
const discLabel = computed(() => x.cover(tracks.value[2]?.track ?? tracks.value[1]?.track, 'medium') ?? pic(albums.value[1]?.album.image, 'medium') ?? sleeveCover.value)

// ---------- Минуты: кассета, счётчик, индикатор по месяцам ----------
const digits = computed(() => String(Math.max(0, props.r.minutes)).split('').map(Number))
const maxMonth = computed(() => Math.max(1, ...props.r.months.map(m => m.minutes)))
const peak = computed(() => x.peakMonth(props.r))
const LEDS = 9
const vu = computed(() => props.r.months.map((m) => {
  const lit = m.minutes > 0 ? Math.max(1, Math.round((m.minutes / maxMonth.value) * LEDS)) : 0
  return { month: m.month, lit, peak: m.month === peak.value.month && m.minutes > 0 }
}))

// ---------- Характер: значок ----------
const persona = computed(() => props.r.personality)
const ring = computed(() => {
  const title = x.personaTitle(props.r).toUpperCase()
  return `${title} · ${year.value} · ${title} · ${year.value} ·`
})

// ---------- Звучание: шкала приёмника ----------
const genres = computed(() => props.r.genres.slice(0, 5))
const genreTotal = computed(() => Math.max(0.0001, props.r.genres.reduce((a, g) => a + g.share, 0)))
const maxGenre = computed(() => Math.max(0.0001, ...genres.value.map(g => g.share)))
const SEGMENTS = 16
const genreLit = (share: number) => Math.max(1, Math.round((share / maxGenre.value) * SEGMENTS))
const moods = computed(() => props.r.moods.slice(0, 3))

// ---------- Ещё послушать ----------
const moreItems = computed(() => props.more?.items.slice(0, 4) ?? [])
const crate = computed(() => (props.more?.items ?? []).map(tr => x.cover(tr, 'medium')).filter((u): u is string => !!u).slice(0, 5))
const waveLabel = computed(() => x.cover(tracks.value[0]?.track, 'medium'))

// ---------- Чек ----------
/** Штрихкод чека — из года, всегда одинаковый */
const barcode = computed(() => {
  const seed = `dalywave-${year.value}`
  let pos = 0
  let h = 2166136261
  const bars: { x: number, w: number }[] = []
  while (pos < 200) {
    h = Math.imul(h ^ seed.charCodeAt(pos % seed.length), 16777619) >>> 0
    const w = 1 + (h % 3)
    if ((h >> 3) & 1) bars.push({ x: pos, w })
    pos += w + 1
  }
  return bars
})
const receiptHead = computed(() => t('receiptFor', { year: year.value }))

async function shareCard(): Promise<Blob> {
  const r = props.r
  const at = props.slides.findIndex(s => s.kind === 'summary')
  return renderRecordCard({
    year: r.year,
    head: t('receiptFor', { year: r.year }),
    name: x.firstName.value,
    colors: recordColors(props.slides[at >= 0 ? at : props.index]?.palette),
    artistsLabel: t('sumArtists'),
    artists: r.topArtists.map(a => ({ label: a.artist.name, value: x.num(a.minutes) })),
    tracksLabel: t('sumTracks'),
    tracks: r.topTracks.map(tr => ({ label: tr.track.title, value: `${x.num(tr.plays)}×` })),
    totals: [
      { label: t('sumMinutesTotal'), value: x.num(r.minutes) },
      ...(r.genres[0] ? [{ label: t('sumGenre'), value: r.genres[0].name }] : []),
      { label: t('sumPersona'), value: x.personaTitle(r) },
    ],
    thanks: t('thanks'),
    brand: t('brand'),
  })
}
defineExpose({ shareCard })
</script>

<template>
  <div class="rc" :class="{ 'is-back': dir < 0 }">
    <Transition name="rc">
      <section :key="index" class="slide" :class="`s-${kind}`" :style="vars" :aria-hidden="false">
        <i class="holes holes--l" aria-hidden="true" />
        <i class="holes holes--r" aria-hidden="true" />
        <div class="frame">
          <!-- Начало: конверт, из которого выезжает пластинка -->
          <template v-if="kind === 'intro'">
            <p class="kicker c-in" style="--d: 0">{{ r.complete ? t('sideA') : t('soFar') }}</p>
            <div class="sleeve obj" aria-hidden="true">
              <div class="disc">
                <i class="disc__grooves" /><i class="disc__shine" />
                <div class="disc__label">
                  <img v-if="discLabel" :src="discLabel" alt="">
                </div>
              </div>
              <div class="sleeve__cover">
                <img v-if="sleeveCover" :src="sleeveCover" alt="">
                <span v-else class="sleeve__plain">{{ t('brand') }}</span>
              </div>
            </div>
            <div class="foot">
              <h1 class="title c-in" style="--d: 3">{{ t('rewindYear', { year }) }}</h1>
              <p class="sub c-in" style="--d: 4">{{ x.firstName.value ? t('oneRecord', { name: x.firstName.value }) : t('oneRecordAnon') }}</p>
            </div>
          </template>

          <!-- Минуты: кассета со счётчиком плёнки -->
          <template v-else-if="kind === 'minutes'">
            <p class="kicker c-in" style="--d: 0">{{ t('minutesKicker') }}</p>
            <div class="cass obj" aria-hidden="true">
              <i class="screw" style="left: 3%; top: 5%" /><i class="screw" style="right: 3%; top: 5%" /><i class="screw" style="left: 3%; bottom: 6%" /><i class="screw" style="right: 3%; bottom: 6%" />
              <div class="cass__label">
                <div class="cass__title"><b>{{ t('mix', { year }) }}</b><span class="ellipsis">{{ x.firstName.value }}</span></div>
                <div class="cass__window"><i class="reel reel--full" /><i class="reel" /></div>
                <div class="cass__stripes"><i /><i /></div>
              </div>
              <div class="cass__foot"><i /><i /><i /><i /></div>
            </div>
            <div class="counter c-in" style="--d: 2" role="img" :aria-label="x.count('minutes', r.minutes)">
              <span v-for="(d, i) in digits" :key="i" class="dg" :class="{ 'dg--gap': digits.length > 3 && i === digits.length - 3 }">
                <span class="dg__strip" :style="{ '--n': d, '--d': i }"><b v-for="n in 10" :key="n">{{ n - 1 }}</b></span>
              </span>
            </div>
            <p class="sub c-in" style="--d: 3">{{ t('onTape', { time: x.nonstop(r.minutes) }) }}</p>
            <div class="vu c-in" style="--d: 4" role="img" :aria-label="t('monthsLabel')">
              <div v-for="m in vu" :key="m.month" class="vu__col" :class="{ 'is-peak': m.peak }">
                <i v-for="k in LEDS" :key="k" :class="{ 'is-lit': LEDS - k < m.lit, 'is-top': LEDS - k === m.lit - 1 }" :style="{ '--k': LEDS - k }" />
                <span>{{ x.monthLetter(m.month) }}</span>
              </div>
            </div>
          </template>

          <!-- Ритм: абонемент на год со штампом серии -->
          <template v-else-if="kind === 'days'">
            <p class="kicker c-in" style="--d: 0">{{ t('rhythm') }}</p>
            <div class="pass obj">
              <div class="pass__stub" aria-hidden="true"><span>{{ t('record.pass', { year }) }}</span></div>
              <div class="pass__main">
                <span class="pass__label">{{ x.word('daysWithMusic', r.days) }}</span>
                <b class="pass__num tnum">{{ x.num(r.days) }}</b>
                <span class="pass__of">{{ t('ofDays', { n: x.num(r.daysInRange) }) }}</span>
              </div>
              <div v-if="r.streak && r.streak.days > 1" class="stamp" aria-hidden="true">
                <b class="tnum">{{ x.num(r.streak.days) }}</b>
                <span>{{ x.word('inARow', r.streak.days) }}</span>
              </div>
            </div>
            <ol class="list c-in" style="--d: 4">
              <li v-if="r.streak && r.streak.days > 1">
                <span>{{ t('record.streak') }}</span><em class="tnum">{{ x.dayShort(r.streak.from) }} — {{ x.dayShort(r.streak.to) }}</em>
              </li>
              <li v-if="r.topDay">
                <span>{{ t('topDayShort') }}</span><em>{{ x.day(r.topDay.date) }}</em>
              </li>
              <li v-if="r.topDay">
                <span>{{ t('record.inDay') }}</span><em class="tnum">{{ x.num(r.topDay.minutes) }}{{ NB }}{{ t('minShort') }} · {{ trackCount(r.topDay.plays) }}</em>
              </li>
            </ol>
          </template>

          <!-- Первый трек года: один полароид на скотче -->
          <template v-else-if="kind === 'first' && r.firstTrack">
            <p class="kicker c-in" style="--d: 0">{{ t('firstKicker', { year }) }}</p>
            <div class="polas polas--one obj">
              <figure class="pola pola--1">
                <i class="tape" />
                <img v-if="x.cover(r.firstTrack.track)" :src="x.cover(r.firstTrack.track)!" alt="">
                <span v-else class="ph" />
                <figcaption>{{ t('record.firstAt', { day: x.day(r.firstTrack.at), time: x.timeOf(r.firstTrack.at) }) }}</figcaption>
              </figure>
            </div>
            <div class="foot">
              <h2 class="name c-in" style="--d: 3">{{ r.firstTrack.track.title }}</h2>
              <p class="sub c-in" style="--d: 4">{{ x.artistsOf(r.firstTrack.track) }}</p>
            </div>
          </template>

          <!-- Треки года: вкладыш кассеты с трек-листом -->
          <template v-else-if="kind === 'tracks'">
            <p class="kicker c-in" style="--d: 0">{{ t('topTracks') }}</p>
            <div class="jcard obj">
              <div class="jcard__spine" aria-hidden="true"><b>{{ t('mix', { year }) }}</b><span>{{ t('sideA') }}</span></div>
              <div class="jcard__body">
                <div class="jcard__head"><b>{{ t('mix', { year }) }}</b><span class="ellipsis">{{ x.firstName.value }}</span></div>
                <div class="cass__stripes" aria-hidden="true"><i /><i /></div>
                <ol class="jcard__list">
                  <li v-for="(tr, i) in tracks" :key="tr.track.id" :style="{ '--d': i }">
                    <span class="jcard__n tnum">{{ String(i + 1).padStart(2, '0') }}</span>
                    <span class="jcard__t">
                      <b class="ellipsis">{{ tr.track.title }}</b>
                      <small class="ellipsis">{{ x.artistsOf(tr.track) }}</small>
                    </span>
                    <span class="jcard__v tnum">{{ x.num(tr.plays) }}×</span>
                  </li>
                </ol>
              </div>
            </div>
            <div class="foot">
              <p class="sub c-in" style="--d: 6">{{ t('sideA') }} · {{ x.count('plays', tracks.reduce((a, tr) => a + tr.plays, 0)) }}</p>
            </div>
          </template>

          <!-- Трек года: пластинка под иглой -->
          <template v-else-if="kind === 'toy' && toy">
            <p class="kicker c-in" style="--d: 0">{{ t('toyKicker') }}</p>
            <div class="turn obj" aria-hidden="true">
              <div class="disc">
                <i class="disc__grooves" /><i class="disc__shine" />
                <div class="disc__label"><img v-if="x.cover(toy.track, 'medium')" :src="x.cover(toy.track, 'medium')!" alt=""></div>
              </div>
              <svg class="arm" viewBox="0 0 100 220">
                <circle cx="70" cy="22" r="16" class="arm__base" />
                <circle cx="70" cy="22" r="6" class="arm__pin" />
                <path d="M70 22 L74 150 L58 196" class="arm__rod" />
                <rect x="46" y="190" width="20" height="26" rx="3" transform="rotate(24 56 203)" class="arm__head" />
              </svg>
            </div>
            <div class="foot">
              <h2 class="name c-in" style="--d: 3">{{ toy.track.title }}</h2>
              <p class="sub c-in" style="--d: 4">{{ x.artistsOf(toy.track) }}</p>
              <div class="stamps c-in" style="--d: 5">
                <span><b class="tnum">{{ x.num(toy.plays) }}</b> {{ x.word('times', toy.plays) }}</span>
                <span>{{ t('since', { day: x.day(toy.firstPlayed) }) }}</span>
                <span>{{ x.monthIn(toy.peakMonth) }} — {{ t('mostOften') }}</span>
              </div>
            </div>
          </template>

          <!-- Артист года: стопка полароидов -->
          <template v-else-if="kind === 'artists' && artists[0]">
            <p class="kicker c-in" style="--d: 0">{{ t('topArtist') }}</p>
            <div class="polas obj">
              <figure v-if="artists[2]" class="pola pola--3" aria-hidden="true">
                <img v-if="pic(artists[2].artist.image, 'medium')" :src="pic(artists[2].artist.image, 'medium')!" alt=""><span v-else class="ph" />
              </figure>
              <figure v-if="artists[1]" class="pola pola--2" aria-hidden="true">
                <img v-if="pic(artists[1].artist.image, 'medium')" :src="pic(artists[1].artist.image, 'medium')!" alt=""><span v-else class="ph" />
              </figure>
              <figure class="pola pola--1">
                <i class="tape" />
                <img v-if="pic(artists[0].artist.image)" :src="pic(artists[0].artist.image)!" alt=""><span v-else class="ph" />
                <figcaption>
                  <span class="ellipsis">{{ artists[0].artist.name }}</span>
                  <small>{{ x.count('minutes', artists[0].minutes) }} · {{ x.count('plays', artists[0].plays) }}</small>
                </figcaption>
              </figure>
            </div>
            <ol class="list c-in" style="--d: 4">
              <li v-for="(a, i) in artists.slice(1, 4)" :key="a.artist.id">
                <b class="tnum">{{ i + 2 }}</b><span>{{ a.artist.name }}</span><em class="tnum">{{ x.num(a.minutes) }}{{ NB }}{{ t('minShort') }}</em>
              </li>
            </ol>
          </template>

          <!-- Альбомы: веер конвертов -->
          <template v-else-if="kind === 'albums' && albums[0]">
            <p class="kicker c-in" style="--d: 0">{{ t('topAlbums') }}</p>
            <div class="fan obj" aria-hidden="true">
              <div v-for="(a, i) in albums.slice(0, 4).reverse()" :key="a.album.id" class="fan__sleeve" :style="{ '--i': Math.min(albums.length, 4) - 1 - i }">
                <img v-if="pic(a.album.image, 'medium')" :src="pic(a.album.image, 'medium')!" alt=""><span v-else class="ph" />
              </div>
              <div class="fan__disc disc"><i class="disc__grooves" /><i class="disc__shine" /></div>
            </div>
            <div class="foot">
              <h2 class="name c-in" style="--d: 3">{{ albums[0].album.title }}</h2>
              <p class="sub c-in" style="--d: 4">{{ albums[0].album.artist.name }} · {{ x.count('times', albums[0].plays) }}</p>
              <ol class="list list--tight c-in" style="--d: 5">
                <li v-for="(a, i) in albums.slice(1, 4)" :key="a.album.id">
                  <b class="tnum">{{ i + 2 }}</b><span>{{ a.album.title }}</span><em class="tnum">{{ x.num(a.plays) }}×</em>
                </li>
              </ol>
            </div>
          </template>

          <!-- Звучание: шкала приёмника и индикаторы жанров -->
          <template v-else-if="kind === 'sound'">
            <p class="kicker c-in" style="--d: 0">{{ t('soundKicker') }}</p>
            <template v-if="genres.length">
              <div class="tuner obj" aria-hidden="true">
                <div class="tuner__scale">
                  <i v-for="k in 41" :key="k" class="tuner__tick" :class="{ 'is-long': (k - 1) % 5 === 0 }" />
                  <span v-for="(g, i) in genres" :key="g.id" class="tuner__name" :style="{ left: `${6 + i * (70 / Math.max(1, genres.length - 1))}%` }">{{ g.name }}</span>
                  <i class="tuner__needle" style="left: 6%" />
                </div>
                <div class="tuner__knobs"><i /><span>{{ t('record.tuner') }}</span><i /></div>
              </div>
              <h2 class="name c-in" style="--d: 3">{{ genres[0]!.name }}</h2>
              <ul class="meters c-in" style="--d: 4">
                <li v-for="g in genres" :key="g.id">
                  <span class="meters__name ellipsis">{{ g.name }}</span>
                  <span class="meters__bar" aria-hidden="true"><i v-for="k in SEGMENTS" :key="k" :class="{ 'is-lit': k <= genreLit(g.share), 'is-top': k === genreLit(g.share) }" :style="{ '--k': k }" /></span>
                  <span class="meters__pct tnum">{{ x.pct(g.share / genreTotal) }}</span>
                </li>
              </ul>
              <div v-if="moods.length" class="stamps c-in" style="--d: 5">
                <span v-for="m in moods" :key="m.id">{{ t(`mood.${m.id}`) }} <b class="tnum">{{ x.pct(m.score) }}</b></span>
              </div>
            </template>
            <div v-else class="foot">
              <h2 class="name c-in" style="--d: 2">{{ t('soundEmpty') }}</h2>
              <p class="sub c-in" style="--d: 3">{{ t('soundEmptyText') }}</p>
            </div>
          </template>

          <!-- Открытия: лента фотобудки -->
          <template v-else-if="kind === 'discoveries'">
            <p class="kicker c-in" style="--d: 0">{{ t('discKicker') }}</p>
            <div class="booth">
              <div class="strip obj" aria-hidden="true">
                <span v-for="k in 4" :key="k" class="strip__shot">
                  <img v-if="r.discoveries[k - 1] && pic(r.discoveries[k - 1]!.artist.image, 'medium')" :src="pic(r.discoveries[k - 1]!.artist.image, 'medium')!" alt="">
                  <span v-else class="ph" />
                </span>
              </div>
              <div v-if="r.newArtists > 0" class="booth__count c-in" style="--d: 2">
                <b class="tnum">{{ x.num(r.newArtists) }}</b>
                <span>{{ x.word('newArtists', r.newArtists) }}</span>
              </div>
            </div>
            <ol v-if="r.discoveries.length" class="list c-in" style="--d: 4">
              <li v-for="d in r.discoveries.slice(0, 3)" :key="d.artist.id">
                <span>{{ d.artist.name }}</span><em class="tnum">{{ t('discSince', { day: x.dayShort(d.firstHeard) }) }} · {{ x.num(d.plays) }}×</em>
              </li>
            </ol>
            <div v-else class="foot">
              <h2 class="name c-in" style="--d: 3">{{ r.newArtists > 0 ? t('discKicker') : t('discEmpty') }}</h2>
              <p class="sub c-in" style="--d: 4">{{ r.newArtists > 0 ? t('discNoFav') : t('discEmptyText') }}</p>
            </div>
          </template>

          <!-- Характер: эмалевый значок -->
          <template v-else-if="kind === 'personality'">
            <p class="kicker c-in" style="--d: 0">{{ t('personaKicker') }}</p>
            <div class="pin obj" aria-hidden="true">
              <svg viewBox="0 0 200 200">
                <defs><path id="rc-ring" d="M100,100 m-71,0 a71,71 0 1,1 142,0 a71,71 0 1,1 -142,0" /></defs>
                <circle cx="100" cy="100" r="97" class="pin__rim" />
                <circle cx="100" cy="100" r="90" class="pin__enamel" />
                <circle cx="100" cy="100" r="58" class="pin__core" />
                <text class="pin__ring"><textPath href="#rc-ring" textLength="440" lengthAdjust="spacingAndGlyphs">{{ ring }}</textPath></text>
                <path v-if="persona.id === 'night_owl'" d="M88 64 a28 28 0 1 0 30 40 a22 22 0 1 1 -30 -40z" class="pin__icon" />
                <g v-else-if="persona.id === 'early_bird'" class="pin__icon">
                  <circle cx="100" cy="92" r="14" />
                  <path d="M100 62v8M100 114v8M70 92h8M122 92h8M79 71l6 6M115 107l6 6M79 113l6-6M115 77l6-6" class="pin__stroke" />
                </g>
                <path v-else-if="persona.id === 'explorer'" d="M100 60 l9 23 24 9 -24 9 -9 23 -9 -23 -24 -9 24 -9z" class="pin__icon" />
                <path v-else-if="persona.id === 'loyal'" d="M100 118 C 78 102 70 92 70 80 a14 14 0 0 1 30 -6 a14 14 0 0 1 30 6 c0 12 -8 22 -30 38z" class="pin__icon" />
                <path v-else-if="persona.id === 'wave_rider'" d="M66 96 q8.5 -16 17 0 t17 0 t17 0 t17 0" class="pin__stroke pin__stroke--wide" />
                <g v-else class="pin__icon">
                  <circle cx="88" cy="104" r="9" /><circle cx="116" cy="98" r="9" />
                  <path d="M96 104V70l29-6v34" class="pin__stroke" />
                </g>
                <text x="100" y="146" class="pin__time">{{ x.hour(persona.peakHour) }}</text>
              </svg>
            </div>
            <div class="foot">
              <h2 class="name c-in" style="--d: 3">{{ x.personaTitle(r) }}</h2>
              <p class="sub c-in" style="--d: 4">{{ x.personaText(r) }}. {{ t('peakHour', { time: x.hour(persona.peakHour) }) }}</p>
            </div>
          </template>

          <!-- Итог: чек из принтера -->
          <template v-else-if="kind === 'summary'">
            <div class="printer">
              <i class="slot" aria-hidden="true" />
              <div class="receipt-clip">
                <div class="receipt">
                  <p class="r-head"><b>{{ t('brand') }}</b>{{ receiptHead }}<template v-if="x.firstName.value"><br>{{ x.firstName.value }}</template></p>
                  <p class="r-sec">{{ t('sumArtists') }}</p>
                  <div v-for="(a, i) in artists" :key="a.artist.id" class="row"><span>{{ i + 1 }}. {{ a.artist.name }}</span><i /><span class="tnum">{{ x.num(a.minutes) }}</span></div>
                  <p class="r-sec">{{ t('sumTracks') }}</p>
                  <div v-for="(tr, i) in tracks" :key="tr.track.id" class="row"><span>{{ i + 1 }}. {{ tr.track.title }}</span><i /><span class="tnum">{{ x.num(tr.plays) }}×</span></div>
                  <div class="r-total">
                    <div class="row"><span>{{ t('sumMinutesTotal') }}</span><i /><span class="tnum">{{ x.num(r.minutes) }}</span></div>
                    <div v-if="r.genres[0]" class="row"><span>{{ t('sumGenre') }}</span><i /><span>{{ r.genres[0].name }}</span></div>
                    <div class="row"><span>{{ t('sumPersona') }}</span><i /><span>{{ x.personaTitle(r) }}</span></div>
                  </div>
                  <svg class="bar" viewBox="0 0 200 36" preserveAspectRatio="none" aria-hidden="true">
                    <rect v-for="b in barcode" :key="b.x" :x="b.x" y="0" :width="b.w" height="36" />
                  </svg>
                  <p class="r-thanks">{{ t('thanks') }}</p>
                </div>
              </div>
            </div>
            <div class="actions c-in" style="--d: 8" @pointerdown.stop @pointerup.stop @click.stop>
              <button type="button" class="btn btn--primary" :disabled="busy === 'share'" @click="emit('share')">
                <Icon :name="busy === 'share' ? 'ph:circle-notch' : 'ph:upload-simple'" :class="{ spin: busy === 'share' }" />{{ t('share') }}
              </button>
              <button type="button" class="btn" :disabled="busy === 'playlist'" @click="emit('playlist')">
                <Icon :name="busy === 'playlist' ? 'ph:circle-notch' : r.playlistId ? 'ph:playlist' : 'ph:plus'" :class="{ spin: busy === 'playlist' }" />{{ r.playlistId ? t('openPlaylist') : t('savePlaylist') }}
              </button>
            </div>
          </template>

          <!-- Дальше: волна из любимого — пластинка крутится -->
          <template v-else-if="kind === 'wave'">
            <p class="kicker c-in" style="--d: 0">{{ t('nextKicker') }}</p>
            <div class="spin-disc obj" aria-hidden="true">
              <div class="disc">
                <i class="disc__grooves" /><i class="disc__shine" />
                <div class="disc__label"><img v-if="waveLabel" :src="waveLabel" alt=""></div>
              </div>
            </div>
            <div class="foot">
              <h2 class="name c-in" style="--d: 3">{{ t('waveTitle', { year }) }}</h2>
              <p class="sub c-in" style="--d: 4">{{ t('waveText', { year }) }}</p>
              <div class="actions actions--one c-in" style="--d: 5" @pointerdown.stop @pointerup.stop @click.stop>
                <button type="button" class="btn btn--primary" @click="emit('wave')">
                  <Icon name="ph:play-fill" />{{ t('waveStart') }}
                </button>
              </div>
            </div>
          </template>

          <!-- Ещё послушать: ящик с пластинками -->
          <template v-else-if="kind === 'more'">
            <p class="kicker c-in" style="--d: 0">{{ t('moreKicker') }}</p>
            <h2 class="name name--top c-in" style="--d: 1">{{ t('moreTitle') }}</h2>
            <div class="crate obj" aria-hidden="true">
              <span v-for="(u, i) in crate" :key="u" class="crate__sleeve" :style="{ '--i': i, '--n': crate.length }"><img :src="u" alt=""></span>
              <span v-if="!crate.length" class="crate__sleeve crate__sleeve--empty" style="--i: 0; --n: 1" />
              <i class="crate__box" />
            </div>
            <ol v-if="moreItems.length" class="list c-in" style="--d: 4">
              <li v-for="tr in moreItems" :key="tr.id">
                <span class="list__two"><span class="ellipsis">{{ tr.title }}</span><small class="ellipsis">{{ x.artistsOf(tr) }}</small></span>
              </li>
            </ol>
            <p v-else class="sub c-in" style="--d: 4">{{ more ? t('moreEmpty') : t('moreLoading') }}</p>
            <div v-if="moreItems.length" class="actions c-in" style="--d: 6" @pointerdown.stop @pointerup.stop @click.stop>
              <button type="button" class="btn btn--primary" @click="emit('playMore')">
                <Icon name="ph:play-fill" />{{ t('morePlay') }}
              </button>
              <button type="button" class="btn" :disabled="busy === 'more'" @click="emit('saveMore')">
                <Icon :name="busy === 'more' ? 'ph:circle-notch' : more?.playlistId ? 'ph:playlist' : 'ph:plus'" :class="{ spin: busy === 'more' }" />{{ more?.playlistId ? t('openPlaylist') : t('savePlaylist') }}
              </button>
            </div>
          </template>

          <!-- Год без прослушиваний: пустой конверт -->
          <template v-else>
            <p class="kicker c-in" style="--d: 0">{{ t('sideA') }}</p>
            <div class="sleeve sleeve--empty obj" aria-hidden="true">
              <div class="disc"><i class="disc__grooves" /><i class="disc__shine" /><div class="disc__label disc__label--plain" /></div>
              <div class="sleeve__cover"><span class="sleeve__plain">{{ t('brand') }}<br>{{ year }}</span></div>
            </div>
            <div class="foot">
              <h1 class="title c-in" style="--d: 3">{{ t('emptyTitle') }}</h1>
              <p class="sub c-in" style="--d: 4">{{ t('emptyText', { year }) }}</p>
            </div>
          </template>
        </div>
      </section>
    </Transition>
    <i class="grain" aria-hidden="true" />
  </div>
</template>

<style scoped>
.rc {
  --ease-out-rc: cubic-bezier(0.2, 0.7, 0.2, 1);
  --ease-io-rc: cubic-bezier(0.65, 0, 0.35, 1);
  --cream: #f3ecdf;
  --soft: rgba(243, 236, 223, 0.72);
  --paper: #f4f0e6;
  --paper-ink: #1b1813;
  --pad: 6.2cqw;
  position: absolute;
  inset: 0;
  z-index: 1;
  isolation: isolate;
  overflow: hidden;
  background: #0d0b09;
  color: var(--cream);
  font-family: var(--font);
  -webkit-font-smoothing: antialiased;
}
/* Зерно бумаги поверх всего */
.grain {
  position: absolute;
  inset: 0;
  z-index: 20;
  pointer-events: none;
  opacity: 0.09;
  mix-blend-mode: overlay;
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='160' height='160'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 1.4 -0.2'/></filter><rect width='160' height='160' filter='url(%23n)'/></svg>");
}

.slide { position: absolute; inset: 0; background: var(--bg); color: var(--cream); }
.frame {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  padding: calc(max(10px, var(--rw-top, 0px)) + 64px) var(--pad) max(28px, var(--rw-bottom, 0px));
}
.tnum { font-variant-numeric: tabular-nums; }
.ellipsis { display: block; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ph { display: block; width: 100%; height: 100%; aspect-ratio: 1; background: rgba(27, 24, 19, 0.16); }

.kicker { margin: 0; font-size: max(12px, 3.1cqw); font-weight: 650; letter-spacing: 0.12em; text-transform: uppercase; color: var(--soft); }
.foot { margin-top: auto; min-width: 0; }
.title { margin: 0; font-size: 13.4cqw; line-height: 0.95; font-weight: 800; letter-spacing: -0.04em; text-wrap: balance; }
.name {
  margin: 0;
  font-size: max(26px, 8.6cqw);
  line-height: 1.02;
  font-weight: 800;
  letter-spacing: -0.035em;
  text-wrap: balance;
  overflow-wrap: anywhere;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.name--top { margin-top: 3cqw; }
.sub { margin: 2.6cqw 0 0; font-size: max(15px, 4.3cqw); line-height: 1.35; font-weight: 500; color: var(--soft); text-wrap: pretty; }
.obj { flex: none; }

/* Пластинка */
.disc { position: relative; aspect-ratio: 1; border-radius: 50%; background: #0c0b0b; box-shadow: 0 18px 40px -16px rgba(0, 0, 0, 0.8), inset 0 0 0 1px rgba(255, 255, 255, 0.1); overflow: hidden; }
.disc__grooves { position: absolute; inset: 0; border-radius: 50%; background: repeating-radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.045) 0 1px, transparent 1px 3.5px); }
/* Блик на виниле — свет на дорожках: он стоит, пластинка крутится под ним */
.disc__shine { position: absolute; inset: 0; z-index: 1; border-radius: 50%; background: conic-gradient(from 30deg, transparent 0 8%, rgba(255, 255, 255, 0.13) 12%, transparent 18% 50%, rgba(255, 255, 255, 0.1) 62%, transparent 68%); }
.disc__label { position: absolute; inset: 32%; border-radius: 50%; overflow: hidden; background: var(--obj); }
.disc__label img { width: 100%; height: 100%; object-fit: cover; display: block; }
.disc__label::after { content: ''; position: absolute; left: 50%; top: 50%; width: 9%; aspect-ratio: 1; border-radius: 50%; background: #0c0b0b; transform: translate(-50%, -50%); }

/* Конверт */
.sleeve { position: relative; width: 62cqw; aspect-ratio: 1; margin: auto 0; }
.sleeve .disc { position: absolute; inset: 2%; transform: translateX(34%); }
.sleeve__cover { position: absolute; inset: 0; z-index: 1; display: grid; place-items: center; background: var(--obj-deep); box-shadow: 0 24px 50px -20px rgba(0, 0, 0, 0.9), 0 0 0 1px rgba(255, 255, 255, 0.06); }
.sleeve__cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
.sleeve__plain { font-size: 7cqw; font-weight: 800; letter-spacing: -0.03em; line-height: 1.05; text-align: center; color: var(--paper); }
.sleeve--empty .sleeve__cover { background: var(--paper); }
.sleeve--empty .sleeve__plain { color: var(--paper-ink); }
.disc__label--plain { background: var(--obj); }

/* Кассета */
.cass { position: relative; width: 88cqw; aspect-ratio: 1.6; margin: auto auto 0; border-radius: 3.4cqw; background: var(--obj-deep); box-shadow: 0 20px 40px -18px rgba(0, 0, 0, 0.85), inset 0 0 0 1px rgba(255, 255, 255, 0.08); }
.screw { position: absolute; width: 3cqw; aspect-ratio: 1; border-radius: 50%; background: rgba(0, 0, 0, 0.35); box-shadow: inset 0 0 0 0.6cqw rgba(255, 255, 255, 0.18); }
.cass__label { position: absolute; left: 7%; right: 7%; top: 8%; height: 64%; border-radius: 1.6cqw; background: var(--paper); color: var(--paper-ink); display: flex; flex-direction: column; padding: 2.4cqw 3.4cqw; }
.cass__title { display: flex; justify-content: space-between; align-items: baseline; gap: 3cqw; font-size: max(12px, 3.4cqw); }
.cass__title b { flex: none; font-weight: 800; letter-spacing: 0.04em; }
.cass__title span { font-weight: 500; opacity: 0.7; }
.cass__window { position: relative; margin: auto auto 0; width: 64%; height: 46%; border-radius: 99px; background: #17130f; display: flex; justify-content: space-between; align-items: center; padding: 0 5%; }
.cass__stripes { display: grid; gap: 0.8cqw; margin-top: 2cqw; }
.cass__stripes i { height: 1cqw; border-radius: 1px; background: var(--obj); }
.cass__stripes i + i { background: var(--obj-deep); }
.reel { position: relative; width: 11cqw; aspect-ratio: 1; border-radius: 50%; background: repeating-conic-gradient(var(--paper) 0 14deg, transparent 14deg 60deg); -webkit-mask: radial-gradient(circle, transparent 30%, #000 31% 68%, transparent 69%); mask: radial-gradient(circle, transparent 30%, #000 31% 68%, transparent 69%); }
.reel--full { box-shadow: 0 0 0 2.4cqw #4a3426; -webkit-mask: none; mask: none; background: repeating-conic-gradient(var(--paper) 0 14deg, #17130f 14deg 60deg); }
.cass__foot { position: absolute; left: 22%; right: 22%; bottom: 0; height: 18%; border-radius: 2cqw 2cqw 0 0; background: rgba(0, 0, 0, 0.22); display: flex; justify-content: space-around; align-items: center; }
.cass__foot i { width: 2.6cqw; aspect-ratio: 1; border-radius: 50%; background: rgba(0, 0, 0, 0.45); }

.counter { display: flex; gap: 1.2cqw; margin-top: 8cqw; font-size: 15cqw; line-height: 1; font-weight: 750; font-variant-numeric: tabular-nums; }
.dg { display: block; height: 1.18em; overflow: hidden; padding: 0 1.4cqw; border-radius: 1.6cqw; background: #0a0908; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08); }
.dg--gap { margin-left: 2.6cqw; }
.dg__strip { display: flex; flex-direction: column; transform: translateY(calc(var(--n) * -1.18em)); }
.dg__strip b { display: block; height: 1.18em; line-height: 1.18em; font-weight: 750; text-align: center; }
.s-minutes .sub { margin-top: 3cqw; }
.vu { display: grid; grid-template-columns: repeat(12, 1fr); gap: 1.6cqw; margin: 10cqw 0 auto; }
.vu__col { display: flex; flex-direction: column; gap: 0.9cqw; }
.vu__col i { height: 1.7cqw; border-radius: 1px; background: rgba(243, 236, 223, 0.1); }
.vu__col i.is-lit { background: rgba(243, 236, 223, 0.72); }
.vu__col i.is-top { background: var(--obj); }
.vu__col.is-peak i.is-lit:not(.is-top) { background: var(--cream); }
.vu__col span { margin-top: 1.6cqw; text-align: center; font-size: max(12px, 3cqw); font-weight: 600; color: var(--soft); }
.vu__col.is-peak span { color: var(--cream); }

/* Абонемент */
.pass {
  position: relative;
  display: flex;
  width: 88cqw;
  margin: auto auto 0;
  border-radius: 2cqw;
  background: var(--paper);
  color: var(--paper-ink);
  box-shadow: 0 20px 40px -18px rgba(0, 0, 0, 0.85);
  transform: rotate(-2deg);
}
.pass__stub {
  display: grid;
  place-items: center;
  width: 15cqw;
  border-right: 1.5px dashed rgba(27, 24, 19, 0.4);
  background: var(--obj);
  border-radius: 2cqw 0 0 2cqw;
  color: var(--paper);
}
.pass__stub span { writing-mode: vertical-rl; transform: rotate(180deg); font-size: max(12px, 3.2cqw); font-weight: 750; letter-spacing: 0.12em; text-transform: uppercase; white-space: nowrap; }
.pass__main { display: flex; flex: 1; flex-direction: column; min-width: 0; padding: 5cqw 6cqw 5.4cqw; }
.pass__label { font-size: max(12px, 3.2cqw); font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; opacity: 0.66; }
.pass__num { margin-top: 1cqw; font-size: 26cqw; line-height: 0.9; font-weight: 800; letter-spacing: -0.05em; }
.pass__of { margin-top: 2cqw; font-size: max(14px, 4cqw); font-weight: 600; opacity: 0.72; }
.stamp {
  position: absolute;
  right: -3cqw;
  bottom: -11cqw;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 30cqw;
  aspect-ratio: 1;
  border: 0.9cqw solid var(--obj);
  border-radius: 50%;
  color: var(--obj);
  text-align: center;
  transform: rotate(-14deg);
  mix-blend-mode: multiply;
}
.stamp b { font-size: 10cqw; line-height: 0.95; font-weight: 800; letter-spacing: -0.04em; }
.stamp span { max-width: 22cqw; font-size: max(11px, 2.8cqw); font-weight: 750; letter-spacing: 0.06em; text-transform: uppercase; line-height: 1.1; }
.s-days .list { margin-top: 12cqw; margin-bottom: auto; }

/* Списки: пунктирные волоски, как на вкладыше */
.list { list-style: none; margin: auto 0 0; padding: 0; }
.list li { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: baseline; gap: 3cqw; padding: 2.4cqw 0; border-top: 1px dashed rgba(243, 236, 223, 0.22); font-size: max(15px, 4.1cqw); font-weight: 600; }
.list li:has(> b) { grid-template-columns: 6cqw minmax(0, 1fr) auto; gap: 0; }
.list b { color: var(--soft); font-weight: 600; }
.list li > span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.list em { font-style: normal; color: var(--soft); white-space: nowrap; }
.list--tight { margin-top: 4cqw; }
.list--tight li { padding: 1.8cqw 0; font-size: max(14px, 3.8cqw); }
.list__two { display: flex !important; flex-direction: column; min-width: 0; white-space: normal !important; }
.list__two small { margin-top: 0.4cqw; font-size: max(13px, 3.5cqw); font-weight: 500; color: var(--soft); }

/* Полароиды */
.polas { position: relative; width: 70cqw; height: 88cqw; margin: 7cqw auto 0; }
.pola { position: absolute; inset: 0; margin: 0; padding: 3.6cqw 3.6cqw 0; background: var(--paper); color: var(--paper-ink); box-shadow: 0 18px 40px -16px rgba(0, 0, 0, 0.85); }
.pola img, .pola .ph { width: 100%; aspect-ratio: 1; object-fit: cover; display: block; }
.pola--3 { transform: rotate(9deg) translate(8%, -2%); }
.pola--2 { transform: rotate(-8deg) translate(-7%, 1%); }
.pola--1 { transform: rotate(-2deg); }
.pola figcaption { padding: 3.4cqw 0.6cqw 0; font-size: max(20px, 6.2cqw); line-height: 1.05; font-weight: 800; letter-spacing: -0.03em; }
.pola figcaption small { display: block; margin-top: 1.4cqw; font-size: max(12px, 3.2cqw); font-weight: 500; letter-spacing: 0; opacity: 0.68; }
.tape { position: absolute; left: 50%; top: -3cqw; z-index: 1; width: 26cqw; height: 7cqw; background: rgba(236, 224, 196, 0.72); transform: translateX(-50%) rotate(-3deg); }
.polas--one { width: 66cqw; height: 84cqw; margin-top: 8cqw; }
.polas--one .pola figcaption { font-size: max(18px, 5.4cqw); }
.polas--one .pola--1 { transform: rotate(3deg); }

/* Вкладыш кассеты */
.jcard { display: flex; width: 88cqw; margin: auto; border-radius: 1.4cqw; overflow: hidden; background: var(--paper); color: var(--paper-ink); box-shadow: 0 20px 40px -18px rgba(0, 0, 0, 0.85); transform: rotate(-1deg); }
.jcard__spine { display: flex; flex-direction: column; justify-content: space-between; align-items: center; width: 11cqw; padding: 4cqw 0; background: var(--obj); color: var(--paper); }
.jcard__spine b, .jcard__spine span { writing-mode: vertical-rl; transform: rotate(180deg); font-size: max(11px, 3cqw); letter-spacing: 0.1em; text-transform: uppercase; white-space: nowrap; }
.jcard__spine b { font-weight: 800; }
.jcard__spine span { font-weight: 600; opacity: 0.8; }
.jcard__body { flex: 1; min-width: 0; padding: 4cqw 4.4cqw 3cqw; }
.jcard__head { display: flex; justify-content: space-between; align-items: baseline; gap: 3cqw; font-size: max(13px, 3.6cqw); }
.jcard__head b { flex: none; font-weight: 800; letter-spacing: 0.04em; }
.jcard__head span { font-weight: 500; opacity: 0.7; }
.jcard__list { list-style: none; margin: 3cqw 0 0; padding: 0; }
.jcard__list li { display: flex; align-items: center; gap: 2.4cqw; padding: 2.2cqw 0; border-top: 1.5px dashed rgba(27, 24, 19, 0.24); }
.jcard__n { flex: none; width: 6cqw; font-size: max(12px, 3.4cqw); font-weight: 700; opacity: 0.55; }
.jcard__t { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.jcard__t b { font-size: max(14px, 4cqw); font-weight: 750; letter-spacing: -0.01em; }
.jcard__t small { margin-top: 0.4cqw; font-size: max(12px, 3.2cqw); font-weight: 500; opacity: 0.62; }
.jcard__v { flex: none; font-size: max(13px, 3.6cqw); font-weight: 700; }

/* Проигрыватель */
.turn { position: relative; width: 100%; margin-top: 5cqw; height: 84cqw; }
.turn .disc { position: absolute; left: -4cqw; top: 0; width: 84cqw; }
.arm { position: absolute; right: -2cqw; top: -4cqw; width: 26cqw; transform-origin: 70% 10%; }
.arm__base { fill: #2a2622; stroke: rgba(255, 255, 255, 0.12); }
.arm__pin { fill: var(--cream); }
.arm__rod { fill: none; stroke: var(--cream); stroke-width: 4; stroke-linecap: round; stroke-linejoin: round; }
.arm__head { fill: var(--obj); }
.stamps { display: flex; flex-wrap: wrap; gap: 2cqw; margin-top: 4cqw; }
.stamps span { padding: 1.6cqw 3cqw; border: 1.5px solid rgba(243, 236, 223, 0.5); border-radius: 1.2cqw; font-size: max(13px, 3.5cqw); font-weight: 600; transform: rotate(-1deg); }
.stamps span:nth-child(2) { transform: rotate(1.2deg); }
.stamps b { font-weight: 800; }

/* Веер конвертов */
.fan { position: relative; width: 100%; height: 66cqw; margin-top: 6cqw; }
.fan__sleeve {
  position: absolute;
  top: calc(var(--i) * 1.6cqw);
  left: calc(2cqw + var(--i) * 9cqw);
  z-index: calc(10 - var(--i));
  width: 54cqw;
  aspect-ratio: 1;
  overflow: hidden;
  background: var(--obj-deep);
  box-shadow: 0 18px 36px -16px rgba(0, 0, 0, 0.9), 0 0 0 1px rgba(255, 255, 255, 0.06);
  transform: rotate(calc(var(--i) * 3deg));
  transform-origin: 50% 100%;
}
.fan__sleeve img { width: 100%; height: 100%; object-fit: cover; display: block; }
.fan__disc { position: absolute; z-index: 9; top: 2cqw; left: 24cqw; width: 50cqw; }
.fan__sleeve:last-of-type { z-index: 10; }

/* Приёмник */
.tuner { margin-top: 6cqw; padding: 3cqw; border-radius: 2.6cqw; background: var(--obj-deep); box-shadow: 0 20px 40px -18px rgba(0, 0, 0, 0.85), inset 0 0 0 1px rgba(255, 255, 255, 0.08); }
.tuner__scale { position: relative; height: 22cqw; padding: 2cqw 3cqw 0; border-radius: 1.4cqw; background: var(--paper); color: var(--paper-ink); overflow: hidden; display: flex; align-items: flex-start; justify-content: space-between; }
.tuner__tick { width: 1.5px; height: 2.4cqw; background: rgba(27, 24, 19, 0.45); }
.tuner__tick.is-long { height: 4cqw; background: rgba(27, 24, 19, 0.75); }
.tuner__name { position: absolute; bottom: 2cqw; max-width: 20cqw; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: max(11px, 2.9cqw); font-weight: 700; transform: rotate(-30deg); transform-origin: 0 100%; opacity: 0.72; }
.tuner__name:first-of-type { opacity: 1; }
.tuner__needle { position: absolute; top: 0; bottom: 0; width: 0.8cqw; margin-left: -0.4cqw; background: var(--obj); }
.tuner__knobs { display: flex; align-items: center; justify-content: space-between; margin-top: 2.4cqw; padding: 0 1cqw; }
.tuner__knobs i { width: 8cqw; aspect-ratio: 1; border-radius: 50%; background: repeating-conic-gradient(#2a2622 0 10deg, #1c1916 10deg 20deg); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12); }
.tuner__knobs span { font-size: max(11px, 2.9cqw); font-weight: 700; letter-spacing: 0.12em; text-transform: uppercase; color: rgba(243, 236, 223, 0.7); }
.s-sound .name { margin-top: 6cqw; }
.meters { list-style: none; margin: 4cqw 0 0; padding: 0; display: flex; flex-direction: column; gap: 2.6cqw; }
.meters li { display: grid; grid-template-columns: 26cqw 1fr 11cqw; align-items: center; gap: 3cqw; font-size: max(13px, 3.6cqw); font-weight: 600; }
.meters__bar { display: grid; grid-template-columns: repeat(16, 1fr); gap: 0.7cqw; }
.meters__bar i { height: 3cqw; border-radius: 1px; background: rgba(243, 236, 223, 0.1); }
.meters__bar i.is-lit { background: rgba(243, 236, 223, 0.72); }
.meters__bar i.is-top { background: var(--obj); }
.meters__pct { text-align: right; color: var(--soft); }
.s-sound .stamps { margin-top: auto; }

/* Фотобудка */
.booth { display: flex; align-items: flex-end; gap: 6cqw; margin-top: 6cqw; }
.strip { display: flex; flex-direction: column; gap: 2cqw; width: 34cqw; padding: 2.6cqw 2.6cqw 7cqw; background: var(--paper); box-shadow: 0 18px 40px -16px rgba(0, 0, 0, 0.85); transform: rotate(-3deg); }
.strip__shot { display: block; aspect-ratio: 1; overflow: hidden; background: #2a2622; }
.strip__shot img { width: 100%; height: 100%; object-fit: cover; display: block; filter: grayscale(0.35) contrast(1.05); }
.strip__shot .ph { background: #d8cfbd; box-shadow: inset 0 0 0 1px rgba(27, 24, 19, 0.08); }
.booth__count { display: flex; flex-direction: column; min-width: 0; padding-bottom: 2cqw; }
.booth__count b { font-size: 24cqw; line-height: 0.85; font-weight: 800; letter-spacing: -0.05em; }
.booth__count span { margin-top: 2cqw; font-size: max(16px, 4.6cqw); font-weight: 700; line-height: 1.15; }

/* Значок */
.pin { width: 70cqw; margin: 8cqw auto 0; filter: drop-shadow(0 18px 24px rgba(0, 0, 0, 0.6)); }
.pin svg { display: block; width: 100%; }
.pin__rim { fill: #d9cdb6; }
.pin__enamel { fill: var(--obj-deep); }
.pin__core { fill: #0f0d0b; }
.pin__ring { fill: var(--cream); font: 700 11.5px var(--font); letter-spacing: 0.06em; }
.pin__icon { fill: var(--cream); }
.pin__stroke { fill: none; stroke: var(--cream); stroke-width: 5; stroke-linecap: round; stroke-linejoin: round; }
.pin__stroke--wide { stroke-width: 7; }
.pin__time { fill: var(--cream); font: 800 22px var(--font); text-anchor: middle; letter-spacing: -0.02em; font-variant-numeric: tabular-nums; }

/* Чек */
.s-summary .frame { padding-top: calc(max(10px, var(--rw-top, 0px)) + 56px); }
.printer { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; align-items: center; }
.slot { display: block; width: 92%; height: 10px; border-radius: 99px; background: #050404; box-shadow: inset 0 2px 3px rgba(0, 0, 0, 0.9), 0 1px 0 rgba(255, 255, 255, 0.06); flex: none; }
.receipt-clip { width: 84%; margin-top: -5px; overflow: hidden; flex: 1; min-height: 0; }
.receipt {
  padding: 5cqw 5cqw 7cqw;
  background: var(--paper);
  color: var(--paper-ink);
  font-size: max(12px, 3.2cqw);
  line-height: 1.35;
  font-variant-numeric: tabular-nums;
  -webkit-mask: conic-gradient(from -45deg at bottom, #0000, #000 1deg 89deg, #0000 90deg) 50% / 10px 100%;
  mask: conic-gradient(from -45deg at bottom, #0000, #000 1deg 89deg, #0000 90deg) 50% / 10px 100%;
}
.receipt p { margin: 0; }
.r-head { text-align: center; padding-bottom: 3cqw; border-bottom: 1.5px dashed rgba(27, 24, 19, 0.4); font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; }
.r-head b { display: block; font-size: max(18px, 5.4cqw); letter-spacing: 0.02em; text-transform: none; font-weight: 800; }
.r-sec { margin-top: 3cqw !important; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; opacity: 0.6; font-size: max(11px, 2.9cqw); }
.row { display: flex; align-items: baseline; gap: 1.4cqw; padding: 0.5cqw 0; font-weight: 600; }
.row span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row i { flex: 1; min-width: 4cqw; border-bottom: 1.5px dotted rgba(27, 24, 19, 0.35); transform: translateY(-0.25em); }
.row span:last-child { flex: none; max-width: 50%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.r-total { margin-top: 3cqw; padding-top: 2cqw; border-top: 1.5px dashed rgba(27, 24, 19, 0.4); }
.r-total .row:first-child { font-size: max(15px, 4.2cqw); font-weight: 800; }
.bar { display: block; width: 100%; height: 9cqw; margin-top: 4cqw; fill: var(--paper-ink); }
.r-thanks { margin-top: 2cqw !important; text-align: center; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; font-size: max(11px, 2.9cqw); opacity: 0.7; }

/* Кнопки */
.actions { display: grid; grid-template-columns: 1fr 1fr; gap: 3cqw; margin-top: 4cqw; }
.actions--one { grid-template-columns: 1fr; margin-top: 6cqw; }
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-width: 0;
  min-height: 48px;
  padding: 0 16px;
  border: 0;
  border-radius: 999px;
  background: rgba(243, 236, 223, 0.12);
  color: var(--cream);
  font: 650 max(15px, 4cqw) / 1 var(--font);
  white-space: nowrap;
  cursor: pointer;
  transition: background 0.15s, transform 0.1s;
}
.btn :deep(svg) { width: 20px; height: 20px; flex: none; }
.btn:hover { background: rgba(243, 236, 223, 0.18); }
.btn:active { transform: scale(0.97); }
.btn:disabled { opacity: 0.6; cursor: default; }
.btn:focus-visible { outline: 2px solid var(--cream); outline-offset: 2px; }
.btn--primary { background: var(--paper); color: var(--paper-ink); }
.btn--primary:hover { background: #fff; }
.spin { animation: rc-spin-plain 0.9s linear infinite; }

/* Крутящаяся пластинка «волны» */
.spin-disc { width: 84cqw; margin: 8cqw auto 0; }

/* Ящик с пластинками */
.crate { position: relative; height: 44cqw; margin-top: 8cqw; }
.crate__sleeve {
  position: absolute;
  bottom: 9cqw;
  left: calc(50% - 17cqw + (var(--i) - (var(--n) - 1) / 2) * 10cqw);
  width: 34cqw;
  aspect-ratio: 1;
  overflow: hidden;
  background: var(--obj-deep);
  box-shadow: 0 14px 30px -14px rgba(0, 0, 0, 0.9), 0 0 0 1px rgba(255, 255, 255, 0.06);
  transform: rotate(calc((var(--i) - (var(--n) - 1) / 2) * 5deg));
  transform-origin: 50% 100%;
}
.crate__sleeve img { width: 100%; height: 100%; object-fit: cover; display: block; }
.crate__box { position: absolute; left: 4cqw; right: 4cqw; bottom: 0; z-index: 10; height: 14cqw; border-radius: 1.4cqw; background: var(--obj-deep); box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.12), 0 14px 30px -14px rgba(0, 0, 0, 0.9); }
.crate__box::after { content: ''; position: absolute; left: 50%; top: 4cqw; width: 18cqw; height: 4cqw; border-radius: 99px; background: rgba(0, 0, 0, 0.35); transform: translateX(-50%); }
.s-more .list { margin-top: 4cqw; }
.s-more .list li { padding: 1.8cqw 0; }
.s-more .actions { margin-top: auto; padding-top: 4cqw; }

/* Перфорация плёнки — видна, только пока кадр протягивается */
.holes { position: absolute; top: 0; bottom: 0; z-index: 5; width: 14px; opacity: 0; pointer-events: none; background: #050404 repeating-linear-gradient(to bottom, transparent 0 7px, rgba(243, 236, 223, 0.5) 7px 17px, transparent 17px 24px) 3px 0 / 8px 100% no-repeat; }
.holes--l { left: 0; }
.holes--r { right: 0; }

/* Переход: плёнка протягивается на кадр вверх и встаёт в рамку с лёгким довсплеском */
.rc-enter-active { z-index: 2; animation: rc-pull-in 0.8s var(--ease-io-rc) both; }
.rc-leave-active { z-index: 1; animation: rc-pull-out 0.8s var(--ease-io-rc) both; pointer-events: none; }
.is-back .rc-enter-active { animation-name: rc-pull-in-back; }
.is-back .rc-leave-active { animation-name: rc-pull-out-back; }
.rc-enter-active .holes, .rc-leave-active .holes { animation: rc-holes 0.8s ease both; }
@keyframes rc-pull-in { 0% { transform: translateY(100%); } 82% { transform: translateY(-1.2%); } 100% { transform: none; } }
@keyframes rc-pull-out { 0% { transform: none; } 82% { transform: translateY(-101.2%); } 100% { transform: translateY(-100%); } }
@keyframes rc-pull-in-back { 0% { transform: translateY(-100%); } 82% { transform: translateY(1.2%); } 100% { transform: none; } }
@keyframes rc-pull-out-back { 0% { transform: none; } 82% { transform: translateY(101.2%); } 100% { transform: translateY(100%); } }
@keyframes rc-holes { 0%, 100% { opacity: 0; } 15%, 75% { opacity: 1; } }

.c-in { animation: rc-rise 0.7s var(--ease-out-rc) both; animation-delay: calc(650ms + var(--d, 0) * 90ms); }
@keyframes rc-rise { from { opacity: 0; transform: translateY(12px); } to { opacity: 1; transform: none; } }

/* Предметы живут: пластинки крутятся, катушки мотают, значок и фото падают на стол */
.sleeve .disc { animation: rc-slide-out 1.4s var(--ease-out-rc) both 0.7s, rc-spin 6s linear infinite 2.1s; }
@keyframes rc-slide-out { from { transform: translateX(0) rotate(-40deg); } to { transform: translateX(34%) rotate(0); } }
@keyframes rc-spin { from { transform: translateX(34%) rotate(0); } to { transform: translateX(34%) rotate(360deg); } }
.turn .disc, .spin-disc .disc { animation: rc-spin-plain 1.8s linear infinite; }
.spin-disc .disc { animation-duration: 3.2s; }
@keyframes rc-spin-plain { to { transform: rotate(360deg); } }
.arm { animation: rc-arm 1.2s var(--ease-out-rc) both 0.8s; }
@keyframes rc-arm { from { transform: rotate(-28deg); } to { transform: rotate(0); } }
.reel { animation: rc-spin-plain 2.4s linear infinite; }
.reel--full { animation-duration: 3.6s; }
.dg__strip { animation: rc-roll 1.8s cubic-bezier(0.3, 0.6, 0.2, 1) both; animation-delay: calc(900ms + var(--d) * 110ms); }
@keyframes rc-roll { from { transform: translateY(0); } to { transform: translateY(calc(var(--n) * -1.18em)); } }
.vu__col i, .meters__bar i { animation: rc-led 0.2s steps(1) both; animation-delay: calc(1100ms + var(--k) * 70ms); }
.meters__bar i { animation-delay: calc(1000ms + var(--k) * 40ms); }
@keyframes rc-led { from { opacity: 0.15; } to { opacity: 1; } }
.pola--1 { animation: rc-drop 0.9s cubic-bezier(0.3, 1.4, 0.5, 1) both 0.75s; }
.pola--2 { animation: rc-drop2 0.9s cubic-bezier(0.3, 1.4, 0.5, 1) both 0.6s; }
.polas--one .pola--1 { animation-name: rc-drop-one; }
@keyframes rc-drop { from { transform: translateY(-30%) rotate(-14deg); opacity: 0; } to { transform: rotate(-2deg); opacity: 1; } }
@keyframes rc-drop2 { from { transform: translateY(-24%) rotate(4deg) translate(-7%, 1%); opacity: 0; } to { transform: rotate(-8deg) translate(-7%, 1%); opacity: 1; } }
@keyframes rc-drop-one { from { transform: translateY(-30%) rotate(-10deg); opacity: 0; } to { transform: rotate(3deg); opacity: 1; } }
.pin { animation: rc-pin 1s cubic-bezier(0.3, 1.5, 0.5, 1) both 0.75s; }
@keyframes rc-pin { from { transform: scale(0.6) rotate(-40deg); opacity: 0; } to { transform: none; opacity: 1; } }
.receipt { animation: rc-print 1.8s steps(14) both 0.8s; }
@keyframes rc-print { from { transform: translateY(-100%); } to { transform: none; } }
.pass { animation: rc-slide-in 0.9s cubic-bezier(0.3, 1.3, 0.5, 1) both 0.7s; }
@keyframes rc-slide-in { from { transform: translateY(-20%) rotate(-8deg); opacity: 0; } to { transform: rotate(-2deg); opacity: 1; } }
.stamp { animation: rc-stamp 0.45s cubic-bezier(0.3, 1.6, 0.5, 1) both 1.5s; }
@keyframes rc-stamp { from { transform: scale(1.6) rotate(-4deg); opacity: 0; } to { transform: rotate(-14deg); opacity: 0.92; } }
.jcard { animation: rc-slide-in-j 0.9s cubic-bezier(0.3, 1.3, 0.5, 1) both 0.7s; }
@keyframes rc-slide-in-j { from { transform: translateY(-16%) rotate(-5deg); opacity: 0; } to { transform: rotate(-1deg); opacity: 1; } }
.jcard__list li { animation: rc-rise 0.5s var(--ease-out-rc) both; animation-delay: calc(1100ms + var(--d) * 90ms); }
.fan__sleeve { animation: rc-fan 0.8s var(--ease-out-rc) both; animation-delay: calc(600ms + (4 - var(--i)) * 90ms); }
@keyframes rc-fan { from { transform: translateY(14%) rotate(0); opacity: 0; } to { transform: rotate(calc(var(--i) * 3deg)); opacity: 1; } }
.fan__disc { animation: rc-fan-disc 1.2s var(--ease-out-rc) both 1.1s; }
@keyframes rc-fan-disc { from { transform: translateX(-30%) rotate(-60deg); } to { transform: none; } }
.tuner__needle { animation: rc-needle 1.4s cubic-bezier(0.3, 1.2, 0.4, 1) both 0.9s; }
@keyframes rc-needle { from { transform: translateX(60cqw); } to { transform: none; } }
.strip { animation: rc-strip 1s var(--ease-out-rc) both 0.7s; }
@keyframes rc-strip { from { transform: translateY(-40%) rotate(-3deg); clip-path: inset(0 0 100% 0); } to { transform: rotate(-3deg); clip-path: inset(0); } }
.crate__sleeve { animation: rc-crate 0.7s var(--ease-out-rc) both; animation-delay: calc(700ms + var(--i) * 80ms); }
@keyframes rc-crate { from { transform: translateY(40%) rotate(0); } to { transform: rotate(calc((var(--i) - (var(--n) - 1) / 2) * 5deg)); } }

@media (prefers-reduced-motion: reduce) {
  .rc *, .rc *::before, .rc *::after, .rc-enter-active, .rc-leave-active {
    animation-duration: 1ms !important;
    animation-delay: 0ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 1ms !important;
  }
}
</style>
