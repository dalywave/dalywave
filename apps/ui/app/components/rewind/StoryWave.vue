<script setup lang="ts">
import type { Rewind, RewindMore } from '~/api/types'
import { renderWaveShare } from '~/lib/rewind/share-wave'
import { chartValue, RewindWave, type WaveLayout } from '~/lib/rewind/wave'
import { waveColors } from '~/utils/rewind-colors'
import type { RewindSlide, SlideKind } from '~/utils/rewind-slides'

// Стиль «Волна года»: одна живая волна DalyWave на все истории. Между слайдами она не сменяется,
// а перетекает: цвет — из обложки слайда, размах — из энергии; на «Минутах» лента ложится графиком
// по месяцам, на артисте, треке и альбоме — обнимает фото и обложку. Текст всплывает строками.
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

// Раскладка волны по слайдам: наклон, высота гребня, размах, энергия, график, свечение вокруг картинки
const LAYOUT: Record<SlideKind, Partial<WaveLayout>> = {
  intro: { tilt: -0.5, offset: 0.06, amp: 1.15, freq: 2.0, energy: 0.62, chart: 0, boxK: 0, vig: 0.6 },
  minutes: { tilt: 0, offset: 0, amp: 0.5, freq: 1.6, energy: 0.45, chart: 1, boxK: 0, vig: 0.4 },
  days: { tilt: -0.08, offset: -0.12, amp: 0.65, freq: 1.5, energy: 0.38, chart: 0, boxK: 0, vig: 0.5 },
  first: { tilt: -0.24, offset: 0, amp: 0.85, freq: 2.0, energy: 0.5, chart: 0, boxK: 1, vig: 0.6 },
  tracks: { tilt: -0.3, offset: 0.2, amp: 0.85, freq: 2.1, energy: 0.6, chart: 0, boxK: 0, vig: 0.6 },
  toy: { tilt: -0.2, offset: 0, amp: 0.9, freq: 2.0, energy: 0.78, chart: 0, boxK: 1, vig: 0.6 },
  artists: { tilt: -0.14, offset: 0, amp: 0.85, freq: 2.0, energy: 0.55, chart: 0, boxK: 1, vig: 0.6 },
  albums: { tilt: -0.1, offset: 0, amp: 0.8, freq: 1.9, energy: 0.52, chart: 0, boxK: 1, vig: 0.6 },
  sound: { tilt: 0.12, offset: 0.24, amp: 0.75, freq: 1.8, energy: 0.58, chart: 0, boxK: 0, vig: 0.55 },
  discoveries: { tilt: -0.36, offset: 0.16, amp: 1.0, freq: 2.2, energy: 0.66, chart: 0, boxK: 0, vig: 0.6 },
  personality: { tilt: -0.06, offset: -0.3, amp: 0.55, freq: 1.4, energy: 0.2, chart: 0, boxK: 0, vig: 0.7 },
  summary: { tilt: -0.1, offset: 0.27, amp: 0.7, freq: 1.8, energy: 0.5, chart: 0, boxK: 0, vig: 0.5 },
  wave: { tilt: -0.4, offset: 0.02, amp: 1.3, freq: 2.3, energy: 0.95, chart: 0, boxK: 0, vig: 0.5 },
  more: { tilt: -0.12, offset: 0.3, amp: 0.65, freq: 1.7, energy: 0.45, chart: 0, boxK: 0, vig: 0.5 },
  empty: { tilt: -0.3, offset: -0.05, amp: 0.6, freq: 1.6, energy: 0.25, chart: 0, boxK: 0, vig: 0.6 },
}

const slide = computed(() => props.slides[props.index] ?? props.slides[0]!)
const kind = computed<SlideKind>(() => slide.value.kind)
const r = computed(() => props.r)

// ---------- Данные слайдов ----------
const artists = computed(() => r.value.topArtists)
const tracks = computed(() => r.value.topTracks)
const albums = computed(() => r.value.topAlbums)
const toy = computed(() => r.value.trackOfYear)
const yearHead = computed(() => String(r.value.year).slice(0, 2))
const yearTail = computed(() => String(r.value.year).slice(2))
const peak = computed(() => x.peakMonth(r.value))
const months = computed(() => r.value.months.map(m => m.minutes))
const hours = computed(() => r.value.personality.hours)
const maxHour = computed(() => Math.max(0.0001, ...hours.value))
const genres = computed(() => r.value.genres.slice(0, 4))
const genreTotal = computed(() => r.value.genres.reduce((a, g) => a + g.share, 0) || 1)
const maxGenre = computed(() => Math.max(0.0001, ...genres.value.map(g => g.share)))
const moods = computed(() => r.value.moods.slice(0, 3))
const moreItems = computed(() => props.more?.items.slice(0, 5) ?? [])
const { value: minutesShown, start: countMinutes } = useRewindCount(1800, 300)

