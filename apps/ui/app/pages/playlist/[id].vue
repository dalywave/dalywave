<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Плейлист: обложка, описание, треки с альбомами
const route = useRoute()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad(() => bridge.playlists.get(id.value), () => id.value)
const { playPlaylist, isContext, isPlayingContext, loading } = usePlayActions()
useHead({ title: () => data.value?.playlist.title ?? t('title') })

const pl = computed(() => data.value?.playlist ?? null)
const tracks = computed(() => data.value?.items.map(i => i.track) ?? [])
const playing = computed(() => !!pl.value && isPlayingContext('playlist', pl.value.id))
const ready = computed(() => tracks.value.filter(tr => tr.audioStatus === 'ready').length)
const kindKey = computed(() => (pl.value?.kind === 'mix' ? 'kind.mix' : pl.value?.kind === 'smart' ? 'kind.smart' : 'kind.user'))

const moreItems = computed<MenuEntry[]>(() => {
  const p = pl.value
  if (!p) return []
  return [
    { label: t('shuffle'), icon: 'ph:shuffle', onSelect: () => playPlaylist(p.id, { shuffle: true }) },
    { label: p.pinned ? t('unpin') : t('pin'), icon: 'ph:push-pin', onSelect: () => toast.show({ text: t('editSoon'), icon: 'ph:push-pin' }) },
    { label: t('edit'), icon: 'ph:pencil-simple', onSelect: () => toast.show({ text: t('editSoon'), icon: 'ph:pencil-simple' }) },
  ]
})
</script>

<template>
  <div class="playlist">
    <MusicPageState v-if="state !== 'ready'" :state="state" @retry="load" />
    <template v-else-if="data && pl">
      <MusicPageHeader :kicker="t(kindKey)" :title="pl.title">
        <template #art>
          <MusicPlaylistCover :id="pl.id" :title="pl.title" :covers="pl.covers" :kind="pl.kind" :generated="pl.kind === 'mix'" />
        </template>
        <template v-if="pl.description" #sub>
          <span class="playlist__desc">{{ pl.description }}</span>
        </template>
        <template #meta>
          <span v-if="pl.pinned" class="pinned"><Icon name="ph:push-pin-fill" />{{ t('pinned') }}</span>
          <span>{{ formatCount(pl.tracksCount, 'track', lang) }}</span>
          <span>{{ formatLongDuration(pl.durationMs, lang) }}</span>
        </template>
        <template v-if="tracks.length && ready < tracks.length" #status>
          <span class="ready"><Icon name="ph:cloud-arrow-down" />{{ t('ready', { n: ready, total: tracks.length }) }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :loading="loading === `playlist:${pl.id}`" :disabled="!tracks.length" @click="playPlaylist(pl.id)">
            {{ playing ? tg('common.pause') : isContext('playlist', pl.id) ? t('resume') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" icon="ph:shuffle" :disabled="!tracks.length" @click="playPlaylist(pl.id, { shuffle: true })">{{ t('shuffle') }}</UiButton>
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <MusicTrackList v-if="tracks.length" :tracks="tracks" variant="wide" :context="{ type: 'playlist', id: pl.id, title: pl.title }" :label="pl.title" />
      <UiEmptyState v-else icon="ph:playlist" :title="t('empty')" :text="t('emptyText')">
        <UiButton to="/search" icon="ph:magnifying-glass">{{ t('find') }}</UiButton>
      </UiEmptyState>
      <ShellDevMockNote :sections="['library']" what="плейлист" class="playlist__note" />
    </template>
  </div>
</template>

<style scoped>
.playlist { display: flex; flex-direction: column; gap: 28px; }
.playlist__desc { font-size: var(--fs-body); font-weight: 450; color: var(--text-2); }
.pinned { display: inline-flex; align-items: center; gap: 4px; color: var(--accent); }
.ready :deep(.iconify) { margin-right: 5px; vertical-align: -2px; color: var(--text-3); }
.playlist__note { align-self: center; }
@media (max-width: 699px) {
  .playlist { gap: 20px; }
  .playlist > :deep(.tl) { padding: 0 8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Плейлист",
    "kind": { "user": "Плейлист", "smart": "Умная подборка", "mix": "Микс" },
    "resume": "Продолжить",
    "shuffle": "Перемешать",
    "pinned": "Закреплён",
    "pin": "Закрепить",
    "unpin": "Открепить",
    "edit": "Изменить",
    "editSoon": "Редактор плейлистов появится вместе с синхронизацией медиатеки",
    "ready": "{n} из {total} на сервере",
    "empty": "В плейлисте пока пусто",
    "emptyText": "Добавьте треки через меню трека → «В плейлист»",
    "find": "Найти музыку"
  },
  "en": {
    "title": "Playlist",
    "kind": { "user": "Playlist", "smart": "Smart playlist", "mix": "Mix" },
    "resume": "Resume",
    "shuffle": "Shuffle",
    "pinned": "Pinned",
    "pin": "Pin",
    "unpin": "Unpin",
    "edit": "Edit",
    "editSoon": "The playlist editor arrives with library sync",
    "ready": "{n} of {total} on the server",
    "empty": "This playlist is empty",
    "emptyText": "Add tracks from a track's menu → “Add to playlist”",
    "find": "Find music"
  }
}
</i18n>
