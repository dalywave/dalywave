<script setup lang="ts">
import type { Rewind, RewindMore } from '~/api/types'
import { renderSpreadCard } from '~/lib/rewind/share-spread'
import { slideColors } from '~/utils/rewind-colors'
import type { RewindSlide } from '~/utils/rewind-slides'

// «Итоги года» в стиле «Разворот»: история как журнальный постер. Плоское поле цвета обложки,
// огромные цифры, кадрированные обложки и фото, волоски-разделители с подписями.
// Переход — следующая страница открывается шторкой, строки выезжают из-под маски (design/rewind/b.*)
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
const colors = computed(() => slideColors(slide.value.palette, slide.value.shift))
const vars = computed(() => ({ '--bg': colors.value.bg, '--ink': colors.value.ink, '--soft': colors.value.soft, '--acc': colors.value.accent }))
/** Номер страницы в подписи: «02» */
const pageNo = computed(() => String(props.index + 1).padStart(2, '0'))
const pageName = computed(() => t(`page.${kind.value}`, { year: props.r.year }))

/** Короткий текст на две строки поровну — каждая выезжает из-под своей маски */
function halves(text: string): string[] {
  const words = text.split(' ')
  if (words.length < 3) return [text]
  let best = 1
  let diff = Infinity
  for (let i = 1; i < words.length; i++) {
    const d = Math.abs(words.slice(0, i).join(' ').length - words.slice(i).join(' ').length)
    if (d < diff) {
      diff = d
      best = i
    }
  }
  return [words.slice(0, best).join(' '), words.slice(best).join(' ')]
}

const yearHalves = computed(() => [String(props.r.year).slice(0, 2), String(props.r.year).slice(2)])
const tracks = computed(() => props.r.topTracks)
const artists = computed(() => props.r.topArtists)
const albums = computed(() => props.r.topAlbums)
const toy = computed(() => props.r.trackOfYear)
const stripCovers = computed(() => {
  const fromTracks = tracks.value.map(t => x.cover(t.track, 'medium'))
  const fromAlbums = albums.value.map(a => x.img(a.album.image, 'medium'))
  return [...new Set([...fromTracks, ...fromAlbums].filter((u): u is string => !!u))].slice(0, 5)
})

// Месяцы
const maxMonth = computed(() => Math.max(1, ...props.r.months.map(m => m.minutes)))
const peak = computed(() => x.peakMonth(props.r))

// Звучание
const genreTotal = computed(() => props.r.genres.reduce((a, g) => a + g.share, 0) || 1)
const maxGenre = computed(() => Math.max(0.01, ...props.r.genres.map(g => g.share)))

// Характер: часы, когда слушают больше всего, — полосы на линейке суток
const persona = computed(() => props.r.personality)
const bands = computed(() => {
  const h = persona.value.hours
  const max = Math.max(0, ...h)
  if (!max) return []
  const out: { from: number, to: number }[] = []
  h.forEach((v, i) => {
    if (v < max * 0.6) return
    const last = out.at(-1)
    if (last && last.to === i) last.to = i + 1
    else out.push({ from: i, to: i + 1 })
  })
  return out
})

const moreItems = computed(() => props.more?.items.slice(0, 5) ?? [])

// Большие числа досчитывают
const counter = useRewindCount()
watch(() => [props.index, kind.value] as const, () => {
  if (kind.value === 'minutes') counter.start(props.r.minutes)
  else if (kind.value === 'toy') counter.start(toy.value?.plays ?? 0)
  else if (kind.value === 'days') counter.start(props.r.days)
  else if (kind.value === 'discoveries') counter.start(props.r.newArtists)
}, { immediate: true })

async function shareCard(): Promise<Blob> {
  const sum = props.slides.find(s => s.kind === 'summary') ?? slide.value
  return renderSpreadCard({
    year: props.r.year,
    colors: slideColors(sum.palette, sum.shift),
    brand: t('brand'),
    title: props.r.complete ? t('rewindYear', { year: props.r.year }) : t('soFar'),
    covers: stripCovers.value,
    artistsLabel: t('sumArtists'),
    artists: artists.value.map(a => a.artist.name),
    tracksLabel: t('sumTracks'),
    tracks: tracks.value.map(tr => tr.track.title),
    minutes: x.num(props.r.minutes),
    minutesUnit: x.word('minutes', props.r.minutes),
    facts: [
      ...(props.r.genres[0] ? [{ label: t('sumGenre'), value: props.r.genres[0].name }] : []),
      { label: t('sumPersona'), value: x.personaTitle(props.r) },
    ],
  })
}
defineExpose({ shareCard })
</script>

