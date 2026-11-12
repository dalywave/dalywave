<script setup lang="ts">
import type { JamGuestView, JamInvite, JamNow, JamQueueItem, JamRequest, JamStream, JamTrack } from '~/api/types'
import type { JamGuestClient } from '~/bridge/types'

// «Слушать вместе» — страница гостя по ссылке /jam/{code}. Аккаунт не нужен: имя → токен этой
// сессии (в localStorage этого браузера). Что играет у хозяина и общая очередь, поиск по каталогу,
// «Добавить в очередь», ❤, голос «пропустить» и «Слушать у себя» — тот же трек с той же позиции.
definePageMeta({ layout: 'bare' })

const route = useRoute()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const code = computed(() => String(route.params.code ?? ''))
const storeKey = computed(() => `dw.jam.${code.value}`)

type Phase = 'loading' | 'join' | 'in' | 'ended' | 'removed'
const phase = ref<Phase>('loading')
const invite = ref<JamInvite | null>(null)
const view = ref<JamGuestView | null>(null)
const now = ref<JamNow | null>(null)
const name = ref('')
const joining = ref(false)
let client: JamGuestClient | null = null
/** Время сервера минус время здесь, мс */
let offset = 0

useHead({ title: () => (invite.value ? t('titleHost', { name: invite.value.hostName }) : t('title')) })

function readToken(): string | null {
  try {
    return localStorage.getItem(storeKey.value)
  }
  catch {
    return null
  }
}
function writeToken(token: string | null) {
  try {
    if (token) localStorage.setItem(storeKey.value, token)
    else localStorage.removeItem(storeKey.value)
  }
  catch {
    // Приватный режим: токен живёт до закрытия вкладки
  }
}

function codeOf(e: unknown): string | undefined {
  return (e as { code?: string })?.code
}

/** Ответ сервера: сессия закончилась или гостя убрали */
function lost(e: unknown): boolean {
  const c = codeOf(e)
  if (c === 'jam_ended' || c === 'unauthorized') {
    leaveLocal('ended')
    return true
  }
  if (c === 'jam_removed') {
    leaveLocal('removed')
    return true
  }
  return false
}

function setNow(n: JamNow | null) {
  if (n) offset = Date.parse(n.at) - Date.now()
  now.value = n
}

async function refresh() {
  if (!client) return
  try {
    const v = await client.view()
    view.value = v
    setNow(v.now)
  }
  catch (e) {
    lost(e)
  }
}

function enter(token: string) {
  client = bridge.jam.guest(token)
  const live = client.live
  live.on('jam.now', (e) => {
    setNow(e.now)
    syncAudio()
  })
  live.on('jam.changed', (e) => {
    if (e.ended) return leaveLocal('ended')
    if (e.removed && e.removed === view.value?.me.id) return leaveLocal('removed')
    void refresh()
  })
  live.on('jam.reaction', e => burst(e.guestName))
  live.onReconnect(() => void refresh())
  live.connect()
  phase.value = 'in'
  void refresh()
}

function leaveLocal(to: 'ended' | 'removed') {
  client?.live.disconnect()
  client = null
  writeToken(null)
  stopListening()
  phase.value = to
}

onMounted(async () => {
  const token = readToken()
  try {
    invite.value = await bridge.jam.invite(code.value)
    if (invite.value.memberName) name.value = invite.value.memberName
  }
  catch {
    phase.value = 'ended'
    return
  }
  if (token) enter(token)
  else phase.value = 'join'
})
onBeforeUnmount(() => {
  client?.live.disconnect()
  stopListening()
})

async function join() {
  if (joining.value) return
  const n = name.value.trim()
  if (!n && !invite.value?.memberName) return
  joining.value = true
  try {
    const res = await bridge.jam.join(code.value, n || null)
    writeToken(res.token)
    enter(res.token)
  }
  catch (e) {
    const c = codeOf(e)
    if (c === 'jam_ended') phase.value = 'ended'
    else toast.error(c === 'too_many_requests' ? t('slowDown') : t('joinFailed'))
  }
  finally {
    joining.value = false
  }
}

