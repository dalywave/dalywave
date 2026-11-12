<script lang="ts">
import type { Rewind, RewindYears } from '~/api/types'
import type { Bridge } from '~/bridge/types'
import { ApiError } from '~/api/types'
import { recordColors, slideColors } from '~/utils/rewind-colors'

// Две карточки на главной (запуск сверху и плитка ниже) делят один запрос
let shared: { at: number, job: Promise<{ years: RewindYears, r: Rewind | null }> } | null = null
function loadShared(bridge: Bridge) {
  if (!shared || Date.now() - shared.at > 30_000) {
    const job = bridge.rewind.years().then(async years => ({ years, r: years.open ? await bridge.rewind.get(years.current) : null }))
    shared = { at: Date.now(), job }
    job.catch(() => (shared = null))
  }
  return shared.job
}
</script>

<script setup lang="ts">
// Вход в «Итоги года». Итоги — событие сервера: пока оно закрыто, карточки нет нигде.
// На главной сверху (`launch`), пока слушатель не посмотрел истории, — крупная карточка «Твои итоги
// готовы» в стиле его историй; после неё — обычная плитка ниже волны (`home`: только после просмотра).
// В медиатеке — плитка, в «Моём вкусе» — с прошлыми годами
const props = withDefaults(defineProps<{ launch?: boolean, home?: boolean, pastYears?: boolean }>(), { launch: false, home: false, pastYears: false })

const bridge = useBridge()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const years = ref<RewindYears | null>(null)
const r = shallowRef<Rewind | null>(null)

onMounted(async () => {
  try {
    const { years: y, r: data } = await loadShared(bridge)
    // Администратор получает ответ и при закрытых итогах — но карточку видят только при открытых
    if (!y.open) return
    years.value = y
    if (props.launch) remind(y)
    r.value = data
  }
  catch (e) {
    // Закрыто (`not_open`) или не загрузилось — карточки просто нет
    if (!(e instanceof ApiError)) console.warn('rewind card', e)
  }
})

/**
 * Приложение на iPhone: итоги открылись, а слушатель их ещё не видел — одно спокойное уведомление
 * вечером того же дня (разрешение система спросит тут же, один раз). Посмотрел — уведомление снимается
 */
function remind(y: RewindYears) {
  const notify = bridge.notify
  if (!notify) return
  const id = `rewind-${y.current}`
  if (y.seen) return void notify.cancel(id)
  const key = `dw.rewind.reminder.${y.current}`
  try {
    if (localStorage.getItem(key)) return
    localStorage.setItem(key, '1')
  }
  catch {
    return
  }
  // В 19:00, а если до него меньше двух часов — завтра в 19:00
  const at = new Date()
  at.setHours(19, 0, 0, 0)
  if (at.getTime() - Date.now() < 2 * 3600_000) at.setDate(at.getDate() + 1)
  void notify.schedule({ id, title: t('notifyTitle', { year: y.current }), body: t('notifyBody'), atMs: at.getTime() })
}

const palette = computed(() => r.value?.topAlbums.flatMap(a => a.album.palette ?? []) ?? null)
const colors = computed(() => slideColors(palette.value))
const covers = (n: number, size: 'small' | 'medium' = 'medium') => {
  const x = r.value
  if (!x) return []
  const list = [...x.topTracks.map(t => t.track.album?.image?.[size]), ...x.topAlbums.map(a => a.album.image?.[size])]
  return [...new Set(list.filter((u): u is string => !!u))].slice(0, n).map(u => bridge.resolveUrl(u))
}
const meta = computed(() => {
  const x = r.value
  if (!x || x.plays === 0) return t('empty')
  const top = x.topArtists[0]?.artist.name
  return [lang.value === 'en' ? `${formatNumber(x.minutes, 'en')} min` : `${formatNumber(x.minutes)}${NBSP}${plural(x.minutes, ['минута', 'минуты', 'минут'])}`, top].filter(Boolean).join(' · ')
})
const past = computed(() => (years.value?.years ?? []).filter(y => y.year !== years.value?.current && y.minutes > 0))
const href = computed(() => `/rewind?year=${r.value?.year ?? ''}`)