// ---------- Волна ----------
const root = ref<HTMLElement | null>(null)
const waveHost = ref<HTMLElement | null>(null)
const slideEl = ref<HTMLElement | null>(null)
const size = reactive({ w: 390, h: 844 })
let wave: RewindWave | null = null
let ro: ResizeObserver | null = null

/** График месяцев: лента от 8 до 92 % ширины, высота — от −0,34 до −0,02 в долях высоты */
const CHART_Y: [number, number] = [-0.34, -0.02]
function placeChart() {
  if (!wave) return
  const { w, h } = size
  wave.chartX = [(w * 0.08 - w / 2) / h, (w * 0.92 - w / 2) / h]
  wave.chartY = CHART_Y
}
/** Подпись пика стоит точно на ленте: та же кривая, что в шейдере */
const peakStyle = computed(() => {
  const { w, h } = size
  const max = Math.max(1, ...months.value)
  const norm = months.value.map(v => v / max)
  const i = peak.value.month - 1
  const px = w * 0.08 + w * 0.84 * (i / 11)
  const y = h / 2 - (CHART_Y[0] + chartValue(norm, i) * (CHART_Y[1] - CHART_Y[0])) * h
  // У края подпись сдвигается внутрь, чтобы не упереться в рамку; черта остаётся над точкой
  const ax = i <= 0 ? 22 : i >= 11 ? 78 : 50
  return { 'left': `${px}px`, 'top': `${y - 0.03 * h}px`, '--ax': `${ax}%` }
})

async function show(first: boolean) {
  if (!wave) return
  const k = kind.value
  wave.set(LAYOUT[k], { instant: first })
  wave.setPalette(waveColors(slide.value.palette), { instant: first })
  wave.bpm = k === 'toy' ? 124 : k === 'personality' ? 70 : k === 'wave' ? 128 : 112
  if (k === 'intro' || (first && k !== 'minutes')) wave.start()
  else if (!first) wave.skip()
  if (k === 'minutes') {
    placeChart()
    countMinutes(r.value.minutes)
  }
  await nextTick()
  // Гребень проходит через центр фото или обложки
  const box = slideEl.value?.querySelector<HTMLElement>('[data-box]') ?? null
  wave.setBox(box)
  if (box && root.value) {
    const b = box.getBoundingClientRect()
    const c = root.value.getBoundingClientRect()
    if (c.height) wave.set({ offset: -((b.top + b.height / 2 - c.top) - c.height / 2) / c.height - 0.02 }, { instant: first })
  }
}

onMounted(() => {
  if (!waveHost.value || !root.value) return
  wave = new RewindWave(waveHost.value)
  wave.setMonths(months.value)
  ro = new ResizeObserver(() => {
    size.w = root.value?.clientWidth ?? size.w
    size.h = root.value?.clientHeight ?? size.h
    placeChart()
  })
  ro.observe(root.value)
  size.w = root.value.clientWidth
  size.h = root.value.clientHeight
  void show(true)
})
watch(() => props.index, () => void show(false))
watch(months, v => wave?.setMonths(v))
onBeforeUnmount(() => {
  ro?.disconnect()
  wave?.destroy()
  wave = null
})

// ---------- Картинка «Поделиться» ----------
async function shareCard(): Promise<Blob> {
  const s = r.value
  const sum = props.slides.find(x => x.kind === 'summary') ?? slide.value
  return renderWaveShare({
    year: s.year,
    kicker: s.complete ? t('rewind') : t('soFar'),
    palette: waveColors(sum.palette),
    months: months.value,
    artistsLabel: t('sumArtists'),
    artists: s.topArtists.map(a => a.artist.name),
    tracksLabel: t('sumTracks'),
    tracks: s.topTracks.map(x => x.track.title),
    stats: [
      { label: t('sumMinutes'), value: x.num(s.minutes) },
      ...(s.genres[0] ? [{ label: t('sumGenre'), value: s.genres[0].name }] : []),
      { label: t('sumPersona'), value: x.personaTitle(s) },
    ],
  })
}
defineExpose({ shareCard })
</script>

