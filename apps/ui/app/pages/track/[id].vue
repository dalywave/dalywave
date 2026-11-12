<script setup lang="ts">
// Трек по ссылке «Поделиться» (wave.daly.rest/track/{id}): обложка, кто, «Слушать» и радио по треку;
// остальное — в ⋯, как у строки трека. Альбом — ссылкой под шапкой
const route = useRoute()
const bridge = useBridge()
const player = usePlayerStore()
const library = useLibraryStore()
const { menuFor } = useTrackActions()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const id = computed(() => String(route.params.id))
const { data: track, state, load } = useLoad(() => bridge.catalog.track(id.value), () => id.value)

useHead({ title: () => track.value?.title ?? t('title') })

const liked = computed(() => (track.value ? library.isLiked(track.value) : false))
const isCurrent = computed(() => !!track.value && player.current?.track.id === track.value.id)
const playing = computed(() => isCurrent.value && player.playing)
const moreItems = computed(() => (track.value ? menuFor(track.value) : []))

function play() {
  const tr = track.value
  if (!tr) return
  if (isCurrent.value) player.toggle()
  else void player.playTracks([tr], 0, { type: 'search', id: null, title: tr.title })
}

function radio() {
  const tr = track.value
  if (tr) void player.startWave(undefined, { seed: { type: 'track', id: tr.id }, title: tr.title })
}
</script>

<template>
  <div class="trk">
    <template v-if="state !== 'ready'">
      <MusicPhoneNav />
      <MusicPageState :state="state" @retry="load" />
    </template>
    <template v-else-if="track">
      <MusicPageHeader :kicker="t('title')" :title="track.title">
        <template #art>
          <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? track.title" eager />
        </template>
        <template #sub>
          <span class="trk__artists ellipsis">
            <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`">{{ a.name }}</NuxtLink></template>
          </span>
          <UiBadge v-if="track.explicit" variant="explicit" />
        </template>
        <template #meta>
          <span>{{ formatDuration(track.durationMs) }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" @click="play">
            {{ playing ? tg('common.pause') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" icon="ph:waveform" @click="radio">{{ t('radio') }}</UiButton>
        </template>
        <template #tools>
          <UiIconButton
            :icon="liked ? 'ph:heart-fill' : 'ph:heart'"
            :label="liked ? tg('player.unlike') : tg('player.like')"
            :tone="liked ? 'liked' : 'default'"
            :pressed="liked"
            size="lg"
            @click="player.toggleLike(track)"
          />
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <NuxtLink v-if="track.album" :to="`/album/${track.album.id}`" class="trk__album">
        <MusicCoverArt :image="track.album.image" size="small" :alt="track.album.title" class="trk__album-art" />
        <span class="trk__album-text">
          <span class="trk__album-kicker">{{ tg('record.album') }}</span>
          <span class="trk__album-title ellipsis">{{ track.album.title }}</span>
        </span>
        <Icon name="ph:caret-right" class="trk__chev" aria-hidden="true" />
      </NuxtLink>
    </template>
  </div>
</template>

<style scoped>
.trk { display: flex; flex-direction: column; gap: 28px; }
.trk__artists { min-width: 0; }
.trk__artists a:hover { text-decoration: underline; text-decoration-color: var(--text-4); text-underline-offset: 3px; }
.trk__album { display: flex; align-items: center; gap: 14px; max-width: 560px; min-height: 64px; padding: 8px 10px; border-radius: var(--r-3); }
.trk__album:hover { background: var(--fill-1); }
.trk__album:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.trk__album-art { flex: none; width: 48px; height: 48px; border-radius: var(--r-1); }
.trk__album-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1; }
.trk__album-kicker { font-size: var(--fs-caption); color: var(--text-3); }
.trk__album-title { font-weight: 600; }
.trk__chev { flex: none; color: var(--text-3); }
@media (max-width: 699px) {
  .trk { gap: 20px; }
  .trk__album { margin: 0 8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Трек",
    "radio": "Радио"
  },
  "en": {
    "title": "Track",
    "radio": "Radio"
  }
}
</i18n>