// Карточка запуска
const showLaunch = computed(() => props.launch && !!years.value && !years.value.seen && !!r.value)
const style = computed(() => years.value?.style ?? 'spread')
const record = computed(() => recordColors(palette.value))
const launchVars = computed(() => {
  if (style.value === 'spread') return { '--bg': colors.value.bg, '--ink': colors.value.ink, '--soft': colors.value.soft }
  if (style.value === 'record') return { '--bg': record.value.bg, '--ink': '#f3ecdf', '--soft': 'rgba(243, 236, 223, 0.72)', '--obj': record.value.obj, '--obj-deep': record.value.objDeep }
  return { '--bg': '#030406', '--ink': '#ffffff', '--soft': 'rgba(255, 255, 255, 0.74)' }
})
const yearHalves = computed(() => [String(r.value?.year ?? '').slice(0, 2), String(r.value?.year ?? '').slice(2)])
</script>

<template>
  <!-- Запуск: «Твои итоги готовы» в стиле историй слушателя -->
  <NuxtLink v-if="showLaunch && r" :to="href" class="rwl" :class="`rwl--${style}`" :style="launchVars">
    <RewindLaunchWave v-if="style === 'wave'" class="rwl__wave" :palette="palette" />

    <span class="rwl__text">
      <span v-if="style === 'spread'" class="rwl__rule"><span>DalyWave</span><span>{{ t('kicker') }}</span></span>
      <span v-else class="rwl__kicker">{{ style === 'record' ? t('sideA') : t('kicker') }}</span>
      <span class="rwl__title">{{ t('ready', { year: r.year }) }}</span>
      <span class="rwl__meta ellipsis">{{ meta }}</span>
      <span class="rwl__go">{{ t('watch') }}<Icon name="ph:arrow-right" /></span>
    </span>

    <span v-if="style === 'spread'" class="rwl__poster" aria-hidden="true">
      <span class="rwl__year tnum"><span>{{ yearHalves[0] }}</span><span>{{ yearHalves[1] }}</span></span>
      <span class="rwl__strip"><img v-for="c in covers(4)" :key="c" :src="c" alt="" loading="lazy"></span>
    </span>
    <span v-else-if="style === 'record'" class="rwl__sleeve" aria-hidden="true">
      <span class="rwl__disc"><i class="rwl__grooves" /><span class="rwl__label"><img v-if="covers(2, 'small')[1]" :src="covers(2, 'small')[1]" alt=""></span></span>
      <span class="rwl__cover"><img v-if="covers(1)[0]" :src="covers(1)[0]" alt=""></span>
    </span>
  </NuxtLink>

  <!-- Обычная плитка: итоги открыты и уже посмотрены, или это не главная -->
  <div v-else-if="r && !launch && (!home || years?.seen)" class="rwc" :class="{ 'rwc--home': home }">
    <NuxtLink :to="href" class="rwc__card" :style="{ '--bg': colors.bg, '--ink': colors.ink, '--soft': colors.soft }">
      <span class="rwc__text">
        <span class="rwc__kicker">{{ r.complete ? t('kicker') : t('soFar') }}</span>
        <span class="rwc__year tnum">{{ r.year }}</span>
        <span class="rwc__meta ellipsis">{{ meta }}</span>
      </span>
      <span v-if="covers(3).length" class="rwc__covers" aria-hidden="true">
        <img v-for="(c, i) in covers(3)" :key="c" :src="c" alt="" :style="{ '--i': i }" loading="lazy">
      </span>
      <Icon name="ph:caret-right" class="rwc__go" />
    </NuxtLink>
    <nav v-if="pastYears && past.length" class="rwc__past" :aria-label="t('past')">
      <NuxtLink v-for="y in past" :key="y.year" :to="`/rewind?year=${y.year}`" class="rwc__chip tnum">{{ y.year }}</NuxtLink>
    </nav>
  </div>