<template>
  <div class="sp" :class="{ 'is-back': dir < 0 }">
    <Transition name="sp-page">
      <section :key="index" class="pg" :class="`pg--${kind}`" :style="vars">
        <!-- Начало: огромный год и полоса обложек -->
        <template v-if="kind === 'intro'">
          <div class="rule"><span>{{ t('brand') }}</span><span>{{ r.complete ? t('rewind') : t('soFar') }}</span></div>
          <h1 class="year">
            <span v-for="(h, i) in yearHalves" :key="h" class="ln"><span :style="{ '--d': i + 1 }">{{ h }}</span></span>
          </h1>
          <p class="lead">
            <span v-for="(l, i) in halves(x.intro(r))" :key="l" class="ln"><span :style="{ '--d': i + 3 }">{{ l }}</span></span>
          </p>
          <div v-if="stripCovers.length" class="strip" aria-hidden="true">
            <i v-for="(u, i) in stripCovers" :key="u" :style="{ '--d': i }"><img :src="u" alt=""></i>
          </div>
        </template>

        <!-- Минуты и столбцы месяцев -->
        <template v-else-if="kind === 'minutes'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <div class="num tnum"><span class="ln"><span style="--d: 1">{{ x.num(counter.value.value) }}</span></span></div>
          <p class="cap"><span class="ln"><span style="--d: 2">{{ x.word('minutesOfMusic', r.minutes) }}</span></span></p>
          <p class="note"><span class="ln"><span style="--d: 3">{{ t('nonstop', { time: x.nonstop(r.minutes) }) }}</span></span></p>
          <div class="bars" role="img" :aria-label="t('monthsLabel')">
            <div v-for="m in r.months" :key="m.month" class="col" :class="{ 'is-peak': m.month === peak.month && m.minutes > 0 }" :style="{ '--h': (m.minutes / maxMonth).toFixed(3), '--d': m.month - 1 }">
              <em v-if="m.month === peak.month && m.minutes > 0" class="tnum">{{ x.num(m.minutes) }}</em>
              <i />
              <span>{{ x.monthLetter(m.month) }}</span>
            </div>
          </div>
        </template>

        <!-- Ритм: дни с музыкой, серия, самый музыкальный день -->
        <template v-else-if="kind === 'days'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <div class="num tnum"><span class="ln"><span style="--d: 1">{{ x.num(counter.value.value) }}</span></span></div>
          <p class="cap"><span class="ln"><span style="--d: 2">{{ x.word('daysWithMusic', r.days) }}</span></span></p>
          <p class="note"><span class="ln"><span style="--d: 3">{{ t('ofDays', { n: x.num(r.daysInRange) }) }}</span></span></p>
          <div v-if="r.topDay && x.cover(r.topDay.track)" class="crop crop--days"><img :src="x.cover(r.topDay.track)!" alt=""></div>
          <ul class="list list--facts">
            <li v-if="r.streak && r.streak.days > 1" style="--d: 3">
              <span>{{ x.count('inARow', r.streak.days) }}</span>
              <em class="tnum">{{ x.dayShort(r.streak.from) }} — {{ x.dayShort(r.streak.to) }}</em>
            </li>
            <li v-if="r.topDay" style="--d: 4">
              <span>{{ t('topDayShort') }}</span>
              <em class="tnum">{{ x.day(r.topDay.date) }} · {{ x.num(r.topDay.minutes) }}&nbsp;{{ t('minShort') }}</em>
            </li>
            <li v-if="r.topDay?.track && r.topDay.trackPlays > 1" style="--d: 5">
              <span class="ellipsis">{{ r.topDay.track.title }}</span>
              <em class="tnum">{{ x.count('times', r.topDay.trackPlays) }}</em>
            </li>
          </ul>
        </template>

        <!-- Первый трек года -->
        <template v-else-if="kind === 'first' && r.firstTrack">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <p class="cap cap--top"><span class="ln"><span style="--d: 1">{{ t('firstKicker', { year: r.year }) }}</span></span></p>
          <figure class="photo photo--square">
            <img v-if="x.cover(r.firstTrack.track)" :src="x.cover(r.firstTrack.track)!" alt="">
            <figcaption class="tnum">{{ x.dayShort(r.firstTrack.at) }}</figcaption>
          </figure>
          <h2 class="name"><span class="ln"><span style="--d: 2">{{ r.firstTrack.track.title }}</span></span></h2>
          <div class="facts">
            <span class="ellipsis">{{ x.artistsOf(r.firstTrack.track) }}</span>
            <span class="tnum">{{ t('firstAt', { day: x.day(r.firstTrack.at), time: x.timeOf(r.firstTrack.at) }) }}</span>
          </div>
        </template>

        <!-- Треки года: полоса обложек и чарт -->
        <template v-else-if="kind === 'tracks'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <div class="strip strip--top" aria-hidden="true">
            <i v-for="(tr, i) in tracks" :key="tr.track.id" :style="{ '--d': i }">
              <img v-if="x.cover(tr.track, 'medium')" :src="x.cover(tr.track, 'medium')!" alt="">
            </i>
          </div>
          <h2 class="statement statement--head">
            <span v-for="(l, i) in t('topTracks').split(' ')" :key="l" class="ln"><span :style="{ '--d': i + 1 }">{{ l }}</span></span>
          </h2>
          <ol class="list list--chart">
            <li v-for="(tr, i) in tracks" :key="tr.track.id" :class="{ 'is-first': i === 0 }" :style="{ '--d': i + 2 }">
              <b class="tnum">{{ i + 1 }}</b>
              <span class="body">
                <span class="title">{{ tr.track.title }}</span>
                <span class="sub ellipsis">{{ x.artistsOf(tr.track) }}</span>
              </span>
              <em class="tnum">{{ x.num(tr.plays) }}</em>
            </li>
          </ol>
        </template>

        <!-- Трек года: огромное «сколько раз» и обложка на всю ширину -->
        <template v-else-if="kind === 'toy' && toy">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <div class="times tnum">
            <span class="ln"><span style="--d: 1">{{ x.num(counter.value.value) }}</span></span>
            <span class="times__u"><span class="ln"><span style="--d: 2">{{ x.word('times', toy.plays) }}</span></span></span>
          </div>
          <div class="crop"><img v-if="x.cover(toy.track)" :src="x.cover(toy.track)!" alt=""></div>
          <div class="track">
            <h2>
              <span v-for="(l, i) in halves(toy.track.title)" :key="l" class="ln"><span :style="{ '--d': i + 3 }">{{ l }}</span></span>
            </h2>
            <p><span class="ln"><span style="--d: 5">{{ x.artistsOf(toy.track) }}</span></span></p>
            <div class="facts">
              <span>{{ t('since', { day: x.day(toy.firstPlayed) }) }}</span>
              <span>{{ t('mostOftenIn', { month: x.monthIn(toy.peakMonth) }) }}</span>
            </div>
          </div>
        </template>

        <!-- Артист года: фото с «№1» и остальные четверо -->
        <template v-else-if="kind === 'artists' && artists[0]">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <figure class="photo">
            <img v-if="x.img(artists[0].artist.image)" :src="x.img(artists[0].artist.image)!" alt="">
            <figcaption>{{ t('number1') }}</figcaption>
          </figure>
          <h2 class="name"><span class="ln"><span style="--d: 1">{{ artists[0].artist.name }}</span></span></h2>
          <div class="facts">
            <span class="tnum">{{ x.count('minutes', artists[0].minutes) }}</span>
            <span class="tnum">{{ x.count('plays', artists[0].plays) }}</span>
          </div>
          <ol class="list">
            <li v-for="(a, i) in artists.slice(1)" :key="a.artist.id" :style="{ '--d': i + 3 }">
              <b class="tnum">{{ i + 2 }}</b>
              <span class="ellipsis">{{ a.artist.name }}</span>
              <em class="tnum">{{ x.num(a.minutes) }}</em>
            </li>
          </ol>
        </template>

        <!-- Альбомы года -->
        <template v-else-if="kind === 'albums' && albums[0]">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <figure class="photo photo--square">
            <img v-if="x.img(albums[0].album.image)" :src="x.img(albums[0].album.image)!" alt="">
            <figcaption>{{ t('number1') }}</figcaption>
          </figure>
          <h2 class="name"><span class="ln"><span style="--d: 1">{{ albums[0].album.title }}</span></span></h2>
          <div class="facts">
            <span class="ellipsis">{{ albums[0].album.artist.name }}</span>
            <span class="tnum">{{ x.count('times', albums[0].plays) }}</span>
          </div>
          <div v-if="albums.length > 1" class="strip strip--albums" aria-hidden="true">
            <i v-for="(a, i) in albums.slice(1)" :key="a.album.id" :style="{ '--d': i + 2 }">
              <img v-if="x.img(a.album.image, 'medium')" :src="x.img(a.album.image, 'medium')!" alt="">
              <b class="tnum">{{ i + 2 }}</b>
            </i>
          </div>
        </template>

        <!-- Звучание: главный жанр крупно, остальные — шкалами -->
        <template v-else-if="kind === 'sound'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <template v-if="r.genres.length">
            <h2 class="statement statement--top">
              <span v-for="(l, i) in halves(r.genres[0]!.name)" :key="l" class="ln"><span :style="{ '--d': i + 1 }">{{ l }}</span></span>
            </h2>
            <p class="note"><span class="ln"><span style="--d: 3">{{ t('spread.shareOfTime', { pct: x.pct(r.genres[0]!.share / genreTotal) }) }}</span></span></p>
            <ul class="meters">
              <li v-for="(g, i) in r.genres.slice(0, 5)" :key="g.id" :style="{ '--d': i + 3, '--w': (g.share / maxGenre).toFixed(3) }">
                <span class="ellipsis">{{ g.name }}</span>
                <em class="tnum">{{ x.pct(g.share / genreTotal) }}</em>
                <i />
              </li>
            </ul>
            <p v-if="r.moods.length" class="moods">
              <template v-for="(m, i) in r.moods.slice(0, 3)" :key="m.id"><template v-if="i"> · </template>{{ t(`mood.${m.id}`) }}&nbsp;<b class="tnum">{{ x.pct(m.score) }}</b></template>
            </p>
          </template>
          <template v-else>
            <h2 class="statement">
              <span v-for="(l, i) in halves(t('soundEmpty'))" :key="l" class="ln"><span :style="{ '--d': i + 1 }">{{ l }}</span></span>
            </h2>
            <p class="note"><span class="ln"><span style="--d: 3">{{ t('soundEmptyText') }}</span></span></p>
          </template>
        </template>

        <!-- Открытия года -->
        <template v-else-if="kind === 'discoveries'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <template v-if="r.newArtists > 0">
            <div class="num tnum"><span class="ln"><span style="--d: 1">{{ x.num(counter.value.value) }}</span></span></div>
            <p class="cap"><span class="ln"><span style="--d: 2">{{ x.word('newArtists', r.newArtists) }}</span></span></p>
            <ol v-if="r.discoveries.length" class="list list--thumbs">
              <li v-for="(d, i) in r.discoveries.slice(0, 5)" :key="d.artist.id" :style="{ '--d': i + 3 }">
                <img v-if="x.img(d.artist.image, 'small')" :src="x.img(d.artist.image, 'small')!" alt="">
                <i v-else />
                <span class="ellipsis">{{ d.artist.name }}</span>
                <em class="tnum">{{ t('discSince', { day: x.dayShort(d.firstHeard) }) }}</em>
              </li>
            </ol>
            <p v-else class="note note--end"><span class="ln"><span style="--d: 3">{{ t('discNoFav') }}</span></span></p>
          </template>
          <template v-else>
            <h2 class="statement">
              <span v-for="(l, i) in halves(t('discEmpty'))" :key="l" class="ln"><span :style="{ '--d': i + 1 }">{{ l }}</span></span>
            </h2>
            <p class="note"><span class="ln"><span style="--d: 3">{{ t('discEmptyText') }}</span></span></p>
          </template>
        </template>

        <!-- Характер: час, когда слушают больше всего, и сутки линейкой -->
        <template v-else-if="kind === 'personality'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <div class="clock tnum"><span class="ln"><span style="--d: 1">{{ x.hour(persona.peakHour) }}</span></span></div>
          <h2 class="statement">
            <span v-for="(l, i) in x.personaTitle(r).split(' ')" :key="l" class="ln"><span :style="{ '--d': i + 2 }">{{ l }}</span></span>
          </h2>
          <p class="note"><span class="ln"><span style="--d: 4">{{ x.personaText(r) }}</span></span></p>
          <div class="day" aria-hidden="true">
            <div class="day__line">
              <i v-for="b in bands" :key="b.from" class="day__band" :style="{ left: `${(b.from / 24) * 100}%`, width: `${((b.to - b.from) / 24) * 100}%` }" />
            </div>
            <div class="day__ticks tnum"><span>00</span><span>06</span><span>12</span><span>18</span><span>24</span></div>
          </div>
        </template>

        <!-- Итог: им же можно поделиться картинкой -->
        <template v-else-if="kind === 'summary'">
          <div class="rule"><span>{{ t('brand') }}</span><span>{{ pageName }}</span></div>
          <div class="sum">
            <div>
              <h3>{{ t('sumArtists') }}</h3>
              <ol>
                <li v-for="(a, i) in artists" :key="a.artist.id" :style="{ '--d': i }"><b class="tnum">{{ i + 1 }}</b><span>{{ a.artist.name }}</span></li>
              </ol>
            </div>
            <div>
              <h3>{{ t('sumTracks') }}</h3>
              <ol>
                <li v-for="(tr, i) in tracks" :key="tr.track.id" :style="{ '--d': i + 1 }"><b class="tnum">{{ i + 1 }}</b><span>{{ tr.track.title }}</span></li>
              </ol>
            </div>
          </div>
          <div class="sum__big tnum">
            <span class="ln"><span style="--d: 2">{{ x.num(r.minutes) }}</span></span>
            <span>{{ x.word('minutes', r.minutes) }}</span>
          </div>
          <dl class="sum__facts">
            <div v-if="r.genres[0]"><dt>{{ t('sumGenre') }}</dt><dd class="ellipsis">{{ r.genres[0].name }}</dd></div>
            <div><dt>{{ t('sumPersona') }}</dt><dd class="ellipsis">{{ x.personaTitle(r) }}</dd></div>
          </dl>
          <div class="actions" @pointerdown.stop @pointerup.stop @click.stop>
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
          <div class="rule"><span>{{ t('brand') }}</span><span>{{ pageName }}</span></div>
          <div v-if="stripCovers.length" class="strip strip--band" aria-hidden="true">
            <i v-for="(u, i) in stripCovers" :key="u" :style="{ '--d': i }"><img :src="u" alt=""></i>
          </div>
          <h2 class="statement">
            <span v-for="(l, i) in halves(t('waveTitle', { year: r.year }))" :key="l" class="ln"><span :style="{ '--d': i + 1 }">{{ l }}</span></span>
          </h2>
          <p class="note"><span class="ln"><span style="--d: 3">{{ t('waveText', { year: r.year }) }}</span></span></p>
          <div class="actions actions--one" @pointerdown.stop @pointerup.stop @click.stop>
            <button type="button" class="btn btn--primary" @click="emit('wave')"><Icon name="ph:play-fill" />{{ t('waveStart') }}</button>
          </div>
        </template>

        <!-- Ещё послушать -->
        <template v-else-if="kind === 'more'">
          <div class="rule"><span>{{ pageNo }}</span><span>{{ pageName }}</span></div>
          <h2 class="name name--head">
            <span v-for="(l, i) in halves(t('moreTitle'))" :key="l" class="ln"><span :style="{ '--d': i + 1 }">{{ l }}</span></span>
          </h2>
          <ol v-if="moreItems.length" class="list list--thumbs list--more">
            <li v-for="(tr, i) in moreItems" :key="tr.id" :style="{ '--d': i + 3 }">
              <img v-if="x.cover(tr, 'small')" :src="x.cover(tr, 'small')!" alt="">
              <i v-else />
              <span class="body">
                <span class="ellipsis">{{ tr.title }}</span>
                <small class="ellipsis">{{ x.artistsOf(tr) }}</small>
              </span>
            </li>
          </ol>
          <p v-else class="note note--end"><span class="ln"><span style="--d: 3">{{ more ? t('moreEmpty') : t('moreLoading') }}</span></span></p>
          <div v-if="moreItems.length" class="actions" @pointerdown.stop @pointerup.stop @click.stop>
            <button type="button" class="btn btn--primary" @click="emit('playMore')"><Icon name="ph:play-fill" />{{ t('morePlay') }}</button>
            <button type="button" class="btn" :disabled="busy === 'more'" @click="emit('saveMore')">
              <Icon :name="busy === 'more' ? 'ph:circle-notch' : more?.playlistId ? 'ph:playlist' : 'ph:plus'" :class="{ spin: busy === 'more' }" />{{ more?.playlistId ? t('openPlaylist') : t('savePlaylist') }}
            </button>
          </div>
        </template>

        <!-- Год без прослушиваний -->
        <template v-else>
          <div class="rule"><span>{{ t('brand') }}</span><span>{{ t('rewind') }}</span></div>
          <h1 class="year">
            <span v-for="(h, i) in yearHalves" :key="h" class="ln"><span :style="{ '--d': i + 1 }">{{ h }}</span></span>
          </h1>
          <h2 class="statement statement--after"><span class="ln"><span style="--d: 3">{{ t('emptyTitle') }}</span></span></h2>
          <p class="note"><span class="ln"><span style="--d: 4">{{ t('emptyText', { year: r.year }) }}</span></span></p>
        </template>
      </section>
    </Transition>
  </div>
