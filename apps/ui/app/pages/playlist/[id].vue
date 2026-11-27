<script setup lang="ts">
import type { PersonRef, PlaylistItem, Track } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Плейлист: обложка, описание, треки с альбомами. Свой — «Изменить» (название, описание, обложка,
// порядок), «Добавить треки», убрать из меню трека или выделением. Микс только для чтения:
// его можно «Сохранить как плейлист». Совместный: под названием — участники, у треков — кто добавил;
// участник добавляет, убирает и переставляет треки. Плейлист друга — только слушать и сохранить копию
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const toast = useToast()
const shareMenu = useShare()
const ui = useUiStore()
const lang = useLang()
const pla = usePlaylistActions()
const pui = usePlaylistUi()
const { t } = useScopedI18n('pages.playlist._id')
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
/** Треки можно менять: свой или совместный, где человек участник */
const mine = computed(() => !!pl.value && pla.editable(pl.value))
/** Название, обложка, сет, удаление — только владелец */
const owner = computed(() => !!pl.value && mine.value && pla.isOwner(pl.value))

// ---------- Совместный: участники и кто добавил ----------

const auth = useAuthStore()
/** Владелец и участники: владелец — первым */
const people = computed<PersonRef[]>(() => {
  const p = pl.value
  if (!p) return []
  const me = auth.user ? { id: auth.user.id, name: auth.user.name, avatarUrl: auth.user.avatarUrl ?? null } : null
  const own = p.owner ?? (p.access === 'owner' ? me : null)
  return [...(own ? [own] : []), ...(p.members ?? [])]
})
const peopleLine = computed(() => {
  const names = people.value.map(x => (x.id === auth.user?.id ? t('you') : x.name))
  const line = names.length <= 3 ? names.join(', ') : t('andMore', { names: names.slice(0, 2).join(', '), n: names.length - 2 })
  return line.charAt(0).toUpperCase() + line.slice(1)
})
const addedBy = computed(() => {
  if (!pl.value?.collaborative) return undefined
  const byId = new Map(people.value.map(x => [x.id, x]))
  return items.value.map(i => (i.addedBy ? byId.get(i.addedBy) ?? null : null))
})
watch(() => pui.peopleChanged, () => void refresh())

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

watch(() => pui.reordered, (r) => {
  if (r?.id === id.value) void refresh()
})

// ---------- Собран по описанию: «Ещё в том же духе» и обновление раз в неделю ----------

const described = computed(() => mine.value && pl.value?.generator === 'describe')
const weekly = ref<boolean | null>(null)
watch([described, id], async ([d]) => {
  weekly.value = null
  if (!d) return
  try {
    weekly.value = (await bridge.describe.get(id.value))?.weekly ?? null
  }
  catch {
    // Пункт меню просто не покажется
  }
}, { immediate: true })
const extending = ref(false)
async function moreLikeThis() {
  if (extending.value) return
  extending.value = true
  try {
    const added = await bridge.describe.extend(id.value, 10)
    await refresh()
    void library.loadPlaylists(true).catch(() => {})
    toast.show({ text: added.length ? t('moreAdded', { n: formatCount(added.length, 'track', lang.value) }) : t('noMore'), icon: 'ph:plus' })
  }
  catch {
    toast.error(tg('pl.failed'))
  }
  finally {
    extending.value = false
  }
}
async function toggleWeekly() {
  const next = !weekly.value
  try {
    weekly.value = (await bridge.describe.setWeekly(id.value, next)).weekly
    toast.show({ text: next ? t('weeklyOn') : t('weeklyOff'), icon: 'ph:calendar-check' })
  }
  catch {
    toast.error(tg('pl.failed'))
  }
}

const moreItems = computed<MenuEntry[]>(() => {
  if (!pl.value) return []
  return [
    ...(mine.value ? [{ label: t('addTracks'), icon: 'ph:plus', onSelect: () => (addOpen.value = true) }] : []),
    ...(described.value ? [{ label: t('moreLikeThis'), icon: 'ph:list-plus', onSelect: () => void moreLikeThis() }] : []),
    ...(described.value && weekly.value !== null ? [{ label: t('weekly'), icon: 'ph:calendar-check', checked: weekly.value, onSelect: () => void toggleWeekly() }] : []),
    ...pla.menuFor(pl.value, { page: true }),
    { kind: 'separator' },
    shareMenu.entry('playlist', pl.value.id, pl.value.title, { owner: (pl.value.access ?? 'owner') === 'owner' }),
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
    <MusicPlaylistEditor v-else-if="data && editing" :key="`edit:${data.playlist.id}`" :detail="data" :order-only="!owner" @done="finishEdit" />
    <template v-else-if="data && pl">
      <MusicPageHeader :kicker="pl.collaborative ? tg('pl.shared') : t(kindKey)" :title="pl.title" back="/library">
        <template #art>
          <MusicPlaylistCover :id="pl.id" :title="pl.title" :covers="pl.covers" :image="pl.image" :kind="pl.kind" :generated="pl.kind === 'mix'" />
        </template>
        <template v-if="pl.description || pl.collaborative || pl.owner" #sub>
          <!-- Совместный: кто в плейлисте (нажатие — участники и приглашение); чужой — чей -->
          <button v-if="pl.collaborative && pl.access !== 'viewer'" type="button" class="playlist__people" @click="pui.openPeople(pl)">
            <span class="playlist__stack" aria-hidden="true">
              <FriendsAvatar v-for="x in people.slice(0, 4)" :key="x.id" :person="x" :size="26" />
            </span>
            <span class="ellipsis">{{ peopleLine }}</span>
          </button>
          <NuxtLink v-else-if="pl.owner" :to="`/friends/${pl.owner.id}`" class="playlist__people">
            <FriendsAvatar :person="pl.owner" :size="26" />
            <span class="ellipsis">{{ pl.owner.name }}</span>
          </NuxtLink>
          <span v-if="pl.description" class="playlist__desc">{{ pl.description }}</span>
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
          <UiIconButton v-if="described && !ui.isPhone" icon="ph:list-plus" :label="t('moreLikeThis')" size="lg" :disabled="extending" @click="moreLikeThis" />
          <UiIconButton v-if="owner && !ui.isPhone" icon="ph:pencil-simple" :label="tg('pl.edit')" size="lg" @click="startEdit" />
          <UiIconButton v-if="mine && !ui.isPhone" :icon="owner ? 'ph:user-plus' : 'ph:users-three'" :label="owner ? (pl.collaborative ? tg('pl.people') : tg('pl.invite')) : tg('pl.people')" size="lg" @click="pui.openPeople(pl)" />
          <UiIconButton v-if="!mine && !ui.isPhone" icon="ph:plus-circle" :label="tg('pl.saveAs')" size="lg" @click="pla.duplicate(pl)" />
          <UiIconButton v-if="!ui.isPhone && pl.tracksCount > 2 && (owner || pl.kind !== 'user')" icon="ph:vinyl-record" :label="tg('pl.set.title')" size="lg" @click="pui.openSet(pl)" />
          <MusicDownloadButton v-if="tracks.length && pl.access !== 'viewer'" kind="playlist" :ref-id="pl.id" :title="pl.title" />
          <UiMenu :items="moreItems" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <MusicTrackList
        v-if="tracks.length"
        :tracks="tracks"
        variant="wide"
        :hook="pl.kind === 'mix'"
        :context="{ type: 'playlist', id: pl.id, title: pl.title }"
        :label="pl.title"
        :extra-menu="mine ? extraMenu : undefined"
        :remove-many="mine ? removeIndices : undefined"
        :added-by="addedBy"
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
.playlist__people { display: inline-flex; align-items: center; gap: 10px; max-width: 100%; min-height: 32px; padding: 0; border: 0; background: none; font: inherit; font-size: var(--fs-body); font-weight: 600; color: var(--text-1); text-align: left; cursor: pointer; }
.playlist__people:hover .ellipsis { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.playlist__people:focus-visible { outline: 2px solid var(--focus); outline-offset: 3px; border-radius: var(--r-1); }
.playlist__stack { display: inline-flex; flex: none; }
.playlist__stack > * + * { margin-left: -8px; box-shadow: 0 0 0 2px var(--ink-1); }
.playlist__desc { flex-basis: 100%; display: -webkit-box; overflow: hidden; font-size: var(--fs-body); font-weight: 450; color: var(--text-2); text-wrap: pretty; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
@media (max-width: 699px) {
  .playlist { gap: 20px; }
  .playlist > :deep(.tl) { padding: 0 8px; }
}
</style>