<template>
  <div ref="root" class="sw">
    <div ref="waveHost" class="sw__wave" aria-hidden="true" />

    <Transition name="sw">
      <section :key="index" ref="slideEl" class="sl" :class="`sl--${kind}`">
        <!-- Начало -->
        <template v-if="kind === 'intro'">
          <p class="kicker in" style="--d: 0">{{ r.complete ? t('rewind') : t('soFar') }}</p>
          <div class="intro__foot">
            <h1 class="year tnum"><span class="in" style="--d: 1">{{ yearHead }}</span><span class="in" style="--d: 2">{{ yearTail }}</span></h1>
            <p class="lead in" style="--d: 3">{{ x.intro(r) }}</p>
          </div>
        </template>

        <!-- Минуты: лента — график по месяцам -->
        <template v-else-if="kind === 'minutes'">
          <p class="kicker in" style="--d: 0">{{ t('minutesKicker') }}</p>
          <div class="big tnum in" style="--d: 1">{{ x.num(minutesShown) }}</div>
          <p class="sub in" style="--d: 2">{{ t('nonstop', { time: x.nonstop(r.minutes) }) }}</p>
          <div v-if="peak.minutes > 0" class="peak in" style="--d: 6" :style="peakStyle">
            <b class="tnum">{{ x.num(peak.minutes) }}</b> {{ x.monthName(peak.month) }}
          </div>
          <div class="months in" style="--d: 4" role="img" :aria-label="t('monthsLabel')">
            <span v-for="m in r.months" :key="m.month" :class="{ 'is-peak': m.month === peak.month && m.minutes > 0 }">{{ x.monthLetter(m.month) }}</span>
          </div>
        </template>

        <!-- Ритм: дни, серия, самый музыкальный день -->
        <template v-else-if="kind === 'days'">
          <p class="kicker in" style="--d: 0">{{ t('rhythm') }}</p>
          <div class="big tnum in" style="--d: 1">{{ x.num(r.days) }}</div>
          <p class="sub in" style="--d: 2">{{ x.word('daysWithMusic', r.days) }} {{ t('ofDays', { n: x.num(r.daysInRange) }) }}</p>
          <div class="foot">
            <dl class="stats stats--two in" style="--d: 4">
              <div v-if="r.streak && r.streak.days > 1">
                <dt>{{ t('wave.streak') }}</dt>
                <dd class="tnum">{{ x.num(r.streak.days) }}</dd>
                <dd class="stats__sub">{{ x.dayShort(r.streak.from) }} — {{ x.dayShort(r.streak.to) }}</dd>
              </div>
              <div v-if="r.topDay">
                <dt>{{ t('wave.topDay') }}</dt>
                <dd>{{ x.day(r.topDay.date) }}</dd>
                <dd class="stats__sub tnum">{{ t('topDayStats', { min: x.num(r.topDay.minutes), plays: formatCount(r.topDay.plays, 'track', x.lang.value) }) }}</dd>
              </div>
            </dl>
            <div v-if="r.topDay?.track && r.topDay.trackPlays > 1" class="row in" style="--d: 5">
              <img v-if="x.cover(r.topDay.track, 'small')" :src="x.cover(r.topDay.track, 'small')!" alt="" class="row__img">
              <span class="row__body">
                <span class="row__title ellipsis">{{ r.topDay.track.title }}</span>
                <span class="row__sub ellipsis">{{ x.artistsOf(r.topDay.track) }} · {{ x.count('times', r.topDay.trackPlays) }}</span>
              </span>
            </div>
          </div>
        </template>

        <!-- Первый трек года -->
        <template v-else-if="kind === 'first' && r.firstTrack">
          <p class="kicker in" style="--d: 0">{{ t('firstKicker', { year: r.year }) }}</p>
          <div class="cover in" style="--d: 1" data-box>
            <img v-if="x.cover(r.firstTrack.track)" :src="x.cover(r.firstTrack.track)!" alt="">
          </div>
          <div class="foot">
            <h2 class="title title--track in" style="--d: 2">{{ r.firstTrack.track.title }}</h2>
            <p class="meta in" style="--d: 3">{{ x.artistsOf(r.firstTrack.track) }}</p>
            <p class="meta meta--date in" style="--d: 4">{{ t('firstAt', { day: x.day(r.firstTrack.at), time: x.timeOf(r.firstTrack.at) }) }}</p>
          </div>
        </template>

        <!-- Топ-5 треков -->
        <template v-else-if="kind === 'tracks'">
          <p class="kicker in" style="--d: 0">{{ t('topTracks') }}</p>
          <div class="foot">
            <ol class="rows">
              <li v-for="(it, i) in tracks" :key="it.track.id" class="row in" :class="{ 'row--top': i === 0 }" :style="{ '--d': 1 + i }">
                <span class="row__n tnum">{{ i + 1 }}</span>
                <img v-if="x.cover(it.track, i === 0 ? 'medium' : 'small')" :src="x.cover(it.track, i === 0 ? 'medium' : 'small')!" alt="" class="row__img">
                <span v-else class="row__img row__img--empty" />
                <span class="row__body">
                  <span class="row__title ellipsis">{{ it.track.title }}</span>
                  <span class="row__sub ellipsis">{{ x.artistsOf(it.track) }}</span>
                </span>
                <span class="row__v tnum">{{ x.num(it.plays) }}</span>
              </li>
            </ol>
          </div>
        </template>

        <!-- Трек года -->
        <template v-else-if="kind === 'toy' && toy">
          <p class="kicker in" style="--d: 0">{{ t('toyKicker') }}</p>
          <div class="cover in" style="--d: 1" data-box>
            <img v-if="x.cover(toy.track)" :src="x.cover(toy.track)!" alt="">
          </div>
          <div class="foot">
            <h2 class="title title--track in" style="--d: 2">{{ toy.track.title }}</h2>
            <p class="meta in" style="--d: 3">{{ x.artistsOf(toy.track) }}</p>
            <dl class="stats in" style="--d: 4">
              <div><dt>{{ t('timesAYear') }}</dt><dd class="tnum">{{ x.num(toy.plays) }}</dd></div>
              <div><dt>{{ t('firstTime') }}</dt><dd>{{ x.day(toy.firstPlayed) }}</dd></div>
              <div><dt>{{ t('mostOften') }}</dt><dd>{{ x.monthIn(toy.peakMonth) }}</dd></div>
            </dl>
          </div>
        </template>

        <!-- Артист года -->
        <template v-else-if="kind === 'artists' && artists[0]">
          <p class="kicker in" style="--d: 0">{{ t('topArtist') }}</p>
          <div class="photo in" style="--d: 1" data-box>
            <img v-if="x.img(artists[0].artist.image)" :src="x.img(artists[0].artist.image)!" alt="">
          </div>
          <div class="foot">
            <h2 class="title in" style="--d: 2">{{ artists[0].artist.name }}</h2>
            <p class="meta tnum in" style="--d: 3">{{ x.count('minutes', artists[0].minutes) }} · {{ x.count('plays', artists[0].plays) }}</p>
            <ol class="list in" style="--d: 4">
              <li v-for="(a, i) in artists.slice(1, 5)" :key="a.artist.id">
                <span class="list__n tnum">{{ i + 2 }}</span>
                <span class="list__l">{{ a.artist.name }}</span>
                <span class="list__v tnum">{{ x.num(a.minutes) }}&nbsp;{{ t('minShort') }}</span>
              </li>
            </ol>
          </div>
        </template>

        <!-- Альбом года и следующие -->
        <template v-else-if="kind === 'albums' && albums[0]">
          <p class="kicker in" style="--d: 0">{{ t('topAlbums') }}</p>
          <div class="cover cover--album in" style="--d: 1" data-box>
            <img v-if="x.img(albums[0].album.image)" :src="x.img(albums[0].album.image)!" alt="">
          </div>
          <div class="foot">
            <h2 class="title title--track in" style="--d: 2">{{ albums[0].album.title }}</h2>
            <p class="meta tnum in" style="--d: 3">{{ albums[0].album.artist.name }} · {{ x.count('times', albums[0].plays) }}</p>
            <ol class="list in" style="--d: 4">
              <li v-for="(a, i) in albums.slice(1, 4)" :key="a.album.id">
                <span class="list__n tnum">{{ i + 2 }}</span>
                <span class="list__l">{{ a.album.title }}</span>
                <span class="list__v ellipsis">{{ a.album.artist.name }}</span>
              </li>
            </ol>
          </div>
        </template>

        <!-- Звучание: жанры и настроение -->
        <template v-else-if="kind === 'sound'">
          <p class="kicker in" style="--d: 0">{{ t('soundKicker') }}</p>
          <div class="foot">
            <template v-if="genres.length">
              <h2 class="ptitle ptitle--genre in" style="--d: 1">{{ genres[0]!.name }}</h2>
              <ul class="bars in" style="--d: 2">
                <li v-for="g in genres" :key="g.id">
                  <span class="bars__name ellipsis">{{ g.name }}</span>
                  <span class="bars__track"><i :style="{ width: `${(g.share / maxGenre) * 100}%` }" /></span>
                  <span class="bars__v tnum">{{ x.pct(g.share / genreTotal) }}</span>
                </li>
              </ul>
              <dl v-if="moods.length" class="stats in" style="--d: 3">
                <div v-for="m in moods" :key="m.id"><dt>{{ t(`mood.${m.id}`) }}</dt><dd class="tnum">{{ x.pct(m.score) }}</dd></div>
              </dl>
            </template>
            <template v-else>
              <h2 class="title in" style="--d: 1">{{ t('soundEmpty') }}</h2>
              <p class="sub in" style="--d: 2">{{ t('soundEmptyText') }}</p>
            </template>
          </div>
        </template>

        <!-- Открытия -->
        <template v-else-if="kind === 'discoveries'">
          <p class="kicker in" style="--d: 0">{{ t('discKicker') }}</p>
          <div class="foot">
            <template v-if="r.newArtists > 0">
              <div class="big tnum in" style="--d: 1">{{ x.num(r.newArtists) }}</div>
              <p class="sub in" style="--d: 2">{{ x.word('newArtists', r.newArtists) }}</p>
              <ol v-if="r.discoveries.length" class="rows rows--disc">
                <li v-for="(d, i) in r.discoveries.slice(0, 4)" :key="d.artist.id" class="row in" :style="{ '--d': 3 + i }">
                  <img v-if="x.img(d.artist.image, 'small')" :src="x.img(d.artist.image, 'small')!" alt="" class="row__img row__img--round">
                  <span v-else class="row__img row__img--round row__img--empty" />
                  <span class="row__body">
                    <span class="row__title ellipsis">{{ d.artist.name }}</span>
                    <span class="row__sub ellipsis">{{ t('discSince', { day: x.dayShort(d.firstHeard) }) }} · {{ x.count('times', d.plays) }}</span>
                  </span>
                </li>
              </ol>
              <p v-else class="sub in" style="--d: 3">{{ t('discNoFav') }}</p>
            </template>
            <template v-else>
              <h2 class="title in" style="--d: 1">{{ t('discEmpty') }}</h2>
              <p class="sub in" style="--d: 2">{{ t('discEmptyText') }}</p>
            </template>
          </div>
        </template>

        <!-- Характер -->
        <template v-else-if="kind === 'personality'">
          <p class="kicker in" style="--d: 0">{{ t('personaKicker') }}</p>
          <h2 class="ptitle in" style="--d: 1">{{ x.personaTitle(r) }}</h2>
          <p class="sub in" style="--d: 2">{{ x.personaText(r) }}. {{ t('peakHour', { time: x.hour(r.personality.peakHour) }) }}</p>
          <div v-if="r.plays > 0" class="hours in" style="--d: 3" aria-hidden="true">
            <div class="hours__bars">
              <i v-for="(h, i) in hours" :key="i" :class="{ 'is-peak': h >= maxHour * 0.8 }" :style="{ '--h': (h / maxHour).toFixed(3) }" />
            </div>
            <div class="hours__axis tnum"><span>00</span><span>06</span><span>12</span><span>18</span><span>24</span></div>
          </div>
        </template>

        <!-- Итог: им же можно поделиться картинкой -->
        <template v-else-if="kind === 'summary'">
          <div class="sum__head">
            <p class="kicker in" style="--d: 0">{{ r.complete ? t('rewindYear', { year: r.year }) : t('soFar') }}</p>
          </div>
          <div class="in" style="--d: 1">
            <div class="sum__cols">
              <div>
                <h3>{{ t('sumArtists') }}</h3>
                <ol><li v-for="a in artists" :key="a.artist.id"><span>{{ a.artist.name }}</span></li></ol>
              </div>
              <div>
                <h3>{{ t('sumTracks') }}</h3>
                <ol><li v-for="it in tracks" :key="it.track.id"><span>{{ it.track.title }}</span></li></ol>
              </div>
            </div>
            <dl class="sum__stats">
              <div><dt>{{ t('sumMinutes') }}</dt><dd class="tnum">{{ x.num(r.minutes) }}</dd></div>
              <div v-if="r.genres[0]"><dt>{{ t('sumGenre') }}</dt><dd>{{ r.genres[0].name }}</dd></div>
              <div><dt>{{ t('sumPersona') }}</dt><dd>{{ x.personaTitle(r) }}</dd></div>
            </dl>
          </div>
          <div class="actions in" style="--d: 3" @pointerdown.stop @pointerup.stop @click.stop>
            <button type="button" class="btn btn--primary" :disabled="busy === 'share'" @click="emit('share')">
              <Icon :name="busy === 'share' ? 'ph:circle-notch' : 'ph:upload-simple'" :class="{ spin: busy === 'share' }" />{{ t('share') }}
            </button>
            <button type="button" class="btn" :disabled="busy === 'playlist'" @click="emit('playlist')">
              <Icon :name="busy === 'playlist' ? 'ph:circle-notch' : r.playlistId ? 'ph:playlist' : 'ph:plus'" :class="{ spin: busy === 'playlist' }" />{{ r.playlistId ? t('openPlaylist') : t('savePlaylist') }}
            </button>
          </div>
        </template>

        <!-- Волна из итогов года -->
        <template v-else-if="kind === 'wave'">
          <p class="kicker in" style="--d: 0">{{ t('nextKicker') }}</p>
          <div class="foot">
            <h2 class="ptitle ptitle--wave in" style="--d: 1">{{ t('waveTitle', { year: r.year }) }}</h2>
            <p class="sub in" style="--d: 2">{{ t('waveText', { year: r.year }) }}</p>
            <div class="actions actions--one in" style="--d: 3" @pointerdown.stop @pointerup.stop @click.stop>
              <button type="button" class="btn btn--primary" @click="emit('wave')">
                <Icon name="ph:play-fill" />{{ t('waveStart') }}
              </button>
            </div>
          </div>
        </template>

        <!-- Ещё послушать -->
        <template v-else-if="kind === 'more'">
          <p class="kicker in" style="--d: 0">{{ t('moreKicker') }}</p>
          <div class="foot">
            <h2 class="title title--track in" style="--d: 1">{{ t('moreTitle') }}</h2>
            <ol v-if="moreItems.length" class="rows rows--more">
              <li v-for="(it, i) in moreItems" :key="it.id" class="row in" :style="{ '--d': 2 + i * 0.5 }">
                <img v-if="x.cover(it, 'small')" :src="x.cover(it, 'small')!" alt="" class="row__img">
                <span v-else class="row__img row__img--empty" />
                <span class="row__body">
                  <span class="row__title ellipsis">{{ it.title }}</span>
                  <span class="row__sub ellipsis">{{ x.artistsOf(it) }}</span>
                </span>
              </li>
            </ol>
            <p v-else class="sub in" style="--d: 2">{{ more ? t('moreEmpty') : t('moreLoading') }}</p>
            <div v-if="moreItems.length" class="actions in" style="--d: 5" @pointerdown.stop @pointerup.stop @click.stop>
              <button type="button" class="btn btn--primary" @click="emit('playMore')">
                <Icon name="ph:play-fill" />{{ t('morePlay') }}
              </button>
              <button type="button" class="btn" :disabled="busy === 'more'" @click="emit('saveMore')">
                <Icon :name="busy === 'more' ? 'ph:circle-notch' : more?.playlistId ? 'ph:playlist' : 'ph:plus'" :class="{ spin: busy === 'more' }" />{{ more?.playlistId ? t('openPlaylist') : t('savePlaylist') }}
              </button>
            </div>
          </div>
        </template>

        <!-- Год без прослушиваний -->
        <template v-else>
          <p class="kicker in" style="--d: 0">{{ t('rewind') }}</p>
          <div class="intro__foot">
            <h1 class="year tnum"><span class="in" style="--d: 1">{{ yearHead }}</span><span class="in" style="--d: 2">{{ yearTail }}</span></h1>
            <h2 class="title in" style="--d: 3">{{ t('emptyTitle') }}</h2>
            <p class="sub in" style="--d: 4">{{ t('emptyText', { year: r.year }) }}</p>
          </div>
        </template>
      </section>
    </Transition>
  </div>
