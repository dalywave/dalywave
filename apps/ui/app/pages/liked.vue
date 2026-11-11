<script setup lang="ts">
import type { Track } from '~/api/types'

// Любимые треки: весь список (рисуется окном — хоть десять тысяч), фильтр по названию и артисту
const bridge = useBridge()
const library = useLibraryStore()
const player = usePlayerStore()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()
useHead({ title: () => t('title') })

const tracks = ref<Track[]>([])
const state = ref<'loading' | 'ready' | 'error'>('loading')
const more = ref(false)
const filter = ref('')

async function load() {
  state.value = 'loading'
  try {
    let page = await bridge.library.liked(null, 200)
    tracks.value = page.items.map(i => i.track)
    state.value = 'ready'
    // Остальное — следом, не задерживая первый экран
    more.value = !!page.nextCursor
    while (page.nextCursor) {
      page = await bridge.library.liked(page.nextCursor, 200)
      tracks.value = [...tracks.value, ...page.items.map(i => i.track)]
    }
    more.value = false
  }
  catch {
    if (!tracks.value.length) state.value = 'error'
    more.value = false
  }
}
onMounted(load)

// Снятые сейчас лайки остаются в списке до перезахода — чтобы можно было вернуть
const shown = computed(() => {
  const q = filter.value.trim().toLowerCase()
  if (!q) return tracks.value
  return tracks.value.filter(tr => tr.title.toLowerCase().includes(q) || tr.artists.some(a => a.name.toLowerCase().includes(q)) || tr.album?.title.toLowerCase().includes(q))
})
const ctx = computed(() => ({ type: 'library' as const, id: 'liked', title: t('title') }))
const isCtx = computed(() => player.context?.type === 'library' && player.context.id === 'liked')
const playing = computed(() => isCtx.value && player.playing)
const duration = computed(() => tracks.value.reduce((s, tr) => s + tr.durationMs, 0))

function play(shuffle = false) {
  if (isCtx.value && !shuffle) return player.toggle()
  return player.playTracks(shown.value, 0, ctx.value, { shuffle })
}
</script>

<template>
  <div class="liked">
    <MusicPageState v-if="state === 'loading'" state="loading" />
    <MusicPageState v-else-if="state === 'error'" state="error" @retry="load" />
    <template v-else>
      <MusicPageHeader :kicker="t('kicker')" :title="t('title')" glow="#ff3d7a">
        <template #art>
          <div class="liked__art"><Icon name="ph:heart-fill" /></div>
        </template>
        <template #meta>
          <span>{{ formatCount(tracks.length, 'track', lang) }}</span>
          <span class="dot">·</span>
          <span>{{ formatLongDuration(duration, lang) }}</span>
          <span v-if="more" class="dot">·</span>
          <span v-if="more">{{ tg('common.loading') }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :disabled="!tracks.length" @click="play()">{{ playing ? tg('common.pause') : tg('common.play') }}</UiButton>
          <UiButton size="lg" icon="ph:shuffle" :disabled="!tracks.length" @click="play(true)">{{ t('shuffle') }}</UiButton>
        </template>
      </MusicPageHeader>

      <div v-if="tracks.length" class="liked__tools">
        <UiSearchField v-model="filter" :placeholder="t('filter')" :size="ui.isPhone ? 'lg' : 'md'" class="liked__filter" />
        <span v-if="filter" class="liked__count">{{ formatCount(shown.length, 'track', lang) }}</span>
      </div>

      <MusicTrackList v-if="shown.length" :tracks="shown" variant="wide" :context="ctx" :virtual-from="120" />
      <UiEmptyState v-else-if="filter" icon="ph:magnifying-glass" :title="t('nothing')" :text="t('nothingText', { q: filter })" compact />
      <UiEmptyState v-else icon="ph:heart" :title="t('empty')" :text="t('emptyText')">
        <UiButton variant="primary" icon="ph:play-fill" @click="player.startWave()">{{ tg('wave.listen') }}</UiButton>
      </UiEmptyState>
      <ShellDevMockNote :sections="['library']" what="любимые треки" class="liked__note" />
    </template>
    <span class="sr-only">{{ library.likedCount }}</span>
  </div>
</template>

<style scoped>
.liked { display: flex; flex-direction: column; gap: 24px; }
.liked__art { display: grid; place-items: center; aspect-ratio: 1; border-radius: var(--r-4); background: radial-gradient(110% 110% at 25% 15%, var(--m3), var(--m2) 45%, var(--m1)); box-shadow: var(--shadow-cover); color: #fff; font-size: 72px; }
.dot { color: var(--text-4); }
.liked__tools { display: flex; align-items: center; gap: 16px; }
.liked__filter { width: min(360px, 100%); }
.liked__count { font-size: var(--fs-small); color: var(--text-3); }
.liked__note { align-self: center; }
@media (max-width: 699px) {
  .liked { gap: 20px; }
  .liked__tools { padding: 0 16px; }
  .liked__filter { width: 100%; }
  .liked > :deep(.tl) { padding: 0 8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Любимые треки",
    "kicker": "Медиатека",
    "shuffle": "Перемешать",
    "filter": "Найти среди любимых",
    "nothing": "Ничего не нашлось",
    "nothingText": "Среди любимых нет «{q}». Поищите во всём каталоге",
    "empty": "Здесь появится то, что вам нравится",
    "emptyText": "Отмечайте треки сердцем — в плеере, в списках или клавишей L"
  },
  "en": {
    "title": "Liked songs",
    "kicker": "Library",
    "shuffle": "Shuffle",
    "filter": "Search liked songs",
    "nothing": "Nothing found",
    "nothingText": "No “{q}” among liked songs. Try the whole catalog",
    "empty": "Songs you like will show up here",
    "emptyText": "Tap the heart — in the player, in lists, or press L"
  }
}
</i18n>