async function leave() {
  try {
    await client?.leave()
  }
  catch {
    // Всё равно уходим
  }
  writeToken(null)
  client?.live.disconnect()
  client = null
  stopListening()
  phase.value = 'join'
  view.value = null
}

// ---------- Что играет: позиция идёт сама ----------

const tick = ref(Date.now())
let timer: ReturnType<typeof setInterval> | undefined
onMounted(() => {
  timer = setInterval(() => {
    tick.value = Date.now()
    syncAudio()
  }, 500)
})
onBeforeUnmount(() => clearInterval(timer))

function positionOf(n: JamNow | null, at = tick.value): number {
  if (!n) return 0
  if (!n.playing) return n.positionMs
  const pos = n.positionMs + Math.max(0, at + offset - Date.parse(n.at))
  return n.durationMs ? Math.min(n.durationMs, pos) : pos
}

const current = computed(() => now.value?.current ?? null)
const progress = computed(() => {
  const n = now.value
  if (!n?.durationMs) return 0
  return Math.min(1, positionOf(n) / n.durationMs)
})
const elapsed = computed(() => formatDuration(positionOf(now.value)))
const total = computed(() => formatDuration(now.value?.durationMs ?? 0))
const artists = (tr: JamTrack) => tr.artists.join(', ')
const titleOf = (tr: JamTrack) => (tr.version ? `${tr.title} (${tr.version})` : tr.title)

// ---------- Очередь и свои треки ----------

const queue = computed<JamQueueItem[]>(() => now.value?.queue ?? [])
const mine = computed<JamRequest[]>(() => view.value?.mine ?? [])
const waiting = computed(() => mine.value.filter(r => r.status === 'pending'))
const inQueue = computed(() => new Set([current.value?.track.id, ...queue.value.map(q => q.track.id)].filter(Boolean) as string[]))
function stateOf(id: string): 'queued' | 'pending' | null {
  if (mine.value.some(r => r.track.id === id && r.status === 'pending')) return 'pending'
  if (inQueue.value.has(id) || mine.value.some(r => r.track.id === id && r.status === 'added')) return 'queued'
  return null
}
const canAdd = computed(() => !!view.value?.settings.guestsCanAdd)
const guests = computed(() => view.value?.guests ?? [])
const guestsLabel = computed(() => {
  const n = guests.value.length
  if (lang.value === 'en') return n === 1 ? '1 guest' : `${n} guests`
  return `${n}${NBSP}${plural(n, ['гость', 'гостя', 'гостей'])}`
})

// ---------- Поиск ----------

const q = ref('')
const results = ref<JamTrack[]>([])
const searching = ref(false)
const searched = ref('')
let searchTimer: ReturnType<typeof setTimeout> | undefined
watch(q, (value) => {
  clearTimeout(searchTimer)
  const text = value.trim()
  if (text.length < 2) {
    results.value = []
    searched.value = ''
    return
  }
  searchTimer = setTimeout(() => void search(text), 350)
})

async function search(text: string) {
  if (!client) return
  searching.value = true
  try {
    const items = await client.search(text)
    if (q.value.trim() === text) {
      results.value = items
      searched.value = text
    }
  }
  catch (e) {
    if (!lost(e)) toast.error(codeOf(e) === 'too_many_requests' ? t('slowDown') : t('searchFailed'))
  }
  finally {
    searching.value = false
  }
}

const adding = ref<string | null>(null)
async function add(tr: JamTrack) {
  if (!client || adding.value) return
  adding.value = tr.id
  try {
    const r = await client.add(tr.id)
    bridge.haptic('success')
    toast.show({ text: r.status === 'pending' ? t('sentForApproval') : t('added', { title: tr.title }), icon: r.status === 'pending' ? 'ph:hourglass' : 'ph:check-circle' })
    view.value = view.value ? { ...view.value, mine: [r, ...view.value.mine] } : view.value
  }
  catch (e) {
    if (lost(e)) return
    const c = codeOf(e)
    const text = c === 'already_queued'
      ? t('alreadyQueued')
      : c === 'nothing_playing'
        ? t('nothingPlaying')
        : c === 'too_many_pending'
          ? t('tooManyPending')
          : c === 'forbidden'
            ? t('addingOff')
            : c === 'explicit_off'
              ? t('explicitOff')
              : c === 'too_many_requests' ? t('slowDown') : t('addFailed')
    toast.error(text)
  }
  finally {
    adding.value = null
  }
}