</template>

<style scoped>
/* ---------- Карточка запуска ---------- */
.rwl {
  position: relative;
  display: flex;
  align-items: stretch;
  gap: var(--s-5);
  min-height: 232px;
  padding: var(--s-6);
  border-radius: var(--r-4);
  overflow: hidden;
  isolation: isolate;
  container-type: inline-size;
  background: var(--bg);
  color: var(--ink);
  text-decoration: none;
  transition: transform var(--dur-2) var(--ease-out);
}
.rwl:active { transform: scale(0.985); }
.rwl:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.rwl__wave { position: absolute; inset: 0; z-index: -1; }
.rwl__text { position: relative; display: flex; flex: 1; flex-direction: column; min-width: 0; }
.rwl__kicker { font-size: var(--fs-small); font-weight: 600; color: var(--soft); }
.rwl--record .rwl__kicker { font-size: 12px; font-weight: 650; letter-spacing: 0.12em; text-transform: uppercase; }
.rwl__title { margin-top: var(--s-2); max-width: 440px; font-size: clamp(28px, 7cqw, 44px); font-weight: 800; line-height: 1; letter-spacing: -0.035em; text-wrap: balance; }
.rwl__meta { margin-top: var(--s-3); font-size: var(--fs-small); font-weight: 500; color: var(--soft); }
.rwl__go {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
  gap: 8px;
  min-height: 44px;
  margin-top: auto;
  padding: 0 20px;
  border-radius: var(--r-pill);
  background: var(--ink);
  color: var(--bg);
  font-size: 15px;
  font-weight: 650;
}
.rwl--wave .rwl__go { color: #08090c; }
.rwl--spread .rwl__go { background: transparent; color: var(--ink); box-shadow: inset 0 0 0 1.5px var(--ink); }
.rwl--record .rwl__go { background: #f4f0e6; color: #1b1813; }

/* «Разворот»: волосок с подписью, огромный год и полоса обложек */
.rwl__rule { display: flex; justify-content: space-between; gap: 12px; padding-bottom: 8px; border-bottom: 1px solid color-mix(in srgb, var(--ink) 28%, transparent); font-size: 12px; font-weight: 600; letter-spacing: 0.02em; color: var(--soft); }
.rwl--spread .rwl__title { margin-top: var(--s-4); }
.rwl__poster { display: flex; flex-direction: column; justify-content: space-between; width: 38%; max-width: 300px; flex: none; }
.rwl__year { display: flex; flex-direction: column; margin: -6px 0 0 -0.04em; font-size: clamp(64px, 15cqw, 128px); font-weight: 800; line-height: 0.78; letter-spacing: -0.075em; }
.rwl__strip { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; margin-top: var(--s-3); }
.rwl__strip img { width: 100%; aspect-ratio: 3 / 4; object-fit: cover; display: block; }

/* «Пластинка»: конверт, из которого выглядывает пластинка */
.rwl__sleeve { position: relative; align-self: center; width: clamp(120px, 26cqw, 190px); aspect-ratio: 1; margin-right: clamp(36px, 9cqw, 72px); flex: none; }
.rwl__disc { position: absolute; inset: 3%; border-radius: 50%; background: #0c0b0b; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1); transform: translateX(36%); overflow: hidden; }
.rwl__grooves { position: absolute; inset: 0; border-radius: 50%; background: repeating-radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.045) 0 1px, transparent 1px 3.5px); }
.rwl__label { position: absolute; inset: 32%; border-radius: 50%; overflow: hidden; background: var(--obj); }
.rwl__label img { width: 100%; height: 100%; object-fit: cover; display: block; }
.rwl__cover { position: absolute; inset: 0; z-index: 1; overflow: hidden; background: var(--obj-deep); box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06); }
.rwl__cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
.rwl--record .rwl__title { font-size: clamp(26px, 6.4cqw, 40px); }