</template>

<style scoped>
.sp {
  --e-out: cubic-bezier(0.2, 0.7, 0.2, 1);
  --e-io: cubic-bezier(0.65, 0, 0.35, 1);
  position: absolute;
  inset: 0;
  z-index: 1;
  isolation: isolate;
  overflow: hidden;
  background: #111;
  font-family: var(--font);
}

.pg {
  --pad: 6.2cqw;
  --line: color-mix(in srgb, var(--ink) 28%, transparent);
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  padding: calc(max(10px, var(--rw-top, 0px)) + 64px) var(--pad) max(28px, var(--rw-bottom, 0px));
  overflow: hidden;
  background: var(--bg);
  color: var(--ink);
}
.ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
.tnum { font-variant-numeric: tabular-nums; }

/* Строки из-под маски и волоски с подписями */
.ln { display: block; overflow: hidden; padding-bottom: 0.06em; margin-bottom: -0.06em; }
.ln > span { display: block; }
.rule {
  position: relative;
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding-bottom: 2.6cqw;
  border-bottom: 1px solid var(--line);
  font-size: max(12px, 3.1cqw);
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--soft);
}

/* Начало */
.year { margin: auto 0 0 -0.04em; font-size: 66cqw; line-height: 0.76; font-weight: 800; letter-spacing: -0.075em; }
.lead { margin: 6cqw 0 0; font-size: max(20px, 6.4cqw); line-height: 1.12; font-weight: 700; letter-spacing: -0.02em; }
.strip { display: grid; grid-template-columns: repeat(5, 1fr); gap: 1.6cqw; margin-top: 7cqw; }
.strip i { position: relative; display: block; aspect-ratio: 3 / 4; overflow: hidden; background: color-mix(in srgb, var(--ink) 12%, transparent); }
.strip img { width: 100%; height: 100%; object-fit: cover; display: block; }
.strip--top { margin-top: 5cqw; }
.strip--band { margin: 6cqw calc(var(--pad) * -1) 0; gap: 0; }
.strip--band i { aspect-ratio: auto; height: 30cqh; }
.strip--albums { grid-template-columns: repeat(4, 1fr); margin-top: auto; }
.strip--albums i { aspect-ratio: 1; }
.strip--albums b { position: absolute; left: 0; bottom: 0; padding: 0.8cqw 1.8cqw 0 0; background: var(--bg); font-size: max(12px, 3.4cqw); font-weight: 750; line-height: 1; }