// ---------- ❤ и «пропустить» ----------

const hearts = ref<{ id: number, name: string, x: number }[]>([])
let heartId = 0
function burst(who: string) {
  const id = ++heartId
  hearts.value.push({ id, name: who, x: 20 + Math.random() * 60 })
  setTimeout(() => (hearts.value = hearts.value.filter(h => h.id !== id)), 2200)
}

async function react() {
  if (!client) return
  bridge.haptic('light')
  try {
    await client.react()
  }
  catch (e) {
    if (!lost(e) && codeOf(e) === 'too_many_requests') toast.show({ text: t('slowDown'), icon: 'ph:heart' })
  }
}

const skip = computed(() => view.value?.skip ?? null)
async function voteSkip() {
  if (!client) return
  try {
    const s = await client.skip()
    if (view.value) view.value = { ...view.value, skip: s }
    bridge.haptic('light')
  }
  catch (e) {
    if (!lost(e)) toast.error(codeOf(e) === 'nothing_playing' ? t('nothingPlaying') : t('skipFailed'))
  }
}

// ---------- «Слушать у себя» ----------

const listening = ref(false)
const listenState = ref<'idle' | 'loading' | 'playing' | 'unavailable'>('idle')
const canListen = computed(() => !!view.value?.settings.guestsCanListen)
let audio: HTMLAudioElement | null = null
let loaded: { trackId: string, stream: JamStream } | null = null
let loadingTrack: string | null = null

function stopListening() {
  listening.value = false
  listenState.value = 'idle'
  loaded = null
  if (audio) {
    audio.pause()
    audio.removeAttribute('src')
    audio.load()
  }
}

async function toggleListen() {
  if (listening.value) return stopListening()
  // Звук в браузере телефона — только после нажатия: элемент создаётся и «разблокируется» здесь
  audio ??= new Audio()
  audio.preload = 'auto'
  listening.value = true
  void audio.play().catch(() => {})
  await syncAudio(true)
}

async function syncAudio(force = false) {
  if (!listening.value || !client || !audio) return
  const n = now.value
  const id = n?.current?.track.id ?? null
  if (!n || !id || !n.live) {
    if (!audio.paused) audio.pause()
    return
  }
  if (loaded?.trackId !== id) {
    if (loadingTrack === id && !force) return
    loadingTrack = id
    listenState.value = 'loading'
    try {
      const s = await client.stream(id)
      if (!listening.value) return
      loaded = { trackId: id, stream: s }
      audio.src = s.url
    }
    catch (e) {
      if (lost(e)) return
      listenState.value = 'unavailable'
      loaded = null
      audio.pause()
      if (codeOf(e) === 'forbidden') stopListening()
      return
    }
    finally {
      loadingTrack = null
    }
  }
  const want = positionOf(n, Date.now()) / 1000
  if (!n.playing) {
    if (!audio.paused) audio.pause()
    if (Math.abs(audio.currentTime - want) > 1) audio.currentTime = want
    listenState.value = 'playing'
    return
  }
  // Ушли дальше чем на секунду от хозяина — догоняем
  if (Math.abs(audio.currentTime - want) > 1.2) audio.currentTime = want
  if (audio.paused) {
    try {
      await audio.play()
    }
    catch {
      // Браузер не дал играть без жеста — кнопка «Слушать у себя» нажмётся ещё раз
      listening.value = false
      listenState.value = 'idle'
      return
    }
  }
  listenState.value = 'playing'
}
</script>

