<script setup lang="ts">
// «Поделиться → DalyWave»: что нашлось по чужой ссылке и что с этим сделать. Один на приложение
// (app.vue). Здесь же подписки, откуда ссылки приходят: dalywave://share из меню «Поделиться»
// iPhone и macOS, вставка в окно (не в поле ввода), перетаскивание ссылки на окно.
// Поле поиска зовёт useSharedLink().open() само.
const { state, open, close } = useSharedLink()
const bridge = useBridge()
const player = usePlayerStore()
const library = useLibraryStore()
const pui = usePlaylistUi()
const ui = useUiStore()
const auth = useAuthStore()
const { playAlbum, loading: albumLoading } = usePlayActions()
const { t } = useScopedI18n('components.music.SharedLink')

const shown = computed({
  get: () => state.value !== null,
  set: (v: boolean) => {
    if (!v) close()
  },
})

const SOURCES: Record<string, string> = {
  deezer: 'Deezer', spotify: 'Spotify', apple: 'Apple Music', youtube: 'YouTube', soundcloud: 'SoundCloud', bandcamp: 'Bandcamp', dalywave: 'DalyWave',
}
const result = computed(() => state.value?.result ?? null)
const track = computed(() => result.value?.track ?? null)
const album = computed(() => (track.value ? null : result.value?.album ?? null))
const title = computed(() => {
  const s = result.value?.source
  return s && SOURCES[s] ? t('from', { source: SOURCES[s] }) : t('title')
})
const liked = computed(() => (track.value ? library.isLiked(track.value) : false))

function playTrack() {
  const tr = track.value
  if (!tr) return
  void player.playTracks([tr], 0, { type: 'search', id: null, title: tr.title })
  close()
}

function addToPlaylist() {
  const tr = track.value
  if (!tr) return
  close()
  pui.pick([tr])
}

function openAlbum(id: string) {
  close()
  void ui.navigate(`/album/${id}`)
}

async function playWholeAlbum(id: string) {
  await playAlbum(id)
  close()
}

// ---------- откуда приходят ссылки ----------

function editable(el: EventTarget | null) {
  const node = el as HTMLElement | null
  return !!node && (node.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(node.tagName))
}

function onPaste(e: ClipboardEvent) {
  if (editable(e.target) || auth.status !== 'ready') return
  const text = e.clipboardData?.getData('text/plain') ?? ''
  if (text && open(text)) e.preventDefault()
}

function dropped(e: DragEvent) {
  const dt = e.dataTransfer
  return dt ? (dt.getData('text/uri-list') || dt.getData('text/plain')) : ''
}

function onDragOver(e: DragEvent) {
  const types = e.dataTransfer?.types ?? []
  if (types.includes('text/uri-list') && !editable(e.target)) e.preventDefault()
}

function onDrop(e: DragEvent) {
  if (editable(e.target) || auth.status !== 'ready') return
  const text = dropped(e)
  if (text && open(text)) e.preventDefault()
}

let unlisten: (() => void) | null = null
let stopped = false
onMounted(() => {
  document.addEventListener('paste', onPaste)
  window.addEventListener('dragover', onDragOver)
  window.addEventListener('drop', onDrop)
  if (bridge.kind !== 'tauri') return
  // Ссылку забираем у оболочки: так она не обработается дважды и не теряется при запуске ею.
  // До входа не забираем — ссылка подождёт, пока человек войдёт
  void Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')]).then(async ([core, event]) => {
    const take = async () => {
      if (auth.status !== 'ready') return
      const url = await core.invoke<string | null>('links_take').catch(() => null)
      if (url) open(url)
    }
    const off = await event.listen('deep-link:open', () => void take())
    if (stopped) off()
    else unlisten = off
    watch(() => auth.status, () => void take(), { immediate: true })
  }).catch(() => {})
})
onBeforeUnmount(() => {
  stopped = true
  unlisten?.()
  document.removeEventListener('paste', onPaste)
  window.removeEventListener('dragover', onDragOver)
  window.removeEventListener('drop', onDrop)
})
</script>

<template>
  <UiModal v-model:open="shown" :title="title" :width="400">
    <div class="sl">
      <template v-if="state?.status === 'loading'">
        <div class="sl__item" aria-busy="true">
          <UiSkeleton class="sl__art" />
          <div class="sl__text">
            <UiSkeleton variant="line" width="70%" />
            <UiSkeleton variant="line" width="45%" />
          </div>
        </div>
        <p class="sl__status">{{ t('looking') }}</p>
      </template>

      <template v-else-if="state?.status === 'error'">
        <UiEmptyState icon="ph:link-break" :title="t(`error.${state.error}`)" :text="t(`errorText.${state.error}`)" compact />
      </template>

      <template v-else-if="track">
        <div class="sl__item">
          <MusicCoverArt :image="track.album?.image" size="small" :alt="track.album?.title ?? ''" class="sl__art" />
          <div class="sl__text">
            <div class="sl__title ellipsis">{{ track.title }}</div>
            <div class="sl__sub ellipsis">{{ track.artists.map(a => a.name).join(', ') }}</div>
          </div>
        </div>
        <UiButton variant="primary" icon="ph:play-fill" block size="lg" @click="playTrack">{{ t('play') }}</UiButton>
        <div class="sl__row">
          <UiButton :icon="liked ? 'ph:heart-fill' : 'ph:heart'" block @click="player.toggleLike(track)">{{ liked ? t('liked') : t('like') }}</UiButton>
          <UiButton icon="ph:plus-circle" block @click="addToPlaylist">{{ t('toPlaylist') }}</UiButton>
        </div>
      </template>

      <template v-else-if="album">
        <div class="sl__item">
          <MusicCoverArt :image="album.image" size="small" :alt="album.title" class="sl__art" />
          <div class="sl__text">
            <div class="sl__title ellipsis">{{ album.title }}</div>
            <div class="sl__sub ellipsis">{{ album.artist.name }}</div>
          </div>
        </div>
        <UiButton variant="primary" icon="ph:play-fill" block size="lg" :loading="albumLoading === `album:${album.id}`" @click="playWholeAlbum(album.id)">{{ t('play') }}</UiButton>
        <UiButton icon="ph:disc" block @click="openAlbum(album.id)">{{ t('openAlbum') }}</UiButton>
      </template>
    </div>
  </UiModal>
</template>

<style scoped>
.sl { display: flex; flex-direction: column; gap: 12px; padding: 4px 20px 20px; }
.sl__item { display: flex; align-items: center; gap: 14px; min-height: 64px; margin-bottom: 4px; }
.sl__art { flex: none; width: 64px; height: 64px; border-radius: var(--r-2); }
.sl__text { display: flex; flex-direction: column; gap: 6px; min-width: 0; flex: 1; }
.sl__title { font-size: var(--fs-h4); font-weight: 650; line-height: 1.25; }
.sl__sub { color: var(--text-2); }
.sl__status { margin: 0; color: var(--text-3); font-size: var(--fs-caption); }
.sl__row { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
</style>
