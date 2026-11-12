<script setup lang="ts">
import type { PlaylistItem, Track } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Плейлист: обложка, описание, треки с альбомами. Свой — «Изменить» (название, описание, обложка,
// порядок), «Добавить треки», убрать из меню трека или выделением. Микс только для чтения:
// его можно «Сохранить как плейлист»
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const toast = useToast()
const shareMenu = useShare()
const ui = useUiStore()
const lang = useLang()
const pla = usePlaylistActions()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad(() => bridge.playlists.get(id.value), () => id.value)
const { playPlaylist, isContext, isPlayingContext, loading } = usePlayActions()
useHead({ title: () => data.value?.playlist.title ?? t('title') })

const library = useLibraryStore()
const pl = computed(() => {
  const p = data.value?.playlist
  if (!p) return null
  // Закрепление и обложка меняются из меню и боковой панели — берём свежую карточку из медиатеки
  const fresh = library.playlists?.find(x => x.id === p.id)
  return fresh ? { ...p, pinned: fresh.pinned, image: fresh.image, title: fresh.title, description: fresh.description } : p
})
const items = computed(() => data.value?.items ?? [])
const tracks = computed(() => items.value.map(i => i.track))
const playing = computed(() => !!pl.value && isPlayingContext('playlist', pl.value.id))
const kindKey = computed(() => (pl.value?.kind === 'mix' ? 'kind.mix' : pl.value?.kind === 'smart' ? 'kind.smart' : 'kind.user'))
const mine = computed(() => !!pl.value && pla.editable(pl.value))

// Правка — в адресе (?edit=1): «Изменить» из меню в боковой панели открывает сразу её
const editing = computed(() => mine.value && route.query.edit === '1')
function startEdit() {
  void router.replace({ query: { ...route.query, edit: '1' } })
}
async function finishEdit() {
  await router.replace({ query: { ...route.query, edit: undefined } })
  await refresh()
}

/** Перечитать без состояния «загрузка»: список не мигает */
async function refresh() {
  try {
    data.value = await bridge.playlists.get(id.value)
  }
  catch {
    // Останется прежнее; следующий заход перечитает
  }
}

const addOpen = ref(false)

// «Собрать как сет» поменял порядок этого плейлиста (или его вернули «Отменить») — перечитать
const pui = usePlaylistUi()
watch(() => pui.reordered, (r) => {
  if (r?.id === id.value) void refresh()
})

const moreItems = computed<MenuEntry[]>(() => {
  if (!pl.value) return []
  return [
    ...(mine.value ? [{ label: t('addTracks'), icon: 'ph:plus', onSelect: () => (addOpen.value = true) }] : []),
    ...pla.menuFor(pl.value, { page: true }),
    { kind: 'separator' },
    shareMenu.entry('playlist', pl.value.id, pl.value.title),
  ]
})


// ---------- Убрать из плейлиста (не в режиме правки) ----------

function setItems(next: PlaylistItem[]) {
  if (data.value) data.value = { ...data.value, items: next }
}

/** Вернуть убранные на прежние места: каждую — после той, что стояла перед ней */
async function restore(removed: { item: PlaylistItem, prev: string | null }[]) {
  const p = pl.value
  if (!p) return
  const moved = new Map<string, string>()
  try {
    for (const r of removed) {
      const after = r.prev ? (moved.get(r.prev) ?? r.prev) : null
      const res = await bridge.playlists.addItems(p.id, after ? { trackIds: [r.item.track.id], afterItemId: after } : { trackIds: [r.item.track.id], atStart: true })
      if (res.items[0]) moved.set(r.item.id, res.items[0].id)
    }
  }
  catch {
    toast.error(tg('pl.failed'))
  }
  await refresh()
}

async function removeIndices(indices: number[]) {
  const p = pl.value
  if (!p || !indices.length) return
  const all = items.value
  const removed = indices.map(i => ({ item: all[i]!, prev: all[i - 1]?.id ?? null })).filter(r => !!r.item)
  const ids = new Set(removed.map(r => r.item.id))
  setItems(all.filter(i => !ids.has(i.id)))
  try {
    if (removed.length === 1) await bridge.playlists.removeItem(p.id, removed[0]!.item.id)
    else await bridge.playlists.removeItems(p.id, [...ids])
  }
  catch {
    setItems(all)
    toast.error(tg('pl.failed'))
    return
  }
  void library.loadPlaylists(true).catch(() => {})
  toast.show({
    text: removed.length === 1 ? tg('pl.removed', { title: removed[0]!.item.track.title }) : tg('pl.removedMany', { n: formatCount(removed.length, 'track', lang.value) }),
    icon: 'ph:minus-circle',
    undo: () => void restore(removed),
  })
}

function extraMenu(_track: Track, index: number): MenuEntry[] {
  return [{ label: tg('pl.removeFrom'), icon: 'ph:minus-circle', danger: true, onSelect: () => void removeIndices([index]) }]
}
</script>

<template>
  <div class="playlist">
    <template v-if="state !== 'ready'">
      <MusicPhoneNav fallback="/library" />
      <MusicPageState :state="state" @retry="load" />
    </template>
    <MusicPlaylistEditor v-else-if="data && editing" :key="`edit:${data.playlist.id}`" :detail="data" @done="finishEdit" />
    <template v-else-if="data && pl">
      <MusicPageHeader :kicker="t(kindKey)" :title="pl.title" back="/library">
        <template #art>
          <MusicPlaylistCover :id="pl.id" :title="pl.title" :covers="pl.covers" :image="pl.image" :kind="pl.kind" :generated="pl.kind === 'mix'" />
        </template>
        <template v-if="pl.description" #sub>
          <span class="playlist__desc">{{ pl.description }}</span>
        </template>
        <template v-if="pl.tracksCount" #meta>
          <span>{{ formatCount(pl.tracksCount, 'track', lang) }}</span>
          <span>{{ formatLongDuration(pl.durationMs, lang) }}</span>
        </template>
        <template #actions>
          <UiButton variant="primary" size="lg" :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" :loading="loading === `playlist:${pl.id}`" :disabled="!tracks.length" @click="playPlaylist(pl.id)">
            {{ playing ? tg('common.pause') : isContext('playlist', pl.id) ? t('resume') : tg('common.play') }}
          </UiButton>
          <UiButton size="lg" icon="ph:shuffle" :disabled="!tracks.length" @click="playPlaylist(pl.id, { shuffle: true })">{{ t('shuffle') }}</UiButton>
        </template>
        <template #tools>
          <UiIconButton v-if="mine && !ui.isPhone" icon="ph:plus" :label="t('addTracks')" size="lg" @click="addOpen = true" />
          <UiIconButton v-if="mine && !ui.isPhone" icon="ph:pencil-simple" :label="tg('pl.edit')" size="lg" @click="startEdit" />
          <UiIconButton v-if="!mine && !ui.isPhone" icon="ph:plus-circle" :label="tg('pl.saveAs')" size="lg" @click="pla.duplicate(pl)" />
          <UiIconButton v-if="!ui.isPhone && pl.tracksCount > 2" icon="ph:vinyl-record" :label="tg('pl.set.title')" size="lg" @click="pui.openSet(pl)" />
          <MusicDownloadButton v-if="tracks.length" kind="playlist" :ref-id="pl.id" :title="pl.title" />
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <MusicTrackList
        v-if="tracks.length"
        :tracks="tracks"
        variant="wide"
        :context="{ type: 'playlist', id: pl.id, title: pl.title }"
        :label="pl.title"
        :extra-menu="mine ? extraMenu : undefined"
        :remove-many="mine ? removeIndices : undefined"
      />
      <UiEmptyState v-else-if="mine" icon="ph:playlist" :title="t('empty')" :text="t('emptyMine')">
        <UiButton variant="primary" icon="ph:plus" @click="addOpen = true">{{ t('addTracks') }}</UiButton>
      </UiEmptyState>
      <UiEmptyState v-else icon="ph:playlist" :title="t('empty')" />
      <MusicPlaylistAddTracks v-if="mine" v-model:open="addOpen" :detail="data" @added="refresh" />
    </template>
  </div>
</template>

<style scoped>
.playlist { display: flex; flex-direction: column; gap: 28px; }
.playlist__desc { display: -webkit-box; overflow: hidden; font-size: var(--fs-body); font-weight: 450; color: var(--text-2); text-wrap: pretty; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
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
    "addTracks": "Добавить треки",
    "empty": "В плейлисте пока пусто",
    "emptyMine": "Найдите треки или возьмите из подходящих"
  },
  "en": {
    "title": "Playlist",
    "kind": { "user": "Playlist", "smart": "Smart playlist", "mix": "Mix" },
    "resume": "Resume",
    "shuffle": "Shuffle",
    "addTracks": "Add tracks",
    "empty": "This playlist is empty",
    "emptyMine": "Search for tracks or pick from suggestions"
  }
}
</i18n>