<template>
  <div class="jg">
    <div v-if="phase === 'loading'" class="jg__center">
      <span class="jg__spinner" aria-hidden="true" />
    </div>

    <!-- Сессии нет -->
    <main v-else-if="phase === 'ended' || phase === 'removed'" class="jg__center jg__card">
      <BrandGlyph class="jg__glyph" />
      <h1 class="jg__h1">{{ phase === 'removed' ? t('removedTitle') : t('endedTitle') }}</h1>
      <p class="jg__lead">{{ phase === 'removed' ? t('removedText') : t('endedText') }}</p>
    </main>

    <!-- Вход: только имя -->
    <main v-else-if="phase === 'join'" class="jg__center jg__card">
      <span class="jg__host">
        <img v-if="invite?.hostAvatar" :src="invite.hostAvatar" alt="">
        <template v-else>{{ (invite?.hostName ?? '?').charAt(0).toUpperCase() }}</template>
      </span>
      <h1 class="jg__h1">{{ t('invite', { name: invite?.hostName ?? '' }) }}</h1>
      <p class="jg__lead">{{ t('inviteText') }}</p>
      <div v-if="invite?.current" class="jg__np-mini">
        <MusicCoverArt :image="invite.current.image" size="small" radius="8px" class="jg__np-mini-art" />
        <span class="jg__np-mini-text">
          <span class="jg__np-mini-title ellipsis">{{ titleOf(invite.current) }}</span>
          <span class="jg__np-mini-artist ellipsis">{{ artists(invite.current) }}</span>
        </span>
        <span class="jg__eq" aria-hidden="true"><i /><i /><i /></span>
      </div>
      <form class="jg__form" @submit.prevent="join">
        <label class="sr-only" for="jg-name">{{ t('nameLabel') }}</label>
        <input id="jg-name" v-model="name" class="jg__input" type="text" :placeholder="t('namePlaceholder')" maxlength="32" autocomplete="nickname" enterkeyhint="go">
        <UiButton variant="primary" size="lg" block type="submit" :loading="joining" :disabled="!name.trim() && !invite?.memberName">{{ t('join') }}</UiButton>
      </form>
      <p class="jg__fine">{{ t('noAccount') }}</p>
    </main>

    <!-- В сессии -->
    <div v-else class="jg__app">
      <header class="jg__bar">
        <BrandGlyph class="jg__bar-glyph" />
        <span class="jg__bar-text">
          <span class="jg__bar-title ellipsis">{{ view?.hostName ?? invite?.hostName ?? '' }}</span>
          <span class="jg__bar-meta ellipsis">{{ t('title') }} · {{ guestsLabel }}</span>
        </span>
        <UiIconButton icon="ph:sign-out" :label="t('leave')" size="md" @click="leave" />
      </header>

      <section class="jg__now" :aria-label="t('nowPlaying')">
        <div class="jg__art">
          <MusicCoverArt :image="current?.track.image ?? null" size="large" radius="var(--r-4)" eager />
          <TransitionGroup name="heart" tag="div" class="jg__hearts" aria-hidden="true">
            <span v-for="h in hearts" :key="h.id" class="jg__heart" :style="{ left: `${h.x}%` }"><Icon name="ph:heart-fill" /><small>{{ h.name }}</small></span>
          </TransitionGroup>
        </div>
        <template v-if="current">
          <h1 class="jg__title ellipsis">{{ titleOf(current.track) }}</h1>
          <p class="jg__artist ellipsis">{{ artists(current.track) }}</p>
          <p v-if="current.addedBy" class="jg__added">{{ t('addedBy', { name: current.addedBy }) }}</p>
          <div class="jg__progress" role="progressbar" :aria-valuenow="Math.round(progress * 100)" aria-valuemin="0" aria-valuemax="100" :aria-label="t('position')">
            <span class="jg__progress-bar" :style="{ transform: `scaleX(${progress})` }" />
          </div>
          <div class="jg__times"><span>{{ elapsed }}</span><span>{{ !now?.live ? t('hostAway') : !now?.playing ? t('paused') : '' }}</span><span>{{ total }}</span></div>
        </template>
        <template v-else>
          <h1 class="jg__title">{{ t('silence') }}</h1>
          <p class="jg__artist">{{ t('silenceText', { name: view?.hostName ?? '' }) }}</p>
        </template>

        <div class="jg__actions">
          <button type="button" class="jg__act" :disabled="!current" @click="react">
            <Icon name="ph:heart" class="jg__act-icon" /><span>{{ t('like') }}</span>
          </button>
          <button type="button" class="jg__act" :class="{ 'is-on': skip?.voted }" :disabled="!current || skip?.voted" @click="voteSkip">
            <Icon name="ph:skip-forward" class="jg__act-icon" /><span>{{ skip && skip.votes ? t('skipVotes', { n: skip.votes, of: skip.needed }) : t('skip') }}</span>
          </button>
          <button v-if="canListen" type="button" class="jg__act" :class="{ 'is-on': listening }" :aria-pressed="listening" :disabled="!current" @click="toggleListen">
            <Icon :name="listening ? 'ph:headphones-fill' : 'ph:headphones'" class="jg__act-icon" />
            <span>{{ listening ? (listenState === 'unavailable' ? t('notReady') : listenState === 'loading' ? t('connecting') : t('listeningHere')) : t('listenHere') }}</span>
          </button>
        </div>
      </section>

      <section v-if="canAdd" class="jg__sec">
        <h2 class="jg__h2">{{ t('addTitle') }}</h2>
        <UiSearchField v-model="q" :placeholder="t('searchPlaceholder')" size="lg" />
        <div v-if="searching && !results.length" class="jg__hint">{{ t('searching') }}</div>
        <div v-else-if="searched && !results.length" class="jg__hint">{{ t('nothingFound') }}</div>
        <ul v-if="results.length" class="jg__list">
          <li v-for="tr in results" :key="tr.id" class="jrow">
            <MusicCoverArt :image="tr.image" size="small" radius="8px" class="jrow__art" />
            <span class="jrow__text">
              <span class="jrow__name ellipsis">{{ titleOf(tr) }}<span v-if="tr.explicit" class="jrow__e" :title="t('explicit')">E</span></span>
              <span class="jrow__meta ellipsis">{{ artists(tr) }}</span>
            </span>
            <span v-if="stateOf(tr.id) === 'queued'" class="jrow__state"><Icon name="ph:check" />{{ t('inQueue') }}</span>
            <span v-else-if="stateOf(tr.id) === 'pending'" class="jrow__state"><Icon name="ph:hourglass" />{{ t('waiting') }}</span>
            <UiIconButton v-else icon="ph:plus" :label="t('add')" size="md" tone="solid" :disabled="adding === tr.id" @click="add(tr)" />
          </li>
        </ul>
      </section>
      <p v-else class="jg__hint jg__hint--block">{{ t('addingOff') }}</p>

      <section v-if="waiting.length" class="jg__sec">
        <h2 class="jg__h2">{{ t('yourWaiting') }}</h2>
        <ul class="jg__list">
          <li v-for="r in waiting" :key="r.id" class="jrow">
            <MusicCoverArt :image="r.track.image" size="small" radius="8px" class="jrow__art" />
            <span class="jrow__text">
              <span class="jrow__name ellipsis">{{ titleOf(r.track) }}</span>
              <span class="jrow__meta ellipsis">{{ t('waitingHost') }}</span>
            </span>
          </li>
        </ul>
      </section>

      <section class="jg__sec">
        <h2 class="jg__h2">{{ t('next') }}</h2>
        <p v-if="!queue.length" class="jg__hint">{{ t('queueEmpty') }}</p>
        <ol v-else class="jg__list">
          <li v-for="(item, i) in queue" :key="`${item.track.id}-${i}`" class="jrow">
            <MusicCoverArt :image="item.track.image" size="small" radius="8px" class="jrow__art" />
            <span class="jrow__text">
              <span class="jrow__name ellipsis">{{ titleOf(item.track) }}</span>
              <span class="jrow__meta ellipsis">{{ artists(item.track) }}<template v-if="item.addedBy"> · <em class="jrow__by">{{ t('addedBy', { name: item.addedBy }) }}</em></template></span>
            </span>
          </li>
        </ol>
      </section>
    </div>
  </div>
