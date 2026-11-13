<script setup lang="ts">
import type { ChartDetail } from '~/api/types'

// Чарт: треки по местам (как плейлист: «Слушать», «Перемешать»), альбомы и артисты чарта.
// Треков, которых нет на сервере, не ждём: качаются как обычно при включении. Чарт, который
// сервер ещё собирает, дорисовывается сам
const route = useRoute()
const bridge = useBridge()
const player = usePlayerStore()
const lang = useLang()
const { t, te } = useI18n()

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad<ChartDetail>(() => bridge.catalog.chart(id.value), () => id.value)

const chart = computed(() => data.value?.chart ?? null)
const name = computed(() => (te(`charts.name.${id.value}`) ? t(`charts.name.${id.value}`) : chart.value?.title ?? ''))
const title = computed(() => (te(`charts.heading.${id.value}`) ? t(`charts.heading.${id.value}`) : name.value))
useHead({ title: () => title.value || t('charts.title') })

const tracks = computed(() => data.value?.tracks ?? [])
const ctx = computed(() => chartContext({ id: id.value }, title.value))
const isCtx = computed(() => player.context?.type === 'search' && player.context.id === ctx.value.id)
const playing = computed(() => isCtx.value && player.playing)
const FIRST = 20
const showAll = ref(false)
const shown = computed(() => (showAll.value ? tracks.value : tracks.value.slice(0, FIRST)))
watch(id, () => {
  showAll.value = false
})

function play(shuffle = false) {
  if (!tracks.value.length) return
  if (isCtx.value && !shuffle) return player.toggle()
  return player.playTracks(tracks.value, 0, ctx.value, { shuffle })
}

// Чарт собирается: спросить ещё раз, пока не соберётся (не дольше пары минут)
let poll = 0
let tries = 0
function schedule() {
  window.clearTimeout(poll)
  if (!data.value?.updating || tries > 40) return
  poll = window.setTimeout(async () => {
    tries++
    try {
      const fresh = await bridge.catalog.chart(id.value)
      if (fresh.chart.id === id.value) data.value = fresh
    }
    catch {
      // Сеть мигнула — следующая попытка через те же секунды
    }
    schedule()
  }, tracks.value.length ? 6000 : 3000)
}
watch(data, () => {
  tries = 0
  schedule()
})
onBeforeUnmount(() => window.clearTimeout(poll))
</script>

<template>
  <div class="chart">
    <template v-if="state !== 'ready'">
      <MusicPhoneNav fallback="/charts" />
      <MusicPageState :state="state" @retry="load" />
    </template>
    <template v-else-if="chart">
      <MusicPageHeader :kicker="chart.kind === 'genre' ? t('charts.kickerGenre') : t('charts.kicker')" :title="title" back="/charts">
        <template #art>
          <div class="chart__cover" :style="{ '--tile': chartTile(chart.id) }">
            <span class="chart__cover-top">{{ t('charts.top') }}</span>
            <span class="chart__cover-name">{{ name }}</span>
            <span v-if="chart.image" class="chart__cover-art"><MusicCoverArt :image="chart.image" size="medium" eager /></span>
          </div>
        </template>
        <template v-if="tracks.length" #meta>
          <span>{{ formatCount(tracks.length, 'track', lang) }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :disabled="!tracks.length" @click="play()">
            {{ playing ? t('common.pause') : t('common.play') }}
          </UiButton>
          <UiButton size="lg" icon="ph:shuffle" :disabled="!tracks.length" @click="play(true)">{{ t('common.shuffle') }}</UiButton>
        </template>
      </MusicPageHeader>

      <section class="chart__tracks" :aria-label="t('charts.tracks')">
        <MusicTrackList v-if="tracks.length" :tracks="shown" hook :context="ctx" :label="title" />
        <div v-else-if="data?.updating" class="chart__wait" aria-hidden="true">
          <div v-for="i in 8" :key="i" class="chart__wait-row">
            <UiSkeleton class="chart__wait-art" />
            <div class="chart__wait-text">
              <UiSkeleton variant="line" width="min(260px, 60%)" />
              <UiSkeleton variant="line" width="min(160px, 40%)" />
            </div>
          </div>
        </div>
        <UiEmptyState v-else icon="ph:chart-bar" :title="t('charts.empty')" compact />
        <UiButton v-if="tracks.length > FIRST" variant="ghost" size="sm" class="chart__more" @click="showAll = !showAll">
          {{ showAll ? t('charts.less') : t('charts.more', { n: tracks.length }) }}
        </UiButton>
      </section>

      <MusicShelf v-if="data?.albums.length" :title="t('charts.albums')" :cols="5">
        <MusicAlbumCard v-for="a in data.albums" :key="a.id" :album="a" />
      </MusicShelf>

      <MusicShelf v-if="data?.artists.length" :title="t('charts.artists')" :cols="6" phone-width="120px">
        <MusicArtistCard v-for="a in data.artists" :key="a.id" :artist="a" />
      </MusicShelf>
    </template>
  </div>
</template>

<style scoped>
.chart { display: flex; flex-direction: column; gap: 32px; }
.chart__cover {
  position: relative;
  aspect-ratio: 1;
  overflow: hidden;
  border-radius: var(--r-4);
  background: var(--tile);
  color: #fff;
  text-align: left;
  box-shadow: var(--shadow-cover);
  isolation: isolate;
}
.chart__cover-top { position: absolute; left: 18px; top: 18px; z-index: 2; font-size: var(--fs-caption); font-weight: 700; letter-spacing: var(--tr-caps); text-transform: uppercase; opacity: 0.8; }
.chart__cover-name { position: absolute; left: 18px; top: 40px; right: 18px; z-index: 2; font-size: 30px; font-weight: 750; letter-spacing: -0.02em; line-height: 1.1; text-wrap: balance; }
.chart__cover-art {
  position: absolute;
  right: -6%;
  bottom: -5%;
  width: 46%;
  overflow: hidden;
  border-radius: var(--r-2);
  box-shadow: -4px 8px 22px -8px rgba(0, 0, 0, 0.5);
  transform: rotate(18deg);
}
.chart__cover-art :deep(.art) { width: 100%; border-radius: 0; box-shadow: none; }
.chart__tracks { display: flex; flex-direction: column; gap: 4px; }
.chart__more { align-self: flex-start; margin-left: 4px; }
.chart__wait { display: flex; flex-direction: column; }
.chart__wait-row { display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 8px 10px; }
.chart__wait-art { flex: none; width: 40px; height: 40px; border-radius: 6px; }
.chart__wait-text { flex: 1; display: flex; flex-direction: column; gap: 8px; }
@media (max-width: 699px) {
  .chart { gap: 24px; }
  .chart__tracks { padding: 0 8px; }
  .chart__more { margin-left: 10px; }
  .chart__cover-name { font-size: 26px; }
}
</style>
