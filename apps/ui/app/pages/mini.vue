<script setup lang="ts">
import { createShell, type InvokeFn, type ListenFn, type PlayerNow, type Shell } from '~/bridge/tauri/shell'

// Мини-плеер приложения для компьютера (окно apps/shell/src/mini.rs): другой вид того же плеера.
// Звук и очередь — в движке и главном окне; здесь только то, что движок сообщает событиями
// `player:*`, и кнопки-пульт. Окно тянется за любое пустое место
definePageMeta({ layout: false })

const { t } = useI18n({ useScope: 'local' })
useHead({ title: 'DalyWave', htmlAttrs: { 'data-mini': '' } })

const now = ref<PlayerNow | null>(null)
const playing = ref(false)
const position = ref(0)
const duration = ref(0)
let anchor = { at: 0, pos: 0 }
let shell: Shell | null = null
let ticker = 0
const offs: (() => void)[] = []

// Разработка в браузере: вид с примером трека (`/mini`), чтобы верстать без приложения
const DEMO: PlayerNow = { title: 'Float', artist: 'Call Me Karizma', album: 'Float', artworkUrl: null, durationMs: 169_000, positionMs: 62_000, playing: true, id: 'demo' }

function apply(n: PlayerNow | null) {
  now.value = n
  playing.value = !!n?.playing
  duration.value = n?.durationMs ?? 0
  setPosition(n?.positionMs ?? 0)
}

function setPosition(ms: number) {
  position.value = ms
  anchor = { at: performance.now(), pos: ms }
}

// Между событиями движка (раз в 250 мс) позицию досчитываем сами — ползунок едет плавно
function tick() {
  if (!playing.value) return
  const p = anchor.pos + (performance.now() - anchor.at)
  position.value = duration.value ? Math.min(p, duration.value) : p
}

onMounted(async () => {
  ticker = window.setInterval(tick, 100)
  window.addEventListener('keydown', onKey)
  if (!('__TAURI_INTERNALS__' in window)) {
    if (import.meta.dev) apply(DEMO)
    return
  }
  const [core, event] = await Promise.all([import('@tauri-apps/api/core'), import('@tauri-apps/api/event')])
  shell = createShell({ invoke: core.invoke as InvokeFn, listen: event.listen as unknown as ListenFn })
  offs.push(
    await shell.on('player:now', n => apply(n)),
    await shell.on('player:position', ({ id, positionMs, durationMs }) => {
      if (now.value?.id && id !== now.value.id) return
      if (durationMs) duration.value = durationMs
      setPosition(positionMs)
    }),
    await shell.on('player:state', ({ state }) => {
      anchor = { at: performance.now(), pos: position.value }
      playing.value = state === 'playing' || (state === 'loading' && playing.value)
    }),
    // Сменился трек: подробности — в `player:now`, а позиция — с начала сразу
    await shell.on('player:track', ({ id, durationMs }) => {
      if (now.value) now.value = { ...now.value, id }
      if (durationMs) duration.value = durationMs
      setPosition(0)
    }),
  )
  apply(await shell.player.now().catch(() => null))
})

onBeforeUnmount(() => {
  clearInterval(ticker)
  window.removeEventListener('keydown', onKey)
  offs.forEach(off => off())
})

function toggle() {
  playing.value = !playing.value
  anchor = { at: performance.now(), pos: position.value }
  void shell?.player.remote('toggle')
}

function seek(ms: number) {
  setPosition(ms)
  void shell?.player.seek(ms)
}

function backToMain() {
  void shell?.mini.close()
}

function onKey(e: KeyboardEvent) {
  const mod = e.metaKey || e.ctrlKey
  if (mod && e.shiftKey && e.key.toLowerCase() === 'm') {
    e.preventDefault()
    backToMain()
  }
  else if (e.code === 'Space' && !(e.target as HTMLElement).closest('button, [role="slider"]')) {
    e.preventDefault()
    toggle()
  }
}
</script>