</template>

<style scoped>
.jg { min-height: 100vh; min-height: 100dvh; background: var(--bg, #08090c); color: var(--text-1); }
.jg__center { display: flex; flex-direction: column; align-items: center; justify-content: center; min-height: 100vh; min-height: 100dvh; padding: calc(24px + var(--safe-top)) 20px calc(24px + var(--safe-bottom)); text-align: center; }
.jg__card { gap: 0; max-width: 440px; margin: 0 auto; }
.jg__spinner { width: 28px; height: 28px; border: 2.5px solid var(--fill-3); border-top-color: var(--text-1); border-radius: 50%; animation: jg-spin 0.8s linear infinite; }
@keyframes jg-spin { to { transform: rotate(360deg); } }
.jg__glyph { width: 48px; height: 48px; color: var(--text-2); }
.jg__host { display: grid; place-items: center; width: 72px; height: 72px; overflow: hidden; border-radius: 50%; background: var(--fill-3); font-size: 28px; font-weight: 700; }
.jg__host img { width: 100%; height: 100%; object-fit: cover; }
.jg__h1 { margin: 18px 0 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; text-wrap: balance; }
.jg__lead { max-width: 340px; margin: 10px 0 0; color: var(--text-2); text-wrap: pretty; }
.jg__np-mini { display: flex; align-items: center; gap: 12px; width: 100%; margin-top: 24px; padding: 12px 0; border-top: 0.5px solid var(--hair); border-bottom: 0.5px solid var(--hair); text-align: left; }
.jg__np-mini-art { width: 44px; height: 44px; flex: none; }
.jg__np-mini-text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.jg__np-mini-title { font-weight: 600; }
.jg__np-mini-artist { font-size: var(--fs-small); color: var(--text-3); }
.jg__eq { display: inline-flex; align-items: flex-end; gap: 2px; width: 14px; height: 14px; flex: none; color: var(--accent); }
.jg__eq i { flex: 1; height: 100%; border-radius: 1px; background: currentColor; transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.jg__eq i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.jg__eq i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
.jg__form { display: flex; flex-direction: column; gap: 12px; width: 100%; margin-top: 24px; }
.jg__input { width: 100%; height: 52px; padding: 0 18px; border: 0; border-radius: var(--r-pill); background: var(--fill-2); color: var(--text-1); font: inherit; font-size: 17px; text-align: center; }
.jg__input::placeholder { color: var(--text-3); }
.jg__input:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.jg__fine { margin: 16px 0 0; font-size: var(--fs-caption); color: var(--text-3); }

.jg__app { display: flex; flex-direction: column; gap: 28px; width: 100%; max-width: 560px; margin: 0 auto; padding: calc(8px + var(--safe-top)) 16px calc(32px + var(--safe-bottom)); }
.jg__bar { display: flex; align-items: center; gap: 12px; min-height: 56px; }
.jg__bar-glyph { width: 28px; height: 28px; flex: none; }
.jg__bar-text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.jg__bar-title { font-weight: 650; }
.jg__bar-meta { font-size: var(--fs-caption); color: var(--text-3); }

.jg__now { display: flex; flex-direction: column; align-items: stretch; }
.jg__art { position: relative; width: min(100%, 42vh, 400px); align-self: center; }
.jg__art :deep(.art) { width: 100%; box-shadow: var(--shadow-cover); }
.jg__title { margin: 22px 0 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; }
.jg__artist { margin: 4px 0 0; font-size: var(--fs-body); color: var(--text-2); }
.jg__added { margin: 6px 0 0; font-size: var(--fs-small); color: var(--accent); }
.jg__progress { position: relative; height: 4px; margin-top: 18px; overflow: hidden; border-radius: 2px; background: var(--fill-3); }
.jg__progress-bar { position: absolute; inset: 0; background: var(--text-1); transform-origin: left; transition: transform 0.5s linear; }
.jg__times { display: flex; justify-content: space-between; gap: 8px; margin-top: 6px; font-size: var(--fs-caption); color: var(--text-3); font-variant-numeric: tabular-nums; }
.jg__actions { display: grid; grid-template-columns: repeat(auto-fit, minmax(0, 1fr)); gap: 8px; margin-top: 20px; }
.jg__act { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; min-height: 64px; padding: 8px 4px; border: 0; border-radius: var(--r-3); background: var(--fill-1); color: var(--text-1); font: inherit; font-size: var(--fs-caption); font-weight: 600; -webkit-tap-highlight-color: transparent; }
.jg__act-icon { font-size: 24px; }
.jg__act span { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.jg__act:active { background: var(--fill-2); }
.jg__act:disabled { opacity: 0.45; }
.jg__act.is-on { color: var(--accent); }
.jg__act:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }

.jg__hearts { position: absolute; inset: 0; pointer-events: none; }
.jg__heart { position: absolute; bottom: 12%; display: flex; flex-direction: column; align-items: center; gap: 2px; color: var(--accent); font-size: 30px; animation: jg-float 2.2s var(--ease-out) forwards; }
.jg__heart small { padding: 1px 8px; border-radius: var(--r-pill); background: rgba(0, 0, 0, 0.55); color: #fff; font-size: 12px; font-weight: 600; }
@keyframes jg-float { from { opacity: 0; transform: translateY(20px) scale(0.6); } 15% { opacity: 1; transform: translateY(0) scale(1); } to { opacity: 0; transform: translateY(-140px) scale(1); } }

.jg__sec { display: flex; flex-direction: column; gap: 10px; }
.jg__h2 { margin: 0; font-size: var(--fs-h4); font-weight: 700; letter-spacing: var(--tr-title); }
.jg__hint { padding: 4px 0; font-size: var(--fs-small); color: var(--text-3); }
.jg__hint--block { margin: 0; }
.jg__list { margin: 0; padding: 0; list-style: none; }
.jrow { position: relative; display: flex; align-items: center; gap: 12px; min-height: 60px; padding: 6px 0; }
.jrow + .jrow::before { content: ''; position: absolute; top: 0; left: 56px; right: 0; height: 0.5px; background: var(--hair); }
.jrow__art { width: 44px; height: 44px; flex: none; }
.jrow__text { display: flex; flex: 1; flex-direction: column; gap: 1px; min-width: 0; }
.jrow__name { font-weight: 600; }
.jrow__meta { font-size: var(--fs-small); color: var(--text-3); }
.jrow__by { font-style: normal; color: var(--accent); }
.jrow__e { display: inline-grid; place-items: center; width: 15px; height: 15px; margin-left: 6px; border-radius: 3px; background: var(--fill-3); font-size: 10px; font-weight: 700; color: var(--text-2); vertical-align: 1px; }
.jrow__state { display: inline-flex; align-items: center; gap: 4px; flex: none; font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); }

.heart-leave-active { transition: opacity 0.2s; }
.heart-leave-to { opacity: 0; }

@media (prefers-reduced-motion: reduce) {
  .jg__heart { animation-duration: 0.01s; }
  .jg__progress-bar { transition: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Слушать вместе",
    "titleHost": "{name} — слушать вместе",
    "invite": "{name} зовёт слушать вместе",
    "inviteText": "Добавляйте свои треки в общую очередь — музыка играет у хозяина",
    "nameLabel": "Ваше имя",
    "namePlaceholder": "Как вас зовут?",
    "join": "Присоединиться",
    "noAccount": "Аккаунт не нужен",
    "joinFailed": "Не получилось войти — попробуйте ещё раз",
    "slowDown": "Слишком часто — подождите минуту",
    "endedTitle": "Сессия закончилась",
    "endedText": "Попросите у хозяина новую ссылку",
    "removedTitle": "Вас убрали из сессии",
    "removedText": "Хозяин закрыл вам доступ к этой сессии",
        "leave": "Выйти",
    "nowPlaying": "Сейчас играет",
    "addedBy": "добавил {name}",
    "position": "Позиция в треке",
    "hostAway": "хозяин не на связи",
    "paused": "пауза",
    "silence": "Пока тишина",
    "silenceText": "Музыка начнётся, когда {name} её включит",
    "like": "Нравится",
    "skip": "Пропустить",
    "skipVotes": "{n} из {of}",
    "listenHere": "Слушать у себя",
    "listeningHere": "Играет у вас",
    "connecting": "Подключаем…",
    "notReady": "Трек не готов",
    "addTitle": "Добавить в очередь",
    "searchPlaceholder": "Трек или артист",
    "searching": "Ищем…",
    "nothingFound": "Ничего не нашли",
    "searchFailed": "Поиск не ответил — попробуйте ещё раз",
    "explicit": "Ненормативная лексика",
    "inQueue": "В очереди",
    "waiting": "Ждёт",
    "add": "Добавить в очередь",
    "added": "«{title}» — в очереди",
    "sentForApproval": "Отправили хозяину на одобрение",
    "alreadyQueued": "Этот трек уже в очереди",
    "nothingPlaying": "У хозяина сейчас ничего не играет",
    "tooManyPending": "Подождите, пока хозяин посмотрит прошлые треки",
    "addingOff": "Хозяин выключил добавление треков",
    "explicitOff": "Хозяин слушает без нецензурных треков",
    "addFailed": "Не получилось добавить — попробуйте ещё раз",
    "skipFailed": "Не получилось проголосовать",
    "yourWaiting": "Ждут одобрения",
    "waitingHost": "Хозяин скоро посмотрит",
    "next": "Далее",
    "queueEmpty": "Очередь пуста — добавьте первый трек"
  },
  "en": {
    "title": "Listen together",
    "titleHost": "{name} — listen together",
    "invite": "{name} invites you to listen together",
    "inviteText": "Add your tracks to the shared queue — the music plays at the host’s",
    "nameLabel": "Your name",
    "namePlaceholder": "What’s your name?",
    "join": "Join",
    "noAccount": "No account needed",
    "joinFailed": "Couldn’t join — try again",
    "slowDown": "Too fast — wait a minute",
    "endedTitle": "This session has ended",
    "endedText": "Ask the host for a new link",
    "removedTitle": "You were removed",
    "removedText": "The host closed this session for you",
        "leave": "Leave",
    "nowPlaying": "Now playing",
    "addedBy": "added by {name}",
    "position": "Track position",
    "hostAway": "host is offline",
    "paused": "paused",
    "silence": "Nothing playing yet",
    "silenceText": "Music starts when {name} presses play",
    "like": "Like",
    "skip": "Skip",
    "skipVotes": "{n} of {of}",
    "listenHere": "Listen here",
    "listeningHere": "Playing here",
    "connecting": "Connecting…",
    "notReady": "Not ready",
    "addTitle": "Add to queue",
    "searchPlaceholder": "Track or artist",
    "searching": "Searching…",
    "nothingFound": "Nothing found",
    "searchFailed": "Search didn’t respond — try again",
    "explicit": "Explicit",
    "inQueue": "Queued",
    "waiting": "Pending",
    "add": "Add to queue",
    "added": "“{title}” is queued",
    "sentForApproval": "Sent to the host for approval",
    "alreadyQueued": "This track is already queued",
    "nothingPlaying": "Nothing is playing at the host’s",
    "tooManyPending": "Wait until the host reviews your tracks",
    "addingOff": "The host turned off adding tracks",
    "explicitOff": "The host is listening without explicit tracks",
    "addFailed": "Couldn’t add — try again",
    "skipFailed": "Couldn’t vote",
    "yourWaiting": "Waiting for approval",
    "waitingHost": "The host will look soon",
    "next": "Up next",
    "queueEmpty": "The queue is empty — add the first track"
  }
}
</i18n>