/* Большие числа */
.num { margin: 10cqw 0 0 -0.03em; font-size: 27cqw; line-height: 0.82; font-weight: 800; letter-spacing: -0.06em; white-space: nowrap; }
.cap { margin: 2cqw 0 0; font-size: max(22px, 7cqw); font-weight: 700; letter-spacing: -0.02em; }
.cap--top { margin-top: 6cqw; font-size: max(20px, 6cqw); }
.note { margin: 3cqw 0 0; max-width: 80cqw; font-size: max(15px, 4.3cqw); line-height: 1.35; font-weight: 500; color: var(--soft); text-wrap: pretty; }
.note--end { margin-top: auto; }

.bars { margin-top: auto; display: grid; grid-template-columns: repeat(12, 1fr); gap: 1.6cqw; align-items: end; height: 40cqh; padding-top: 10cqw; border-bottom: 1px solid var(--line); }
.col { position: relative; display: flex; flex-direction: column; justify-content: flex-end; height: 100%; }
.col i { display: block; height: calc(var(--h) * 100%); min-height: 2px; background: color-mix(in srgb, var(--ink) 34%, transparent); transform-origin: bottom; }
.col.is-peak i { background: var(--ink); }
.col span { position: absolute; left: 0; right: 0; bottom: -7cqw; text-align: center; font-size: max(12px, 3cqw); font-weight: 600; color: var(--soft); }
.col.is-peak span { color: var(--ink); }
.col em { position: absolute; left: 50%; bottom: calc(var(--h) * 100% + 2.4cqw); transform: translateX(-50%); font-style: normal; font-size: max(14px, 4cqw); font-weight: 750; white-space: nowrap; }
.pg--minutes { padding-bottom: calc(max(28px, var(--rw-bottom, 0px)) + 8cqw); }

