<script setup lang="ts">
import type { ImageSet, Track } from '~/api/types'

// «Распознать»: экран прослушивания во весь экран (спокойно пульсирующий круг) и лист результата.
// Один на приложение — в AppShell. Открыть просят кнопка в поиске, трей macOS, быстрое действие
// на значке iPhone (bridge.recognizer.onOpen)
const bridge = useBridge()
const rec = useRecognizeStore()
const player = usePlayerStore()
const library = useLibraryStore()
const pui = usePlaylistUi()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })

let offOpen: (() => void) | null = null
onMounted(() => {
  offOpen = bridge.recognizer?.onOpen(() => void rec.start()) ?? null
})
onBeforeUnmount(() => offOpen?.())

const overlay = computed(() => rec.phase !== 'idle')
const pulsing = computed(() => rec.phase === 'listening')

// Esc закрывает экран прослушивания (на компьютере)
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && overlay.value) rec.cancel()
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

const title = computed(() => {
  switch (rec.phase) {
    case 'listening': return t('listening')
    case 'resolving': return rec.heard?.title ?? t('found')
    case 'nomatch': return t('noMatch')
    case 'error': return t(`fail.${rec.failure ?? 'shazam'}.title`)
    default: return ''
  }
})
const sub = computed(() => {
  switch (rec.phase) {
    case 'resolving': return rec.heard?.artist ?? ''
    case 'nomatch': return t('closer')
    case 'error': return t(`fail.${rec.failure ?? 'shazam'}.text`)
    default: return ''
  }
})
const retry = computed(() => rec.phase === 'nomatch' || (rec.phase === 'error' && rec.failure !== 'unsupported'))

// Лист результата
const r = computed(() => rec.result)
const track = computed<Track | null>(() => r.value?.track ?? null)
const cover = computed<ImageSet | null>(() => {
  if (track.value?.album?.image) return track.value.album.image
  const url = r.value?.artworkUrl
  return url ? { small: url, medium: url, large: url } : null
})
const name = computed(() => track.value?.title ?? r.value?.title ?? '')
const artists = computed(() => (track.value ? track.value.artists.map(a => a.name).join(', ') : r.value?.artist ?? ''))
const liked = computed(() => (track.value ? library.isLiked(track.value) : false))
const playing = computed(() => !!track.value && player.current?.track.id === track.value.id && player.playing)

function close() {
  rec.sheetOpen = false
}
function listen() {
  const tr = track.value
  if (!tr) return
  if (player.current?.track.id === tr.id) void player.toggle()
  else void player.playTracks([tr], 0, { type: 'search', id: 'recognized', title: t('recognized') })
  close()
}
function like() {
  if (track.value) void player.toggleLike(track.value)
}
function toPlaylist() {
  if (!track.value) return
  const tr = track.value
  close()
  pui.pick([tr])
}
function wave() {
  const tr = track.value
  if (!tr) return
  close()
  void player.startWave(undefined, { seed: { type: 'track', id: tr.id }, title: tr.title })
}
function searchIt() {
  const q = [r.value?.artist, r.value?.title].filter(Boolean).join(' ')
  close()
  void ui.navigate(`/search?q=${encodeURIComponent(q)}`)
}
</script>

