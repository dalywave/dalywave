<script setup lang="ts">
import type { RecentItem } from '~/composables/useSearchRecent'

// «Недавние»: запросы и то, что открывали из поиска. ✕ убирает одно, «Очистить» — всё
const props = withDefaults(defineProps<{ limit?: number }>(), { limit: 12 })

const router = useRouter()
const player = usePlayerStore()
const { t } = useI18n({ useScope: 'local' })
const recent = useSearchRecent()
const list = computed(() => recent.items.value.slice(0, props.limit))

function sub(item: RecentItem) {
  const kind = t(`kind.${item.kind}`)
  return item.sub ? `${kind} · ${item.sub}` : kind
}

function link(item: RecentItem): string | undefined {
  if (item.kind === 'artist') return `/artist/${item.id}`
  if (item.kind === 'album') return `/album/${item.id}`
  if (item.kind === 'playlist') return `/playlist/${item.id}`
  return undefined
}

function open(item: RecentItem) {
  if (item.kind === 'query') {
    void router.push({ path: '/search', query: { q: item.id } })
    return
  }
  if (item.kind === 'track' && item.track) {
    recent.add(item)
    if (player.current?.track.id === item.track.id) void player.toggle()
    else void player.playTracks([item.track], 0, { type: 'search', id: item.title, title: item.title })
    return
  }
  recent.add(item)
}
</script>

<template>
  <section v-if="list.length" class="recent" :aria-label="t('title')">
    <header class="recent__head">
      <h2 class="recent__title">{{ t('title') }}</h2>
      <button type="button" class="recent__clear" @click="recent.clear()">{{ t('clear') }}</button>
    </header>
    <div class="recent__list">
      <SearchItemRow
        v-for="item in list"
        :key="`${item.kind}:${item.id}`"
        :title="item.title"
        :sub="item.kind === 'query' ? undefined : sub(item)"
        :image="item.image"
        :playlist="item.kind === 'playlist' ? { id: item.id, title: item.title, covers: item.covers ?? [] } : undefined"
        :icon="item.kind === 'query' ? 'ph:magnifying-glass' : undefined"
        :round="item.kind === 'artist'"
        :to="link(item)"
        @open="open(item)"
      >
        <template #end>
          <UiIconButton icon="ph:x" size="sm" :label="t('remove')" :tooltip="false" class="recent__x" @click="recent.remove(item)" />
        </template>
      </SearchItemRow>
    </div>
  </section>
</template>

<style scoped>
.recent { display: flex; flex-direction: column; gap: 4px; }
.recent__head { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 44px; }
.recent__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.recent__clear { min-height: 44px; padding: 0 2px 0 12px; border: 0; background: none; font: inherit; font-size: var(--fs-body); font-weight: 550; color: var(--text-2); transition: color var(--dur-1); }
.recent__clear:hover { color: var(--text-1); }
.recent__clear:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: var(--r-1); }
.recent__list { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 0 16px; margin: 0 -8px; }
.recent .recent__list :deep(.recent__x) { color: var(--text-3); }
.recent .recent__list :deep(.recent__x:hover) { color: var(--text-1); }
@media (max-width: 699px) {
  .recent__title { font-size: 20px; }
  .recent__list { grid-template-columns: minmax(0, 1fr); }
  .recent .recent__list :deep(.recent__x) { --size: 44px; margin-right: -8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Недавние",
    "clear": "Очистить",
    "remove": "Убрать",
    "kind": { "query": "Запрос", "artist": "Артист", "album": "Альбом", "track": "Трек", "playlist": "Плейлист" }
  },
  "uk": {
    "title": "Нещодавні",
    "clear": "Очистити",
    "remove": "Прибрати",
    "kind": { "query": "Запит", "artist": "Виконавець", "album": "Альбом", "track": "Трек", "playlist": "Плейлист" }
  },
  "en": {
    "title": "Recent",
    "clear": "Clear",
    "remove": "Remove",
    "kind": { "query": "Search", "artist": "Artist", "album": "Album", "track": "Song", "playlist": "Playlist" }
  }
}
</i18n>