/* Фото и обложки */
.photo { position: relative; margin: 5cqw 0 0; height: 46cqh; overflow: hidden; flex: none; background: color-mix(in srgb, var(--ink) 12%, transparent); }
.photo--square { height: auto; aspect-ratio: 1; max-height: 44cqh; }
.photo img { width: 100%; height: 100%; object-fit: cover; display: block; }
.photo figcaption { position: absolute; left: 0; bottom: 0; padding: 1.6cqw 3cqw 0 0; background: var(--bg); font-size: 15cqw; line-height: 0.8; font-weight: 800; letter-spacing: -0.06em; }
.pg--first .photo { flex: 1 1 auto; aspect-ratio: auto; max-height: none; min-height: 30cqh; }
.pg--first figcaption { font-size: 9cqw; letter-spacing: -0.04em; }
.name { margin: 4cqw 0 0; font-size: 12.6cqw; line-height: 0.95; font-weight: 800; letter-spacing: -0.045em; overflow-wrap: anywhere; }
.name .ln > span { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.name--head { margin-top: 6cqw; }
.facts { display: flex; gap: 4cqw; margin-top: 3.4cqw; min-width: 0; font-size: max(14px, 3.9cqw); font-weight: 600; }
.facts span + span { padding-left: 4cqw; border-left: 1px solid var(--line); }
.facts span { flex: none; }
.facts span.ellipsis { flex: 0 1 auto; }
.facts b { font-weight: 750; }

/* Списки с волосками */
.list { list-style: none; margin: auto 0 0; padding: 0; }
.list li { display: grid; grid-template-columns: 6cqw minmax(0, 1fr) auto; align-items: baseline; gap: 2cqw; padding: 2.2cqw 0; border-top: 1px solid var(--line); font-size: max(14px, 4cqw); font-weight: 600; }
.list b { font-weight: 600; color: var(--soft); }
.list em { font-style: normal; color: var(--soft); }
.list--facts li { grid-template-columns: minmax(0, 1fr) auto; padding: 3cqw 0; }
.list--chart li { grid-template-columns: 6cqw minmax(0, 1fr) auto; align-items: center; padding: 2.4cqw 0; }
.list--chart .body { display: flex; flex-direction: column; min-width: 0; }
.list--chart .title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 700; }
.list--chart .sub { margin-top: 0.4cqw; font-size: max(12px, 3.3cqw); font-weight: 500; color: var(--soft); }
.list--chart .is-first .title { font-size: max(20px, 6.4cqw); line-height: 1.05; letter-spacing: -0.02em; white-space: normal; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
.list--chart .is-first b { align-self: start; color: var(--ink); font-size: max(20px, 6.4cqw); line-height: 1.05; }
.list--thumbs li { grid-template-columns: 11cqw minmax(0, 1fr) auto; align-items: center; gap: 3cqw; padding: 2cqw 0; }
.list--thumbs img, .list--thumbs li > i { width: 11cqw; aspect-ratio: 1; object-fit: cover; display: block; background: color-mix(in srgb, var(--ink) 14%, transparent); }
.list--more { margin-top: auto; }
.list--more li { grid-template-columns: 11cqw minmax(0, 1fr); }
.list--more .body { display: flex; flex-direction: column; min-width: 0; }
.list--more small { margin-top: 0.4cqw; font-size: max(12px, 3.3cqw); font-weight: 500; color: var(--soft); }

/* Трек года */
.crop { margin: 6cqw calc(var(--pad) * -1) 0; height: 30cqh; overflow: hidden; flex: none; background: color-mix(in srgb, var(--ink) 12%, transparent); }
.crop img { width: 100%; height: 100%; object-fit: cover; object-position: 50% 42%; display: block; }
.crop--days { height: 26cqh; margin-top: 7cqw; }
.times { position: relative; margin: 8cqw 0 0 -0.04em; display: flex; align-items: flex-end; gap: 2cqw; font-size: 46cqw; line-height: 0.8; font-weight: 800; letter-spacing: -0.07em; white-space: nowrap; }
.times__u { font-size: max(22px, 7cqw); letter-spacing: -0.02em; line-height: 1; padding-bottom: 1cqw; }
.track { margin-top: auto; padding-top: 4cqw; border-top: 1px solid var(--line); min-width: 0; }
.track h2 { margin: 0; font-size: 11cqw; line-height: 0.98; font-weight: 800; letter-spacing: -0.045em; overflow-wrap: anywhere; }
.track p { margin: 2.6cqw 0 0; font-size: max(16px, 4.6cqw); font-weight: 600; color: var(--soft); }

/* Звучание */
.statement { margin: auto 0 0; font-size: 16.4cqw; line-height: 0.88; font-weight: 800; letter-spacing: -0.05em; overflow-wrap: anywhere; }
.statement--top { margin-top: 10cqw; }
.statement--head { margin-top: 6cqw; }
.statement--after { margin-top: 8cqw; font-size: 12.6cqw; }
.meters { list-style: none; margin: auto 0 0; padding: 0; }
.meters li { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 3cqw; padding: 2.4cqw 0 3.2cqw; border-top: 1px solid var(--line); font-size: max(14px, 4cqw); font-weight: 600; }
.meters em { font-style: normal; color: var(--soft); }
.meters i { position: absolute; left: 0; bottom: 0; height: 1.2cqw; width: calc(var(--w) * 100%); background: color-mix(in srgb, var(--ink) 34%, transparent); transform-origin: left; }
.meters li:first-child i { background: var(--ink); }
.moods { margin: 4cqw 0 0; font-size: max(14px, 3.9cqw); font-weight: 600; line-height: 1.4; color: var(--soft); animation: sp-fade 0.5s ease both 0.9s; }
.moods b { font-weight: 750; color: var(--ink); }

/* Характер */
.clock { margin: 10cqw 0 0 -0.03em; font-size: 29cqw; line-height: 0.8; font-weight: 800; letter-spacing: -0.06em; color: var(--acc); }
.day { margin-top: 8cqw; }
.day__line { position: relative; height: 3cqw; border-bottom: 1px solid var(--line); }
.day__band { position: absolute; bottom: 0; height: 100%; background: var(--acc); transform-origin: left; }
.day__ticks { display: flex; justify-content: space-between; margin-top: 2cqw; font-size: max(12px, 3cqw); font-weight: 600; color: var(--soft); }

/* Итог */
.sum { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 5cqw; margin-top: 5cqw; }
.sum h3 { margin: 0 0 1cqw; font-size: max(12px, 3.1cqw); font-weight: 600; color: var(--soft); }
.sum ol { list-style: none; margin: 0; padding: 0; }
.sum li { display: grid; grid-template-columns: 5cqw minmax(0, 1fr); align-items: baseline; padding: 3.4cqw 0; border-top: 1px solid var(--line); font-size: max(16px, 4.6cqw); font-weight: 650; }
.sum li b { font-weight: 600; color: var(--soft); }
.sum li span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sum__big { margin-top: auto; display: flex; align-items: baseline; gap: 2.4cqw; font-size: 22cqw; line-height: 0.85; font-weight: 800; letter-spacing: -0.06em; white-space: nowrap; }
.sum__big > span:last-child { font-size: max(18px, 5.4cqw); letter-spacing: -0.01em; }
.sum__facts { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 5cqw; margin: 4cqw 0 0; padding-top: 3cqw; border-top: 1px solid var(--line); }
.sum__facts div { min-width: 0; }
.sum__facts dt { font-size: max(12px, 3.1cqw); font-weight: 600; color: var(--soft); }
.sum__facts dd { margin: 0.6cqw 0 0; font-size: max(15px, 4.3cqw); font-weight: 700; }

/* Кнопки */
.actions { display: grid; grid-template-columns: 1fr 1fr; gap: 3cqw; margin-top: 5cqw; }
.actions--one { grid-template-columns: 1fr; margin-top: 7cqw; }
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-width: 0;
  min-height: 48px;
  padding: 0 16px;
  border: 1.5px solid var(--ink);
  border-radius: 999px;
  background: transparent;
  color: var(--ink);
  font: 650 max(15px, 4cqw) / 1 var(--font);
  white-space: nowrap;
  cursor: pointer;
  transition: background 0.15s, transform 0.1s;
}
.btn :deep(svg) { width: 20px; height: 20px; flex: none; }
.btn:hover { background: color-mix(in srgb, var(--ink) 10%, transparent); }
.btn:active { transform: scale(0.97); }
.btn:disabled { opacity: 0.6; cursor: default; }
.btn:focus-visible { outline: 2px solid var(--ink); outline-offset: 2px; }
.btn--primary { background: var(--ink); color: var(--bg); }
.btn--primary:hover { background: color-mix(in srgb, var(--ink) 88%, var(--bg)); }
.spin { animation: sp-spin 0.9s linear infinite; }
@keyframes sp-spin { to { transform: rotate(360deg); } }