<template>
  <Teleport to="body">
    <Transition name="rcg">
      <div v-if="overlay" class="rcg" role="dialog" aria-modal="true" :aria-label="t('aria')">
        <div class="rcg__center">
          <div class="rcg__orb" :class="{ 'is-pulsing': pulsing, 'is-still': rec.phase === 'nomatch' || rec.phase === 'error' }" aria-hidden="true">
            <span class="rcg__ring" />
            <span class="rcg__ring rcg__ring--2" />
            <span class="rcg__ring rcg__ring--3" />
            <span class="rcg__core"><Icon :name="rec.phase === 'error' && rec.failure === 'permission' ? 'ph:microphone-slash' : 'ph:microphone'" /></span>
          </div>
          <div class="rcg__text" aria-live="polite">
            <h2 class="rcg__title ellipsis">{{ title }}</h2>
            <p v-if="sub" class="rcg__sub ellipsis">{{ sub }}</p>
          </div>
        </div>
        <div class="rcg__actions">
          <template v-if="retry">
            <UiButton variant="primary" size="lg" block @click="rec.start()">{{ t('again') }}</UiButton>
            <UiButton variant="ghost" size="lg" block @click="rec.cancel()">{{ t('close') }}</UiButton>
          </template>
          <UiButton v-else variant="secondary" size="lg" block @click="rec.cancel()">{{ rec.phase === 'error' ? t('close') : t('cancel') }}</UiButton>
        </div>
      </div>
    </Transition>
  </Teleport>

  <UiModal v-model:open="rec.sheetOpen" :title="t('sheetTitle')" :show-title="false" :width="400">
    <div v-if="r" class="rres">
      <MusicCoverArt :image="cover" size="large" class="rres__art" eager />
      <div class="rres__name">
        <h2 class="rres__title">{{ name }}</h2>
        <p class="rres__artist ellipsis">{{ artists }}</p>
      </div>
      <template v-if="track">
        <UiButton variant="primary" size="lg" block :icon="playing ? 'ph:pause-fill' : 'ph:play-fill'" @click="listen">{{ playing ? t('pause') : t('play') }}</UiButton>
        <div class="rres__row">
          <button type="button" class="rres__act" :class="{ 'is-on': liked }" :aria-pressed="liked" @click="like">
            <Icon :name="liked ? 'ph:heart-fill' : 'ph:heart'" class="rres__icon" />
            <span>{{ liked ? t('liked') : t('like') }}</span>
          </button>
          <button type="button" class="rres__act" @click="toPlaylist">
            <Icon name="ph:plus-circle" class="rres__icon" />
            <span>{{ t('playlist') }}</span>
          </button>
          <button type="button" class="rres__act" @click="wave">
            <Icon name="ph:waveform" class="rres__icon" />
            <span>{{ t('wave') }}</span>
          </button>
        </div>
      </template>
      <template v-else>
        <p class="rres__missing">{{ t('missing') }}</p>
        <UiButton variant="secondary" size="lg" block icon="ph:magnifying-glass" @click="searchIt">{{ t('search') }}</UiButton>
      </template>
    </div>
  </UiModal>
</template>

<style scoped>
.rcg {
  position: fixed;
  inset: 0;
  z-index: 190;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: calc(var(--titlebar-h, 0px) + var(--safe-top) + 16px) 24px calc(var(--safe-bottom) + 24px);
  background: var(--ink-1);
}
.rcg__center { display: flex; flex: 1; flex-direction: column; align-items: center; justify-content: center; gap: 40px; width: 100%; max-width: 420px; }
.rcg__text { display: flex; flex-direction: column; align-items: center; gap: 8px; width: 100%; min-height: 64px; text-align: center; }
.rcg__title { max-width: 100%; margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.2; }
.rcg__sub { max-width: 100%; margin: 0; font-size: var(--fs-body); color: var(--text-2); }
.rcg__actions { display: flex; flex-direction: column; gap: 8px; width: 100%; max-width: 360px; }

/* Круг: плоская заливка акцентом, вокруг — кольца, медленно расходятся и тают */
.rcg__orb { position: relative; display: grid; place-items: center; width: 248px; height: 248px; }
.rcg__core {
  position: relative;
  z-index: 1;
  display: grid;
  place-items: center;
  width: 128px;
  height: 128px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--on-light);
  font-size: 52px;
  transition: background-color var(--dur-3), color var(--dur-3);
}
.rcg__ring { position: absolute; inset: 60px; border-radius: 50%; background: var(--accent); opacity: 0; }
.rcg__orb.is-pulsing .rcg__core { animation: rcg-breathe 2.4s var(--ease-in-out) infinite; }
.rcg__orb.is-pulsing .rcg__ring { animation: rcg-ring 3.6s cubic-bezier(0.2, 0.6, 0.35, 1) infinite; }
.rcg__orb.is-pulsing .rcg__ring--2 { animation-delay: 1.2s; }
.rcg__orb.is-pulsing .rcg__ring--3 { animation-delay: 2.4s; }
.rcg__orb.is-still .rcg__core { background: var(--fill-3); color: var(--text-1); }
@keyframes rcg-ring {
  0% { opacity: 0.22; transform: scale(1); }
  100% { opacity: 0; transform: scale(1.9); }
}
@keyframes rcg-breathe {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.04); }
}
@media (prefers-reduced-motion: reduce) {
  .rcg__orb.is-pulsing .rcg__core, .rcg__orb.is-pulsing .rcg__ring { animation: none; }
  .rcg__orb.is-pulsing .rcg__ring { opacity: 0.12; transform: scale(1.35); }
}

