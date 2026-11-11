<script setup lang="ts">
import type { AdminAcquisition, ResolveRequest } from '~/api/types'

// Спорные: слева очередь записей, справа ожидаемый трек с превью Deezer и кандидаты с плеером
// и разбором оценки. Выбрать кандидата, вставить ссылку, «нет в источниках», искать заново.
// Клавиши: ↑ ↓ — запись, 1–9 — выбрать кандидата, P — превью, L — ссылка, U — нет в источниках, R — заново
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const ui = useUiStore()
const player = usePlayerStore()
const { refresh: refreshCounts } = useAdminCounts()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const items = ref<AdminAcquisition[] | null>(null)
const cursor = ref<string | null>(null)
const failed = ref(false)
const currentId = ref<string | null>(null)
const names = ref<Record<string, string>>({})

async function load() {
  failed.value = false
  try {
    const p = await bridge.admin.acquisitions({ status: 'disputed', limit: 100 })
    items.value = p.items
    cursor.value = p.nextCursor
    if (!items.value.some(i => i.acquisition.id === currentId.value)) currentId.value = items.value[0]?.acquisition.id ?? null
  }
  catch {
    failed.value = true
  }
}
onMounted(() => {
  void load()
  void bridge.admin.sources().then((s) => {
    names.value = Object.fromEntries(s.downloaders.map(d => [d.id, d.name]))
  }).catch(() => {})
})
const reloadSoon = useDebounced(() => void load(), 1500)
useAdminLive({ 'acquisition.updated': e => (e.status === 'disputed' ? reloadSoon() : undefined) }, load)

const index = computed(() => items.value?.findIndex(i => i.acquisition.id === currentId.value) ?? -1)
const current = computed(() => (index.value >= 0 ? items.value![index.value]! : null))
const track = computed(() => current.value?.track ?? null)
function go(delta: number) {
  const list = items.value
  if (!list?.length) return
  const i = Math.min(list.length - 1, Math.max(0, index.value + delta))
  currentId.value = list[i]!.acquisition.id
}

// ---------- Превью Deezer ----------

const audio = ref<HTMLAudioElement | null>(null)
const preview = ref<{ id: string, url: string | null } | null>(null)
const previewLoading = ref(false)
const previewPlaying = ref(false)
const previewPos = ref(0)
async function togglePreview() {
  const tr = track.value
  if (!tr || !audio.value) return
  if (previewPlaying.value) {
    audio.value.pause()
    return
  }
  if (preview.value?.id !== tr.id) {
    previewLoading.value = true
    try {
      const p = await bridge.admin.preview(tr.id)
      preview.value = { id: tr.id, url: p.url }
      if (p.url) audio.value.src = p.url
    }
    catch {
      toast.error(t('previewFailed'))
      return
    }
    finally {
      previewLoading.value = false
    }
  }
  if (!preview.value?.url) {
    toast.show({ text: t('noPreview'), icon: 'ph:speaker-slash' })
    return
  }
  // Музыка приложения замолкает, чтобы сравнивать на слух
  if (player.playing) player.pause()
  detail.value?.play(null)
  await audio.value.play().catch(() => toast.error(t('previewFailed')))
}
watch(currentId, () => {
  audio.value?.pause()
  previewPos.value = 0
})

// ---------- Решения ----------

const busy = ref<string | null>(null)
const link = ref('')
const linkInput = ref<HTMLInputElement | null>(null)
const detail = ref<{ play: (v: string | null) => void } | null>(null)

async function resolve(req: ResolveRequest, key: string, done: string) {
  const c = current.value
  if (!c || busy.value) return
  busy.value = key
  try {
    await bridge.admin.resolve(c.acquisition.id, req)
    toast.show({ text: done, icon: req.action === 'unavailable' ? 'ph:prohibit' : 'ph:download-simple' })
    const i = index.value
    items.value = items.value?.filter(x => x.acquisition.id !== c.acquisition.id) ?? null
    const next = items.value?.[Math.min(i, (items.value?.length ?? 1) - 1)]
    currentId.value = next?.acquisition.id ?? null
    link.value = ''
    void refreshCounts()
  }
  catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : t('failed'))
  }
  finally {
    busy.value = null
  }
}
const title = computed(() => track.value?.title ?? '')
const choose = (videoId: string) => resolve({ action: 'choose', videoId }, videoId, t('chosen', { title: title.value }))
const submitLink = () => link.value.trim() && resolve({ action: 'url', url: link.value.trim() }, 'url', t('linked', { title: title.value }))
const unavailable = () => resolve({ action: 'unavailable' }, 'unavailable', t('markedUnavailable', { title: title.value }))
const retry = () => resolve({ action: 'retry' }, 'retry', t('retried', { title: title.value }))