</template>

<style scoped>
.sw {
  --rw-ease-out: cubic-bezier(0.2, 0.7, 0.2, 1);
  --soft: rgba(255, 255, 255, 0.78);
  --muted: rgba(255, 255, 255, 0.64);
  --line: rgba(255, 255, 255, 0.14);
  position: absolute;
  inset: 0;
  z-index: 1;
  isolation: isolate;
  overflow: hidden;
  background: #030406;
  color: #fff;
  font-family: var(--font);
}
.sw__wave { position: absolute; inset: 0; }
.sw__wave :deep(.rw-wave-canvas) { position: absolute; inset: 0; display: block; width: 100%; height: 100%; }

.sl {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  padding: calc(max(10px, var(--rw-top, 0px)) + 64px) 6.2cqw max(28px, var(--rw-bottom, 0px));
}

/* Текст всплывает строками; уходящий слайд тает вверх, волна перетекает сама */
.in { opacity: 0; animation: a-rise 0.8s var(--rw-ease-out) both; animation-delay: calc(240ms + var(--d, 0) * 90ms); }
.photo.in, .cover.in { animation-name: a-scale; }
@keyframes a-rise { from { opacity: 0; transform: translateY(18px); } to { opacity: 1; transform: none; } }
@keyframes a-scale { from { opacity: 0; transform: scale(0.86); } to { opacity: 1; transform: none; } }
.sw-leave-active { animation: a-out 0.4s ease both; pointer-events: none; }
.sw-enter-active { z-index: 2; }
@keyframes a-out { from { opacity: 1; } to { opacity: 0; transform: translateY(-14px); } }

