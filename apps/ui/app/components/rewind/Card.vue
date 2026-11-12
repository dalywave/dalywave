<script setup lang="ts">
import type { Rewind, RewindYears } from '~/api/types'

// Вход в «Итоги года»: плоская плитка цвета обложек года, крупный год и обложки топ-альбомов.
// На главной — только в сезон (с 1 декабря), в «Моём вкусе» — всегда, с прошлыми годами рядом
const props = withDefaults(defineProps<{ seasonOnly?: boolean, pastYears?: boolean }>(), { seasonOnly: false, pastYears: false })

const bridge = useBridge()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const years = ref<RewindYears | null>(null)
const r = shallowRef<Rewind | null>(null)

onMounted(async () => {
  try {
    years.value = await bridge.rewind.years()
    if (props.seasonOnly && !years.value.season) return
    r.value = await bridge.rewind.get(years.value.current)
  }
  catch {
    // Без итогов плитки просто нет
  }
})

const colors = computed(() => slideColors(r.value?.topAlbums.flatMap(a => a.album.palette ?? []) ?? null))
const covers = computed(() => (r.value?.topAlbums ?? []).slice(0, 3).map(a => a.album.image?.medium).filter((u): u is string => !!u).map(u => bridge.resolveUrl(u)))
const meta = computed(() => {
  const x = r.value
  if (!x || x.plays === 0) return t('empty')
  const top = x.topArtists[0]?.artist.name
  return [lang.value === 'en' ? `${formatNumber(x.minutes, 'en')} min` : `${formatNumber(x.minutes)}${NBSP}${plural(x.minutes, ['минута', 'минуты', 'минут'])}`, top].filter(Boolean).join(' · ')
})
const past = computed(() => (years.value?.years ?? []).filter(y => y.year !== years.value?.current && y.minutes > 0))
</script>

<template>
  <div v-if="r" class="rwc">
    <NuxtLink :to="`/rewind?year=${r.year}`" class="rwc__card" :style="{ '--bg': colors.bg, '--ink': colors.ink, '--soft': colors.soft }">
      <span class="rwc__text">
        <span class="rwc__kicker">{{ r.complete ? t('kicker') : t('soFar') }}</span>
        <span class="rwc__year tnum">{{ r.year }}</span>
        <span class="rwc__meta ellipsis">{{ meta }}</span>
      </span>
      <span v-if="covers.length" class="rwc__covers" aria-hidden="true">
        <img v-for="(c, i) in covers" :key="c" :src="c" alt="" :style="{ '--i': i }" loading="lazy">
      </span>
      <Icon name="ph:caret-right" class="rwc__go" />
    </NuxtLink>
    <nav v-if="pastYears && past.length" class="rwc__past" :aria-label="t('past')">
      <NuxtLink v-for="y in past" :key="y.year" :to="`/rewind?year=${y.year}`" class="rwc__chip tnum">{{ y.year }}</NuxtLink>
    </nav>
  </div>
</template>

<style scoped>
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
  "ru": { "kicker": "Итоги года", "soFar": "Итоги по сей день", "empty": "Пока тихо — итоги соберутся из прослушиваний", "past": "Прошлые годы" },
  "en": { "kicker": "Year in review", "soFar": "Your year so far", "empty": "Quiet so far — your review builds as you listen", "past": "Past years" }
}
</i18n>