/* Переход: шторка следующей страницы, прежняя отъезжает */
.sp-page-enter-active { z-index: 2; animation: sp-wipe 0.72s var(--e-io) both; }
.sp-page-leave-active { z-index: 1; pointer-events: none; animation: sp-push 0.72s var(--e-io) both; }
.is-back .sp-page-enter-active { animation-name: sp-wipe-back; }
.is-back .sp-page-leave-active { animation-name: sp-push-back; }
@keyframes sp-wipe { from { clip-path: inset(0 0 0 100%); } to { clip-path: inset(0); } }
@keyframes sp-wipe-back { from { clip-path: inset(0 100% 0 0); } to { clip-path: inset(0); } }
@keyframes sp-push { to { transform: translateX(-22%); } }
@keyframes sp-push-back { to { transform: translateX(22%); } }

/* Строки выезжают, волосок прочерчивается, картинки проявляются */
.ln > span { animation: sp-line 0.75s var(--e-out) both; animation-delay: calc(380ms + var(--d, 0) * 80ms); }
@keyframes sp-line { from { transform: translateY(105%); } to { transform: none; } }
.rule { animation: sp-fade 0.5s ease both 0.3s; }
.rule::after { content: ''; position: absolute; left: 0; right: 0; bottom: -1px; height: 1px; background: var(--ink); transform-origin: left; opacity: 0; animation: sp-draw 0.9s var(--e-out) both 0.35s; }
@keyframes sp-draw { from { transform: scaleX(0); opacity: 0.6; } to { transform: scaleX(1); opacity: 0; } }
@keyframes sp-fade { from { opacity: 0; } to { opacity: 1; } }
.strip i { animation: sp-up 0.8s var(--e-out) both; animation-delay: calc(600ms + var(--d, 0) * 70ms); }
@keyframes sp-up { from { transform: translateY(40%); opacity: 0; } to { transform: none; opacity: 1; } }
.col i { animation: sp-grow 0.9s var(--e-out) both; animation-delay: calc(500ms + var(--d, 0) * 45ms); }
.col em { animation: sp-fade 0.4s ease both 1.3s; }
@keyframes sp-grow { from { transform: scaleY(0); } to { transform: none; } }
.photo img, .crop img { animation: sp-zoom 1.4s var(--e-out) both 0.2s; }
@keyframes sp-zoom { from { scale: 1.18; } to { scale: 1; } }
.photo, .crop { animation: sp-reveal 0.9s var(--e-io) both 0.3s; }
@keyframes sp-reveal { from { clip-path: inset(100% 0 0 0); } to { clip-path: inset(0); } }
.list li, .sum li, .meters li { animation: sp-up 0.6s var(--e-out) both; animation-delay: calc(500ms + var(--d, 0) * 60ms); }
.meters i { animation: sp-grow-x 0.9s var(--e-out) both; animation-delay: calc(700ms + var(--d, 0) * 60ms); }
.facts, .sum__facts, .actions, .sum h3, .col span, .day__ticks { animation: sp-fade 0.5s ease both 0.9s; }
.day__band { animation: sp-grow-x 0.9s var(--e-out) both 0.9s; }
@keyframes sp-grow-x { from { transform: scaleX(0); } to { transform: none; } }

@media (prefers-reduced-motion: reduce) {
  .sp *, .sp *::before, .sp *::after, .sp-page-enter-active, .sp-page-leave-active { animation-duration: 1ms !important; animation-delay: 0ms !important; transition-duration: 1ms !important; }
}
</style>