.kicker { margin: 0; font-size: max(13px, 3.6cqw); font-weight: 600; letter-spacing: 0.01em; color: var(--soft); }
.foot { margin-top: auto; min-width: 0; }
.ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }

/* Начало */
.intro__foot { margin-top: auto; }
.year { margin: 0; display: flex; flex-direction: column; font-size: 40cqw; line-height: 0.8; font-weight: 800; letter-spacing: -0.06em; }
.year span { display: block; margin-left: -0.05em; }
.lead { margin: 6cqw 0 0; max-width: 72cqw; font-size: max(17px, 5cqw); line-height: 1.3; font-weight: 500; text-wrap: balance; }

/* Крупные числа */
.big { margin-top: 3cqw; font-size: 24cqw; line-height: 0.9; font-weight: 800; letter-spacing: -0.05em; white-space: nowrap; }
.sub { margin: 4cqw 0 0; max-width: 80cqw; font-size: max(15px, 4.3cqw); line-height: 1.35; font-weight: 500; color: var(--soft); text-wrap: pretty; }

/* Минуты: подписи месяцев и пика на ленте */
.months { position: absolute; left: 8cqw; right: 8cqw; bottom: max(28px, var(--rw-bottom, 0px)); display: flex; justify-content: space-between; }
.months span { width: 0; display: flex; justify-content: center; font-size: max(12px, 3.1cqw); font-weight: 600; color: var(--muted); }
.months span.is-peak { color: #fff; }
.peak { position: absolute; translate: calc(var(--ax, 50%) * -1) calc(-100% - 30px); white-space: nowrap; font-size: max(13px, 3.4cqw); font-weight: 600; color: rgba(255, 255, 255, 0.8); text-align: center; }
.peak b { display: block; font-size: max(20px, 6cqw); font-weight: 750; color: #fff; letter-spacing: -0.02em; }
.peak::after { content: ''; position: absolute; left: var(--ax, 50%); bottom: -24px; width: 1px; height: 16px; background: rgba(255, 255, 255, 0.5); }

/* Фото артиста и обложки: волна обнимает их свечением */
.photo, .cover { align-self: center; margin-top: 5cqh; flex: none; overflow: hidden; }
.photo { width: 58cqw; aspect-ratio: 1; border-radius: 50%; }
.cover { width: 58cqw; aspect-ratio: 1; border-radius: 12px; }
.photo img, .cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
.photo, .cover { background: rgba(255, 255, 255, 0.08); }
.title { margin: 0; font-size: max(30px, 10.5cqw); line-height: 1; font-weight: 800; letter-spacing: -0.035em; text-wrap: balance; overflow-wrap: anywhere; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.title--track { font-size: max(26px, 8.6cqw); line-height: 1.05; }
.meta { margin: 2.6cqw 0 0; font-size: max(15px, 4.2cqw); font-weight: 500; color: rgba(255, 255, 255, 0.74); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta--date { margin-top: 1cqw; }

/* Списки с волосками */
.list { list-style: none; margin: 5cqw 0 0; padding: 0; }
.list li { display: flex; align-items: baseline; gap: 3cqw; padding: 2.6cqw 0; border-top: 1px solid var(--line); font-size: max(15px, 4.2cqw); font-weight: 500; }
.list__n { width: 4cqw; flex: none; color: rgba(255, 255, 255, 0.56); }
.list__l { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.list__v { flex: none; max-width: 36cqw; color: var(--muted); }

/* Строки с картинкой: треки, открытия, «ещё послушать» */
.rows { list-style: none; margin: 0; padding: 0; }
.row { display: flex; align-items: center; gap: 3.4cqw; min-width: 0; padding: 2.4cqw 0; border-top: 1px solid var(--line); }
.rows .row:first-child { border-top: 0; }
.row__n { width: 4cqw; flex: none; font-size: max(14px, 3.9cqw); color: rgba(255, 255, 255, 0.56); }
.row__img { width: 12cqw; aspect-ratio: 1; flex: none; object-fit: cover; border-radius: 6px; }
.row__img--round { border-radius: 50%; }
.row__img--empty { display: block; background: rgba(255, 255, 255, 0.1); }
.row__body { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.row__title { font-size: max(15px, 4.2cqw); font-weight: 600; }
.row__sub { margin-top: 0.4cqw; font-size: max(13px, 3.5cqw); font-weight: 500; color: var(--muted); }
.row__v { flex: none; font-size: max(14px, 3.9cqw); color: var(--muted); }
.row--top { padding-bottom: 3.4cqw; }
.row--top .row__img { width: 24cqw; border-radius: 8px; }
.row--top .row__title { font-size: max(20px, 6cqw); font-weight: 750; letter-spacing: -0.02em; white-space: normal; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; line-height: 1.08; }
.row--top .row__n { align-self: flex-start; padding-top: 1cqw; color: #fff; }
.sl--days .row { margin-top: 5cqw; padding: 4cqw 0 0; border-top: 1px solid var(--line); }
.rows--disc { margin-top: 6cqw; }
.rows--more { margin-top: 4cqw; }

/* Статы через вертикальные волоски */
.stats { display: grid; grid-template-columns: auto auto minmax(0, 1fr); margin: 6cqw 0 0; }
.stats--two { grid-template-columns: minmax(0, auto) minmax(0, 1fr); margin-top: 0; }
.stats > div { min-width: 0; padding: 0 4cqw; border-left: 1px solid rgba(255, 255, 255, 0.16); }
.stats > div:first-child { padding-left: 0; border-left: 0; }
.stats dt { font-size: max(12px, 3.2cqw); color: var(--muted); }
.stats dd { margin: 1cqw 0 0; font-size: max(17px, 5cqw); font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.stats--two dd { font-size: max(20px, 6.4cqw); letter-spacing: -0.02em; }
.stats dd.stats__sub { margin-top: 0.6cqw; font-size: max(13px, 3.5cqw); font-weight: 500; letter-spacing: 0; color: var(--muted); }

/* Звучание */
.ptitle { margin: 5cqh 0 0; font-size: 15cqw; line-height: 0.92; font-weight: 800; letter-spacing: -0.045em; text-wrap: balance; overflow-wrap: anywhere; }
.ptitle--genre { margin: 0; font-size: 13cqw; }
.ptitle--wave { margin: 0; font-size: 13cqw; }
.bars { list-style: none; margin: 6cqw 0 0; padding: 0; }
.bars li { display: grid; grid-template-columns: 30cqw minmax(0, 1fr) 12cqw; align-items: center; gap: 3cqw; padding: 2.4cqw 0; border-top: 1px solid var(--line); font-size: max(14px, 3.9cqw); font-weight: 500; }
.bars__track { display: block; height: 3px; border-radius: 2px; background: rgba(255, 255, 255, 0.14); overflow: hidden; }
.bars__track i { display: block; height: 100%; background: #fff; }
.bars__v { text-align: right; color: var(--muted); }

/* Характер: часы суток */
.hours { margin-top: 9cqw; }
.hours__bars { display: flex; align-items: flex-end; gap: 1.2cqw; height: 18cqw; }
.hours__bars i { flex: 1; height: calc(var(--h) * 100%); min-height: 2px; border-radius: 2px; background: rgba(255, 255, 255, 0.28); }
.hours__bars i.is-peak { background: #fff; }
.hours__axis { display: flex; justify-content: space-between; margin-top: 2cqw; font-size: max(12px, 3cqw); color: rgba(255, 255, 255, 0.56); }

/* Итог */
.sum__head { flex: 1; }
.sum__cols { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 5cqw; }
.sum__cols h3, .sum__stats dt { margin: 0 0 2cqw; font-size: max(12px, 3.2cqw); font-weight: 600; color: var(--muted); }
.sum__cols ol { margin: 0; padding: 0; list-style: none; counter-reset: n; }
.sum__cols li { counter-increment: n; display: flex; gap: 2cqw; padding: 0.9cqw 0; font-size: max(14px, 3.9cqw); font-weight: 600; white-space: nowrap; overflow: hidden; }
.sum__cols li::before { content: counter(n); width: 3cqw; flex: none; color: rgba(255, 255, 255, 0.5); font-weight: 500; font-variant-numeric: tabular-nums; }
.sum__cols li span { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
.sum__stats { display: grid; grid-template-columns: auto minmax(0, auto) minmax(0, 1fr); gap: 5cqw; margin: 5cqw 0 0; padding-top: 4cqw; border-top: 1px solid var(--line); }
.sum__stats > div { min-width: 0; }
.sum__stats dd { margin: 0; font-size: max(15px, 4.3cqw); font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* Кнопки */
.actions { display: grid; grid-template-columns: 1fr 1fr; gap: 3cqw; margin-top: 6cqw; }
.actions--one { grid-template-columns: 1fr; }
.btn { display: inline-flex; align-items: center; justify-content: center; gap: 8px; min-height: 48px; padding: 0 16px; border: 0; border-radius: 999px; background: rgba(255, 255, 255, 0.12); color: #fff; font: 600 max(15px, 4cqw) / 1 var(--font); cursor: pointer; transition: background 0.15s, transform 0.1s; }
.btn :deep(svg) { width: 20px; height: 20px; }
.btn:hover { background: rgba(255, 255, 255, 0.18); }
.btn:active { transform: scale(0.97); }
.btn:focus-visible { outline: 2px solid #fff; outline-offset: 2px; }
.btn:disabled { opacity: 0.6; cursor: default; }
.btn--primary { background: #fff; color: #08090c; }
.btn--primary:hover { background: #e8eaf0; }
.spin { animation: spin 0.9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }

@media (prefers-reduced-motion: reduce) {
  .in, .sw-leave-active { animation-duration: 1ms !important; animation-delay: 0ms !important; }
}
</style>