function onKey(e: KeyboardEvent) {
  const el = e.target as HTMLElement | null
  if (el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName))) {
    if (e.key === 'Escape') el.blur()
    return
  }
  if (e.metaKey || e.ctrlKey || e.altKey || !current.value) return
  const k = e.key.toLowerCase()
  if (k === 'arrowdown' || k === 'j') go(1)
  else if (k === 'arrowup' || k === 'k') go(-1)
  else if (/^[1-9]$/.test(k)) {
    const c = current.value.candidates[Number(k) - 1]
    if (c) void choose(c.videoId)
  }
  else if (k === 'p') void togglePreview()
  else if (k === 'l') linkInput.value?.focus()
  else if (k === 'u') void unavailable()
  else if (k === 'r') void retry()
  else return
  e.preventDefault()
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  audio.value?.pause()
})

const best = (a: AdminAcquisition) => a.candidates.reduce((m, c) => Math.max(m, c.score ?? 0), 0)
</script>

<template>
  <div class="ap dp">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <div v-if="!ui.isPhone && items?.length" class="dp__keys">
        <span><UiKbd :keys="['↑', '↓']" />{{ t('keys.move') }}</span>
        <span><UiKbd :keys="['1–9']" />{{ t('keys.choose') }}</span>
        <span><UiKbd :keys="['P']" />{{ t('keys.preview') }}</span>
        <span><UiKbd :keys="['L']" />{{ t('keys.link') }}</span>
        <span><UiKbd :keys="['U']" />{{ t('keys.none') }}</span>
      </div>
    </header>

    <AdminState :loading="!items && !failed" :error="failed" :empty="!!items && !items.length" empty-icon="ph:check-circle" :empty-title="t('empty')" :empty-text="t('emptyText')" @retry="load">
      <div class="dp__grid">
        <!-- Очередь спорных -->
        <nav v-if="!ui.isPhone" class="dp__list thin-scrollbar" :aria-label="t('queue')">
          <button
            v-for="a in items"
            :key="a.acquisition.id"
            type="button"
            class="dp__item"
            :class="{ 'is-active': a.acquisition.id === currentId }"
            :aria-current="a.acquisition.id === currentId ? 'true' : undefined"
            @click="currentId = a.acquisition.id"
          >
            <span class="ap-cell">
              <b>{{ a.track?.title ?? t('noTrack') }}</b>
              <small>{{ a.track ? artistNames(a.track.artists) : '' }}</small>
            </span>
            <span class="dp__best tnum" :title="t('bestScore')">{{ Math.round(best(a) * 100) }}</span>
          </button>
          <p v-if="cursor" class="ap-hint dp__more">{{ t('more') }}</p>
        </nav>
        <div v-else class="dp__pager">
          <UiIconButton icon="ph:caret-left" :label="t('prev')" :disabled="index <= 0" @click="go(-1)" />
          <span class="tnum">{{ t('of', { i: index + 1, n: items?.length ?? 0 }) }}</span>
          <UiIconButton icon="ph:caret-right" :label="t('next')" :disabled="index >= (items?.length ?? 0) - 1" @click="go(1)" />
        </div>

        <!-- Ожидаемый трек и решения -->
        <article v-if="current" class="dp__detail">
          <section class="dp__expected" :aria-label="t('expected')">
            <span class="dp__label">{{ t('expected') }}</span>
            <div class="dp__track">
              <button type="button" class="dp__cover" :aria-label="previewPlaying ? t('stopPreview') : t('playPreview')" @click="togglePreview">
                <MusicCoverArt :image="track?.album?.image" size="medium" :alt="track?.album?.title ?? ''" />
                <span class="dp__play" :class="{ 'is-on': previewPlaying }">
                  <Icon :name="previewLoading ? 'ph:circle-notch' : previewPlaying ? 'ph:pause-fill' : 'ph:play-fill'" :class="{ spin: previewLoading }" />
                </span>
              </button>
              <div class="dp__info">
                <h2 class="dp__title">{{ track?.title ?? t('noTrack') }}<span v-if="track?.version" class="ap-muted"> {{ track.version }}</span></h2>
                <p class="dp__artists">{{ track ? artistNames(track.artists) : '' }}</p>
                <dl class="dp__facts">
                  <div v-if="track?.album"><dt>{{ t('album') }}</dt><dd>{{ track.album.title }}</dd></div>
                  <div v-if="track"><dt>{{ t('duration') }}</dt><dd class="tnum">{{ formatDuration(track.durationMs) }}</dd></div>
                  <div v-if="track?.isrc"><dt>ISRC</dt><dd class="ap-mono">{{ track.isrc }}</dd></div>
                  <div><dt>{{ t('tried') }}</dt><dd>{{ formatRelative(current.acquisition.updatedAt, lang) }}</dd></div>
                </dl>
                <div class="dp__progress" :class="{ 'is-on': previewPlaying || previewPos > 0 }" aria-hidden="true"><i :style="{ width: `${previewPos * 100}%` }" /></div>
              </div>
            </div>
            <div class="dp__actions">
              <form class="dp__link" @submit.prevent="submitLink">
                <input ref="linkInput" v-model="link" class="ap-input" type="url" inputmode="url" :placeholder="t('linkPlaceholder')" :aria-label="t('linkLabel')">
                <UiButton type="submit" :disabled="!link.trim()" :loading="busy === 'url'">{{ t('download') }}</UiButton>
              </form>
              <div class="dp__buttons">
                <UiButton variant="ghost" icon="ph:magnifying-glass" :loading="busy === 'retry'" @click="retry">{{ t('searchAgain') }}</UiButton>
                <UiButton variant="ghost" icon="ph:prohibit" :loading="busy === 'unavailable'" @click="unavailable">{{ t('none') }}</UiButton>
              </div>
            </div>
          </section>
          <AdminAcquisition ref="detail" :acq="current" selectable hotkeys :busy="busy" :names="names" @choose="choose" />
        </article>
      </div>
    </AdminState>
    <audio
      ref="audio"
      preload="none"
      @play="previewPlaying = true"
      @pause="previewPlaying = false"
      @ended="previewPlaying = false; previewPos = 0"
      @timeupdate="previewPos = audio && audio.duration ? audio.currentTime / audio.duration : 0"
    />
  </div>
