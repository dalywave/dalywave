<script setup lang="ts">
import type { TvCode } from '~/api/types'

// Режим вечеринки — сцена для телевизора и проектора (и AirPlay-зеркалирования): живая волна, обложка,
// крупно название и исполнитель, текст песни «сценой», следующий трек, QR «Добавить трек» для гостей
// «Слушать вместе» и имена тех, кто добавил трек. Читается с дивана: кегль — от высоты экрана.
// Управление прячется само и появляется от мыши или клавиш. Телевизор без входа показывает код и QR:
// подтверждают на телефоне (/tv), браузер телевизора получает свою сессию — строку в «Доступе».
definePageMeta({ layout: 'bare' })

const auth = useAuthStore()
const bridge = useBridge()
const ui = useUiStore()
const player = usePlayerStore()
const connect = useConnectStore()
const jam = useJamStore()
const party = usePartyStore()
const { t } = useScopedI18n('pages.party')
const { tg } = useGlobalT()

useHead({ title: () => t('title') })

const signedIn = computed(() => auth.status === 'ready' || auth.status === 'offline')

// ---------- Телевизор без входа: код на экране ----------
const code = ref<TvCode | null>(null)
const codeFailed = ref(false)
let pollTimer: ReturnType<typeof setTimeout> | undefined
let alive = true

async function newCode() {
  clearTimeout(pollTimer)
  try {
    code.value = await bridge.party.code()
    codeFailed.value = false
    schedulePoll()
  }
  catch {
    codeFailed.value = true
    pollTimer = setTimeout(() => void newCode(), 10_000)
  }
}

function schedulePoll() {
  const c = code.value
  if (!c || !alive) return
  pollTimer = setTimeout(() => void poll(c), c.intervalSecs * 1000)
}

async function poll(c: TvCode) {
  if (!alive || code.value !== c) return
  if (Date.parse(c.expiresAt) < Date.now()) return void newCode()
  try {
    const res = await bridge.party.token(c.deviceCode)
    if (res.status === 'signed_in') {
      code.value = null
      await auth.load({ force: true })
      if (signedIn.value) startStage()
      return
    }
  }
  catch (e) {
    // Код истёк или уже использован — новый
    if ((e as { status?: number }).status === 400) return void newCode()
  }
  schedulePoll()
}