@container (max-width: 460px) {
  .rwl { padding: var(--s-5); }
  .rwl__poster { width: 34%; }
  .rwl__strip { grid-template-columns: repeat(2, 1fr); }
  .rwl__strip img:nth-child(n + 3) { display: none; }
}
@media (prefers-reduced-motion: reduce) {
  .rwl { transition: none; }
}
/* Телефон: на главной у страницы нет полей — карточка держит свои, как «Моя волна» */
@media (max-width: 699px) {
  .rwl { margin: 0 16px 16px; padding: var(--s-5); }
  .rwc--home { margin: 0 16px; }
}

/* ---------- Обычная плитка ---------- */
.rwc { display: flex; flex-direction: column; gap: var(--s-3); }
.rwc__card {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--s-4);
  min-height: 132px;
  padding: var(--s-5) var(--s-12) var(--s-5) var(--s-5);
  border-radius: var(--r-4);
  overflow: hidden;
  background: var(--bg);
  color: var(--ink);
  text-decoration: none;
  transition: transform var(--dur-2) var(--ease-out);
}
.rwc__card:active { transform: scale(0.985); }
.rwc__card:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.rwc__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.rwc__kicker { font-size: var(--fs-small); font-weight: 700; color: var(--soft); }
.rwc__year { margin-top: 2px; font-size: 52px; font-weight: 850; line-height: 0.95; letter-spacing: -0.05em; }
.rwc__meta { margin-top: 6px; font-size: var(--fs-small); font-weight: 600; color: var(--soft); }
.rwc__covers { position: relative; width: 132px; height: 92px; flex: none; }
.rwc__covers img {
  position: absolute;
  top: calc(var(--i) * 4px);
  left: calc(var(--i) * 30px);
  width: 72px;
  height: 72px;
  object-fit: cover;
  border-radius: var(--r-2);
  transform: rotate(calc((var(--i) - 1) * 6deg));
}
.rwc__go { position: absolute; right: var(--s-4); top: 50%; font-size: 20px; transform: translateY(-50%); }
.rwc__past { display: flex; flex-wrap: wrap; gap: var(--s-2); }
.rwc__chip {
  display: inline-flex;
  align-items: center;
  min-height: 36px;
  padding: 0 var(--s-4);
  border-radius: var(--r-pill);
  background: var(--fill-1);
  color: var(--text-1);
  font-size: var(--fs-small);
  font-weight: 650;
  text-decoration: none;
  transition: background var(--dur-1);
}
.rwc__chip:hover { background: var(--fill-2); }
@container main (max-width: 420px) {
  .rwc__covers { width: 104px; }
  .rwc__covers img { width: 60px; height: 60px; left: calc(var(--i) * 22px); }
  .rwc__year { font-size: 44px; }
}
@media (prefers-reduced-motion: reduce) {
  .rwc__card { transition: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "kicker": "Итоги года",
    "soFar": "Итоги по сей день",
    "sideA": "Сторона А",
    "ready": "Твои итоги {year} готовы",
    "watch": "Смотреть",
    "empty": "Пока тихо — итоги соберутся из прослушиваний",
    "past": "Прошлые годы",
    "notifyTitle": "Итоги {year} готовы",
    "notifyBody": "Минуты, артист и трек года — всё в одной истории"
  },
  "en": {
    "kicker": "Year in review",
    "soFar": "Your year so far",
    "sideA": "Side A",
    "ready": "Your {year} is ready",
    "watch": "Watch",
    "empty": "Quiet so far — your review builds as you listen",
    "past": "Past years",
    "notifyTitle": "Your {year} is ready",
    "notifyBody": "Minutes, artist and track of the year — all in one story"
  }
}
</i18n>