<template>
  <main class="mini" data-tauri-drag-region="deep">
    <div class="mini__row" data-tauri-drag-region="deep">
      <div class="mini__art">
        <img v-if="now?.artworkUrl" :src="now.artworkUrl" alt="" draggable="false">
        <span v-else class="mini__ph"><Icon name="ph:music-notes-simple" /></span>
      </div>
      <div class="mini__body" data-tauri-drag-region="deep">
        <div class="mini__meta" data-tauri-drag-region="deep">
          <div class="mini__title ellipsis">{{ now?.title ?? t('nothing') }}</div>
          <div v-if="now" class="mini__artist ellipsis">{{ now.artist }}</div>
        </div>
        <div class="mini__transport">
          <UiIconButton icon="ph:skip-back-fill" :label="t('prev')" size="sm" :tooltip="false" :disabled="!now" @click="shell?.player.remote('previous')" />
          <MusicPlayButton :playing="playing" :size="34" :disabled="!now" :label="playing ? t('pause') : t('play')" @click="toggle" />
          <UiIconButton icon="ph:skip-forward-fill" :label="t('next')" size="sm" :tooltip="false" :disabled="!now" @click="shell?.player.remote('next')" />
        </div>
      </div>
      <span class="mini__back"><UiIconButton icon="ph:arrows-out-simple" :label="t('back')" size="xs" :tooltip="false" @click="backToMain" /></span>
    </div>
    <MusicScrubber
      class="mini__scrub"
      :peaks="[]"
      variant="line"
      :position-ms="position"
      :duration-ms="duration"
      :disabled="!now"
      :label="t('seek')"
      @seek="seek"
    />
  </main>
</template>

<style scoped>
.mini {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 6px;
  height: 100vh;
  padding: 14px 14px 8px;
  overflow: hidden;
  background: var(--ink-2);
  user-select: none;
  -webkit-user-select: none;
  cursor: default;
}
.mini__row { position: relative; display: flex; align-items: center; gap: 14px; min-width: 0; }
.mini__art { position: relative; width: 76px; height: 76px; flex: none; overflow: hidden; border-radius: var(--r-2); background: var(--ink-4); box-shadow: inset 0 0 0 1px var(--hair); }
.mini__art img { width: 100%; height: 100%; object-fit: cover; pointer-events: none; }
.mini__ph { display: grid; place-items: center; width: 100%; height: 100%; font-size: 26px; color: var(--text-4); }
.mini__body { display: flex; flex: 1; flex-direction: column; gap: 6px; min-width: 0; }
.mini__meta { display: flex; flex-direction: column; gap: 1px; min-width: 0; padding-right: 28px; }
.mini__title { font-size: var(--fs-body); font-weight: 650; color: var(--text-1); }
.mini__artist { font-size: var(--fs-small); color: var(--text-2); }
.mini__transport { display: flex; align-items: center; gap: 6px; margin-left: -6px; }
.mini__back { position: absolute; top: -8px; right: -8px; opacity: 0; transition: opacity var(--dur-1); }
.mini:hover .mini__back, .mini__back:focus-within { opacity: 1; }
.mini__scrub { flex: none; }
.mini__scrub :deep(.scrub__time) { min-width: 0; width: auto; font-size: var(--fs-micro); }
</style>

<style>
html[data-mini], html[data-mini] body { background: var(--ink-2); overflow: hidden; }
html[data-mini] .ambient { display: none; }
</style>

<i18n lang="json">
{
  "ru": {
    "nothing": "Ничего не играет",
    "prev": "Предыдущий",
    "next": "Следующий",
    "play": "Слушать",
    "pause": "Пауза",
    "seek": "Позиция в треке",
    "back": "Открыть DalyWave"
  },
  "en": {
    "nothing": "Nothing playing",
    "prev": "Previous",
    "next": "Next",
    "play": "Play",
    "pause": "Pause",
    "seek": "Playback position",
    "back": "Open DalyWave"
  }
}
</i18n>