// ---------- Сцена ----------
const track = computed(() => player.current?.track ?? null)
const next = computed(() => player.nextEntry?.track ?? null)
const artists = (tr: { artists: { name: string }[] }) => tr.artists.map(a => a.name).join(', ')
const addedBy = computed(() => (track.value ? jam.addedBy.get(track.value.id) : undefined))
const nextBy = computed(() => (next.value ? jam.addedBy.get(next.value.id) : undefined))
const jamUrl = computed(() => jam.jam?.url ?? '')
const shortUrl = computed(() => jamUrl.value.replace(/^https?:\/\//, ''))
// Сначала те, кто сейчас со своим телефоном
const guests = computed(() => [...jam.guests].sort((a, b) => Number(b.online) - Number(a.online)).slice(0, 8))
const moreGuests = computed(() => Math.max(0, jam.guests.length - guests.value.length))

const trackId = computed(() => track.value?.id ?? null)
const { state: lyricsState } = useTrackLyrics(trackId)
const hasLyrics = computed(() => {
  const s = lyricsState.value
  return s.status === 'ready' && ((s.data.kind === 'synced' && s.data.lines.length > 0) || (s.data.kind === 'plain' && !!s.data.plain))
})
/** Звук играет не в этой вкладке: можно взять его сюда (колонки телевизора) */
const elsewhere = computed(() => connect.remote)

let stageStarted = false
function startStage() {
  if (stageStarted) return
  stageStarted = true
  bridge.live.connect()
  connect.start()
  party.init()
  // Сцена открыта — вечеринка идёт: открываем «Слушать вместе» для QR
  void party.load().then(() => {
    if (!party.active) void party.start()
    else void jam.load()
  })
}

// ---------- Управление: прячется само ----------
const awake = ref(true)
const settingsOpen = ref(false)
let idleTimer: ReturnType<typeof setTimeout> | undefined
function wake() {
  awake.value = true
  clearTimeout(idleTimer)
  idleTimer = setTimeout(() => {
    if (!settingsOpen.value) awake.value = false
  }, 3500)
}

const { isFullscreen, toggle: toggleFullscreen } = useFullscreen()

function onKey(e: KeyboardEvent) {
  wake()
  if (!signedIn.value || e.target instanceof HTMLInputElement) return
  if (e.key === ' ' || e.key === 'MediaPlayPause') {
    e.preventDefault()
    player.toggle()
  }
  else if (e.key === 'ArrowRight' || e.key === 'MediaTrackNext') player.next('skipped')
  else if (e.key === 'f') void toggleFullscreen()
  else if (e.key === 'Escape' && settingsOpen.value) settingsOpen.value = false
}

async function finish() {
  await party.stop()
  if (isFullscreen.value) void toggleFullscreen()
  await navigateTo('/')
}

const energetic = computed({ get: () => party.settings.energetic, set: v => void party.update({ energetic: v }) })
const noRepeat = computed({ get: () => party.settings.noArtistRepeat, set: v => void party.update({ noArtistRepeat: v }) })
const dj = computed({ get: () => party.settings.dj, set: v => void party.update({ dj: v }) })

onMounted(() => {
  wake()
  window.addEventListener('keydown', onKey)
  if (signedIn.value) startStage()
  else void newCode()
})
watch(signedIn, (v) => {
  if (v) startStage()
})
onBeforeUnmount(() => {
  alive = false
  clearTimeout(pollTimer)
  clearTimeout(idleTimer)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <main class="party scheme-dark" :class="{ 'is-idle': !awake, 'is-still': ui.reducedMotion }" @pointermove="wake" @pointerdown="wake">
    <!-- Телевизор без входа -->
    <section v-if="!signedIn" class="pair">
      <BrandLogo class="pair__logo" />
      <h1 class="pair__title">{{ t('pairTitle') }}</h1>
      <template v-if="code">
        <div class="pair__body">
          <MusicJamQr class="pair__qr" :value="code.verificationUrlComplete" :label="t('pairQr')" />
          <div class="pair__text">
            <p class="pair__step">{{ t('pairScan') }}</p>
            <p class="pair__step pair__step--dim">{{ t('pairOr', { url: code.verificationUrl.replace(/^https?:\/\//, '') }) }}</p>
            <div class="pair__code" :aria-label="code.userCode.split('').join(' ')">{{ code.userCode }}</div>
          </div>
        </div>
      </template>
      <p v-else-if="codeFailed" class="pair__step pair__step--dim">{{ t('pairOffline') }}</p>
    </section>

    <!-- Сцена -->
    <template v-else>
      <div class="party__wave"><MusicWaveCanvas layout="full" dark /></div>

      <div v-if="track" class="stage" :class="{ 'stage--lyrics': hasLyrics }">
        <div class="stage__now">
          <div class="stage__art"><MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager /></div>
          <div class="stage__info">
            <h1 class="stage__title">{{ track.title }}</h1>
            <p class="stage__artist">{{ artists(track) }}</p>
            <p v-if="addedBy" class="stage__by"><Icon name="ph:user-circle-plus" />{{ t('addedBy', { name: addedBy }) }}</p>
          </div>
        </div>
        <div v-if="hasLyrics" class="stage__lyrics">
          <MusicLyricsView variant="stage" :anchor="0.38" />
        </div>
      </div>
      <div v-else class="stage stage--empty">
        <h1 class="stage__title">{{ t('silenceTitle') }}</h1>
        <p class="stage__artist">{{ t('silenceHint') }}</p>
      </div>

      <footer class="deck">
        <div class="deck__next">
          <template v-if="next">
            <span class="deck__label">{{ t('next') }}</span>
            <div class="deck__track">
              <span class="deck__art"><MusicCoverArt :image="next.album?.image" size="small" :alt="''" /></span>
              <span class="deck__text">
                <span class="deck__name ellipsis">{{ next.title }}</span>
                <span class="deck__meta ellipsis">{{ artists(next) }}<template v-if="nextBy"> · {{ t('addedBy', { name: nextBy }) }}</template></span>
              </span>
            </div>
          </template>
        </div>
        <div v-if="jamUrl" class="deck__join">
          <span class="deck__join-text">
            <span class="deck__join-title">{{ t('addTrack') }}</span>
            <span class="deck__join-url">{{ shortUrl }}</span>
            <!-- Гости — строкой имён: кто сейчас со своим телефоном, ярче -->
            <span v-if="guests.length" class="deck__guests" :aria-label="t('guests')">
              <template v-for="(g, i) in guests" :key="g.id"><span v-if="i" class="deck__dot"> · </span><span :class="{ 'is-away': !g.online }">{{ g.name }}</span></template>
              <span v-if="moreGuests" class="is-away"> +{{ moreGuests }}</span>
            </span>
          </span>
          <MusicJamQr class="deck__qr" :value="jamUrl" :label="t('addTrack')" />
        </div>
      </footer>

      <!-- Управление: появляется от мыши и клавиш -->
      <div class="ctl" :aria-hidden="!awake">
        <UiIconButton icon="ph:arrow-left" :label="t('back')" tone="glass" size="lg" side="right" to="/" />
        <div class="ctl__right">
          <UiButton v-if="elsewhere" variant="secondary" icon="ph:speaker-high" @click="connect.continueHere(true)">{{ t('playHere') }}</UiButton>
          <UiIconButton :icon="player.playing ? 'ph:pause-fill' : 'ph:play-fill'" :label="player.playing ? tg('common.pause') : tg('common.play')" tone="glass" size="lg" side="bottom" @click="player.toggle()" />
          <UiIconButton icon="ph:skip-forward-fill" :label="tg('player.next')" tone="glass" size="lg" side="bottom" @click="player.next('skipped')" />
          <UiIconButton icon="ph:sliders-horizontal" :label="t('settings')" tone="glass" size="lg" side="bottom" :active="settingsOpen" @click="settingsOpen = !settingsOpen" />
          <UiIconButton :icon="isFullscreen ? 'ph:corners-in' : 'ph:corners-out'" :label="isFullscreen ? t('exitFull') : t('full')" tone="glass" size="lg" side="bottom" @click="toggleFullscreen()" />
        </div>
      </div>
      <Transition name="fade">
        <div v-if="settingsOpen" class="set" role="dialog" :aria-label="t('settings')">
          <UiSwitch v-model="energetic" :label="t('energetic')" />
          <UiSwitch v-model="noRepeat" :label="t('noRepeat')" />
          <UiSwitch v-model="dj" :label="t('dj')" :disabled="!connect.djAvailable" />
          <div class="set__sep" />
          <UiButton variant="ghost" icon="ph:stop-circle" :disabled="party.busy" @click="finish">{{ t('finish') }}</UiButton>
        </div>
      </Transition>
    </template>
  </main>
</template>

<style scoped>
/* Кегль от высоты экрана: 1080p и 4K читаются одинаково с дивана */
.party {
  --u: 1vh;
  position: fixed;
  inset: 0;
  overflow: hidden;
  background: #05060a;
  color: var(--text-1);
  font-family: var(--font);
  cursor: default;
}
.party.is-idle { cursor: none; }
.party__wave { position: absolute; inset: 0; opacity: 0.75; }

/* ---------- Сцена ---------- */
.stage {
  position: absolute;
  inset: calc(var(--u) * 9) calc(var(--u) * 7) calc(var(--u) * 24);
  display: flex;
  align-items: center;
  justify-content: center;
  gap: calc(var(--u) * 7);
}
.stage__now { display: flex; align-items: center; gap: calc(var(--u) * 5); min-width: 0; }
.stage__art { width: calc(var(--u) * 46); flex: none; border-radius: calc(var(--u) * 1.6); overflow: hidden; box-shadow: 0 calc(var(--u) * 3) calc(var(--u) * 8) rgba(0, 0, 0, 0.5); }
.stage__info { display: flex; flex-direction: column; gap: calc(var(--u) * 1.4); min-width: 0; max-width: calc(var(--u) * 90); }
.stage__title { margin: 0; font-size: calc(var(--u) * 7.4); font-weight: 750; line-height: 1.04; letter-spacing: -0.02em; text-wrap: balance; overflow-wrap: anywhere; }
.stage__artist { margin: 0; font-size: calc(var(--u) * 3.6); font-weight: 500; color: var(--text-2); }
.stage__by { display: inline-flex; align-items: center; gap: calc(var(--u) * 0.8); margin: calc(var(--u) * 0.6) 0 0; font-size: calc(var(--u) * 2.4); color: var(--text-2); }
.stage__by :deep(svg) { font-size: calc(var(--u) * 2.8); color: var(--accent); }

/* С текстом: обложка и подпись — колонкой слева, текст — справа крупно */
.stage--lyrics { justify-content: space-between; align-items: stretch; }
.stage--lyrics .stage__now { flex-direction: column; align-items: flex-start; justify-content: center; width: calc(var(--u) * 52); flex: none; gap: calc(var(--u) * 3); }
.stage--lyrics .stage__art { width: calc(var(--u) * 40); }
.stage--lyrics .stage__title { font-size: calc(var(--u) * 5); }
.stage--lyrics .stage__artist { font-size: calc(var(--u) * 3); }
.stage__lyrics { position: relative; flex: 1; min-width: 0; height: 100%; container-type: inline-size; }
.stage__lyrics :deep(.ly) { height: 100%; }

.stage--empty { flex-direction: column; gap: calc(var(--u) * 2); text-align: center; }

/* ---------- Низ: следующий, гости, QR ---------- */
.deck {
  position: absolute;
  left: calc(var(--u) * 7);
  right: calc(var(--u) * 7);
  bottom: calc(var(--u) * 5);
  display: flex;
  align-items: flex-end;
  gap: calc(var(--u) * 4);
}
.deck__next { flex: 1; min-width: 0; }
.deck__label { display: block; margin-bottom: calc(var(--u) * 1.2); font-size: calc(var(--u) * 1.7); font-weight: 650; letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-3); }
.deck__track { display: flex; align-items: center; gap: calc(var(--u) * 1.8); min-width: 0; }
.deck__art { width: calc(var(--u) * 7.5); flex: none; border-radius: calc(var(--u) * 0.8); overflow: hidden; }
.deck__text { display: flex; flex-direction: column; gap: calc(var(--u) * 0.4); min-width: 0; }
.deck__name { font-size: calc(var(--u) * 2.7); font-weight: 650; }
.deck__meta { font-size: calc(var(--u) * 2.1); color: var(--text-2); }
.deck__guests { margin-top: calc(var(--u) * 0.8); max-width: calc(var(--u) * 60); font-size: calc(var(--u) * 2.1); font-weight: 600; color: var(--text-1); }
.deck__guests .is-away, .deck__dot { color: var(--text-3); font-weight: 450; }
.deck__join { display: flex; align-items: center; gap: calc(var(--u) * 2); flex: none; }
.deck__join-text { display: flex; flex-direction: column; align-items: flex-end; gap: calc(var(--u) * 0.4); text-align: right; }
.deck__join-title { font-size: calc(var(--u) * 2.7); font-weight: 700; }
.deck__join-url { font-size: calc(var(--u) * 1.8); color: var(--text-3); }
.deck__qr { display: block; width: calc(var(--u) * 17); padding: 6%; border-radius: calc(var(--u) * 1.4); }

/* ---------- Управление ---------- */
.ctl {
  position: absolute;
  top: calc(var(--u) * 3);
  left: calc(var(--u) * 4);
  right: calc(var(--u) * 4);
  display: flex;
  align-items: center;
  justify-content: space-between;
  transition: opacity var(--dur-3) var(--ease-out);
}
.ctl__right { display: flex; align-items: center; gap: 10px; }
.is-idle .ctl { opacity: 0; pointer-events: none; }
.set {
  position: absolute;
  top: calc(var(--u) * 3 + 56px);
  right: calc(var(--u) * 4);
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 320px;
  padding: 18px 18px 12px;
  border-radius: 18px;
  background: rgba(20, 22, 28, 0.92);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
}
.pair__qr :deep(svg), .deck__qr :deep(svg) { height: auto; }
.set__sep { height: 0.5px; background: rgba(255, 255, 255, 0.12); }
.fade-enter-active, .fade-leave-active { transition: opacity var(--dur-2) var(--ease-out); }
.fade-enter-from, .fade-leave-to { opacity: 0; }

/* ---------- Код для телевизора ---------- */
.pair { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: calc(var(--u) * 5); padding: calc(var(--u) * 6); }
.pair__logo { position: absolute; top: calc(var(--u) * 5); left: calc(var(--u) * 6); }
.pair__title { margin: 0; font-size: calc(var(--u) * 5.6); font-weight: 750; letter-spacing: -0.02em; text-align: center; }
.pair__body { display: flex; align-items: center; gap: calc(var(--u) * 7); }
.pair__qr { display: block; width: calc(var(--u) * 34); border-radius: calc(var(--u) * 2); }
.pair__text { display: flex; flex-direction: column; gap: calc(var(--u) * 1.6); max-width: calc(var(--u) * 70); }
.pair__step { margin: 0; font-size: calc(var(--u) * 3); font-weight: 550; }
.pair__step--dim { color: var(--text-2); font-weight: 450; font-size: calc(var(--u) * 2.6); }
.pair__code { margin-top: calc(var(--u) * 2); font-size: calc(var(--u) * 10); font-weight: 750; letter-spacing: 0.08em; font-variant-numeric: tabular-nums; }

/* Узкий экран (телефон открыл сцену): всё колонкой */
@media (max-aspect-ratio: 1/1) {
  .party { --u: 0.6vh; }
  .stage, .stage--lyrics { flex-direction: column; justify-content: center; align-items: center; gap: calc(var(--u) * 4); }
  .stage__now { flex-direction: column; text-align: center; }
  .stage--lyrics .stage__lyrics { display: none; }
  .deck { flex-direction: column; align-items: stretch; }
  .pair__body { flex-direction: column; }
}

@media (prefers-reduced-motion: reduce) {
  .ctl, .fade-enter-active, .fade-leave-active { transition: none; }
}
</style>