</template>

<style scoped>
.dp { max-width: 1320px; }
.dp__keys { display: flex; flex-wrap: wrap; gap: 6px 16px; font-size: var(--fs-caption); color: var(--text-3); }
.dp__keys span { display: inline-flex; align-items: center; gap: 6px; }
.dp__grid { display: grid; grid-template-columns: 280px minmax(0, 1fr); gap: 32px; align-items: start; }
.dp__list { position: sticky; top: 24px; display: flex; flex-direction: column; max-height: calc(100dvh - 160px); overflow-y: auto; padding-right: 4px; }
.dp__item { display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 8px 12px; border: 0; border-radius: var(--r-3); background: none; color: var(--text-2); font: inherit; font-size: var(--fs-small); text-align: left; cursor: pointer; transition: background-color var(--dur-1), color var(--dur-1); }
.dp__item + .dp__item { box-shadow: inset 0 1px 0 var(--hair); }
.dp__item:hover { background: var(--fill-1); color: var(--text-1); }
.dp__item:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.dp__item.is-active { background: var(--fill-2); color: var(--text-1); box-shadow: none; }
.dp__item.is-active + .dp__item { box-shadow: none; }
.dp__item .ap-cell { flex: 1; }
.dp__best { font-size: var(--fs-caption); font-weight: 700; color: var(--warning); }
.dp__more { padding: 8px 12px; }
.dp__pager { display: flex; align-items: center; justify-content: space-between; gap: 12px; font-size: var(--fs-small); font-weight: 600; color: var(--text-2); }
.dp__detail { display: flex; flex-direction: column; gap: 28px; min-width: 0; }
.dp__expected { display: flex; flex-direction: column; gap: 16px; padding-bottom: 24px; border-bottom: 1px solid var(--hair); }
.dp__label { font-size: 11px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.dp__track { display: grid; grid-template-columns: 132px minmax(0, 1fr); gap: 20px; align-items: start; }
.dp__cover { position: relative; display: block; width: 132px; padding: 0; border: 0; border-radius: var(--r-3); background: none; cursor: pointer; }
.dp__cover :deep(.cover), .dp__cover :deep(img) { width: 132px; height: 132px; border-radius: var(--r-3); }
.dp__cover:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.dp__play { position: absolute; right: 8px; bottom: 8px; display: grid; place-items: center; width: 40px; height: 40px; border-radius: 50%; background: var(--text-1); color: var(--ink-1); font-size: 18px; box-shadow: var(--shadow-1); transition: transform var(--dur-1) var(--ease-out); }
.dp__cover:hover .dp__play { transform: scale(1.06); }
.dp__cover:active .dp__play { transform: scale(0.96); }
.dp__play.is-on { background: var(--accent); }
.spin { animation: dp-spin 0.9s linear infinite; }
@keyframes dp-spin { to { transform: rotate(360deg); } }
.dp__info { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.dp__title { margin: 0; overflow: hidden; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); text-overflow: ellipsis; white-space: nowrap; }
.dp__artists { margin: 0; overflow: hidden; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.dp__facts { display: flex; flex-wrap: wrap; gap: 4px 20px; margin: 8px 0 0; font-size: var(--fs-caption); }
.dp__facts div { display: flex; gap: 6px; min-width: 0; }
.dp__facts dt { color: var(--text-3); }
.dp__facts dd { margin: 0; overflow: hidden; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; max-width: 260px; }
.dp__progress { opacity: 0; transition: opacity var(--dur-2); }
.dp__progress.is-on { opacity: 1; }
.dp__progress { height: 3px; margin-top: 12px; overflow: hidden; border-radius: 2px; background: var(--fill-2); }
.dp__progress i { display: block; height: 100%; background: var(--accent); transition: width 0.25s linear; }
.dp__actions { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; }
.dp__link { display: flex; flex: 1 1 360px; gap: 8px; min-width: 0; }
.dp__link .ap-input { flex: 1; }
.dp__buttons { display: flex; flex-wrap: wrap; gap: 4px; }
@media (max-width: 1099px) {
  .dp__grid { grid-template-columns: 220px minmax(0, 1fr); gap: 24px; }
}
@media (max-width: 699px) {
  .dp__grid { grid-template-columns: minmax(0, 1fr); gap: 16px; }
  .dp__track { grid-template-columns: 96px minmax(0, 1fr); gap: 14px; }
  .dp__cover, .dp__cover :deep(.cover), .dp__cover :deep(img) { width: 96px; height: 96px; }
  .dp__play { right: 6px; bottom: 6px; width: 36px; height: 36px; }
  .dp__title { font-size: var(--fs-h4); white-space: normal; }
  .dp__link { flex-basis: 100%; }
  .dp__link .ap-input { height: 44px; }
  .dp__buttons { width: 100%; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Спорные",
    "queue": "Спорные записи",
    "empty": "Спорных записей нет",
    "emptyText": "Сюда попадает скачанное, что не совпало с превью Deezer",
    "noTrack": "Трек удалён из каталога",
    "bestScore": "Лучшая оценка кандидата",
    "more": "Показаны первые 100",
    "prev": "Предыдущая",
    "next": "Следующая",
    "of": "{i} из {n}",
    "expected": "Ожидаемый трек",
    "album": "Альбом",
    "duration": "Длительность",
    "tried": "Последняя попытка",
    "playPreview": "Слушать превью Deezer",
    "stopPreview": "Остановить превью",
    "noPreview": "У этого трека нет превью Deezer",
    "previewFailed": "Превью не загрузилось",
    "linkLabel": "Ссылка на запись",
    "linkPlaceholder": "Ссылка: YouTube, SoundCloud, Bandcamp…",
    "download": "Скачать",
    "searchAgain": "Искать заново",
    "none": "Нет в источниках",
    "chosen": "«{title}» скачается из выбранного видео",
    "linked": "«{title}» скачается по ссылке",
    "markedUnavailable": "«{title}» помечен: нет в источниках",
    "retried": "«{title}» поищется заново",
    "failed": "Не получилось — попробуйте ещё раз",
    "keys": { "move": "запись", "choose": "выбрать", "preview": "превью", "link": "ссылка", "none": "нет в источниках" }
  },
  "en": {
    "title": "Disputed",
    "queue": "Disputed recordings",
    "empty": "No disputed recordings",
    "emptyText": "Downloads that didn't match the Deezer preview land here",
    "noTrack": "Track removed from the catalog",
    "bestScore": "Best candidate score",
    "more": "Showing the first 100",
    "prev": "Previous",
    "next": "Next",
    "of": "{i} of {n}",
    "expected": "Expected track",
    "album": "Album",
    "duration": "Duration",
    "tried": "Last attempt",
    "playPreview": "Play the Deezer preview",
    "stopPreview": "Stop the preview",
    "noPreview": "This track has no Deezer preview",
    "previewFailed": "The preview didn't load",
    "linkLabel": "Link to the recording",
    "linkPlaceholder": "Link: YouTube, SoundCloud, Bandcamp…",
    "download": "Download",
    "searchAgain": "Search again",
    "none": "Not in sources",
    "chosen": "“{title}” will download from the chosen video",
    "linked": "“{title}” will download from the link",
    "markedUnavailable": "“{title}” marked as not in sources",
    "retried": "“{title}” will be searched again",
    "failed": "Didn't work — try again",
    "keys": { "move": "recording", "choose": "choose", "preview": "preview", "link": "link", "none": "not in sources" }
  }
}
</i18n>