.rcg-enter-active, .rcg-leave-active { transition: opacity var(--dur-3) var(--ease-out); }
.rcg-enter-from, .rcg-leave-to { opacity: 0; }

/* Лист результата */
.rres { display: flex; flex-direction: column; align-items: center; gap: 20px; padding: 8px 8px 16px; }
.rres__art { width: min(240px, 64vw); border-radius: var(--r-3); box-shadow: var(--shadow-cover); }
.rres__name { display: flex; flex-direction: column; align-items: center; gap: 4px; width: 100%; text-align: center; }
.rres__title { display: -webkit-box; overflow: hidden; margin: 0; font-size: 22px; font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.25; -webkit-box-orient: vertical; -webkit-line-clamp: 2; }
.rres__artist { max-width: 100%; margin: 0; font-size: var(--fs-body); color: var(--text-2); }
.rres__missing { margin: 0; font-size: var(--fs-body); color: var(--text-2); }
.rres__row { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; width: 100%; }
.rres__act {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 72px;
  padding: 10px 4px;
  border: 0;
  border-radius: var(--r-3);
  background: var(--fill-1);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 550;
  color: var(--text-1);
  transition: background-color var(--dur-1);
}
.rres__act:hover { background: var(--fill-2); }
.rres__act:active { background: var(--fill-3); }
.rres__act:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.rres__act.is-on .rres__icon { color: var(--accent); }
.rres__icon { font-size: 24px; }
.rres__act span { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>

<i18n lang="json">
{
  "ru": {
    "aria": "Распознать песню",
    "listening": "Слушаю…",
    "found": "Нашли",
    "noMatch": "Не узнали",
    "closer": "Попробуйте ближе к звуку",
    "again": "Ещё раз",
    "cancel": "Отменить",
    "close": "Закрыть",
    "fail": {
      "permission": { "title": "Нет доступа к микрофону", "text": "Разрешите его в настройках системы" },
      "no_microphone": { "title": "Микрофон не найден", "text": "Подключите микрофон и попробуйте ещё раз" },
      "shazam": { "title": "Shazam не ответил", "text": "Проверьте интернет и попробуйте ещё раз" },
      "network": { "title": "Не получилось найти трек", "text": "Проверьте интернет и попробуйте ещё раз" },
      "unsupported": { "title": "Здесь распознавания нет", "text": "Оно есть в приложении для iPhone и Mac" }
    },
    "sheetTitle": "Распознанная песня",
    "recognized": "Распознанное",
    "play": "Слушать",
    "pause": "Пауза",
    "like": "В любимые",
    "liked": "В любимых",
    "playlist": "В плейлист",
    "wave": "Волна по треку",
    "missing": "Этой песни нет в каталоге",
    "search": "Искать"
  },
  "en": {
    "aria": "Recognize a song",
    "listening": "Listening…",
    "found": "Found it",
    "noMatch": "No match",
    "closer": "Try getting closer to the sound",
    "again": "Try again",
    "cancel": "Cancel",
    "close": "Close",
    "fail": {
      "permission": { "title": "No microphone access", "text": "Allow it in system settings" },
      "no_microphone": { "title": "No microphone", "text": "Connect a microphone and try again" },
      "shazam": { "title": "Shazam didn't respond", "text": "Check your connection and try again" },
      "network": { "title": "Couldn't find the song", "text": "Check your connection and try again" },
      "unsupported": { "title": "Not available here", "text": "Use the iPhone or Mac app" }
    },
    "sheetTitle": "Recognized song",
    "recognized": "Recognized",
    "play": "Play",
    "pause": "Pause",
    "like": "Like",
    "liked": "Liked",
    "playlist": "Add to playlist",
    "wave": "Song radio",
    "missing": "This song isn't in the catalog",
    "search": "Search"
  }
}
</i18n>
