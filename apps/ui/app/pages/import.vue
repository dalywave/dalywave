<script setup lang="ts">
import type { ImportItem, ImportKind, ImportSummary } from '~/api/types'

// Перенос музыки: архив данных Spotify, список треков или свои файлы. Один спокойный путь:
// выбрать способ → отдать файл или текст → видеть ход → итог одной фразой и «не нашлось» с поиском.
// Сопоставление идёт на сервере в фоне, страницу можно закрыть; прошлые переносы — внизу
type Mode = 'spotify' | 'list' | 'files'

const bridge = useBridge()
const toast = useToast()
const ui = useUiStore()
const lang = useLang()
const route = useRoute()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })
const { y: scrollY } = useWindowScroll()

const MODES: { value: Mode, icon: string }[] = [
  { value: 'spotify', icon: 'ph:spotify-logo' },
  { value: 'list', icon: 'ph:list-bullets' },
  { value: 'files', icon: 'ph:waveform' },
]
const initial = route.query.mode
const mode = ref<Mode>(initial === 'list' || initial === 'files' ? initial : 'spotify')
watch(mode, m => void navigateTo({ query: { ...route.query, mode: m } }, { replace: true }))

function modeOf(kind: ImportKind): Mode {
  return kind === 'spotify_archive' ? 'spotify' : kind === 'files' ? 'files' : 'list'
}

// ---------- Текущий перенос ----------

/** Показываем ход и итог (а не выбор способа) */
const active = ref(false)
const current = ref<ImportSummary | null>(null)
const uploadShare = ref<number | null>(null)
const fileName = ref('')
let abort: AbortController | null = null
const reviewKey = ref(0)

const running = computed(() => current.value?.status === 'running')
const done = computed(() => !!current.value && current.value.status !== 'running')

async function refresh() {
  if (!current.value) return
  try {
    const next = await bridge.imports.get(current.value.id)
    const finished = current.value.status === 'running' && next.status !== 'running'
    current.value = next
    if (finished) {
      void loadHistory()
      reviewKey.value++
    }
  }
  catch {
    // Сеть моргнула — спросим в следующий раз
  }
}
useIntervalFn(() => {
  if (active.value && running.value) void refresh()
}, 1500)

function reset() {
  abort?.abort()
  abort = null
  active.value = false
  current.value = null
  uploadShare.value = null
  fileName.value = ''
  files.value = []
  filesImport.value = null
}

function open(s: ImportSummary) {
  reset()
  mode.value = modeOf(s.kind)
  current.value = s
  active.value = true
  if (s.kind === 'files') void loadFileItems()
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

async function stop() {
  if (!current.value) return
  try {
    await bridge.imports.remove(current.value.id)
    await refresh()
    toast.show({ text: t('stopped'), icon: 'ph:pause-circle-fill' })
  }
  catch (e) {
    toast.error((e as Error).message)
  }
}

async function forget() {
  const s = current.value
  if (!s) return
  try {
    await bridge.imports.remove(s.id)
    history.value = history.value.filter(h => h.id !== s.id)
    reset()
    toast.show({ text: t('forgotten'), icon: 'ph:trash' })
  }
  catch (e) {
    toast.error((e as Error).message)
  }
}

// ---------- Архив Spotify ----------

async function startSpotify(list: File[]) {
  const file = list[0]
  if (!file) return
  if (!/\.(zip|json)$/i.test(file.name)) {
    toast.error(t('spotify.wrongFile'))
    return
  }
  if (file.size > 500 * 1024 * 1024) {
    toast.error(t('spotify.tooBig'))
    return
  }
  reset()
  active.value = true
  fileName.value = file.name
  uploadShare.value = 0
  abort = new AbortController()
  try {
    current.value = await bridge.imports.spotify(file, (s) => { uploadShare.value = s }, abort.signal)
    uploadShare.value = null
    void loadHistory()
  }
  catch (e) {
    if ((e as Error).name !== 'AbortError') toast.error((e as Error).message || t('uploadFailed'))
    reset()
  }
}

// ---------- Список ----------

const listText = ref('')
const listTarget = ref<'playlist' | 'likes'>('playlist')
const listTitle = ref('')
const listBusy = ref(false)
const listLines = computed(() => listText.value.split('\n').filter(l => l.trim() && !l.trim().startsWith('#')).length)
const listOver = ref(false)

async function readListFile(e: DragEvent | Event) {
  const files = 'dataTransfer' in e && e.dataTransfer ? e.dataTransfer.files : (e.target as HTMLInputElement).files
  listOver.value = false
  const file = files?.[0]
  if (!file) return
  if (file.size > 4 * 1024 * 1024) {
    toast.error(t('list.tooBig'))
    return
  }
  listText.value = await file.text()
  if (!listTitle.value) listTitle.value = file.name.replace(/\.(txt|csv|m3u8?|tsv)$/i, '')
  if (e.target instanceof HTMLInputElement) e.target.value = ''
}

async function startList() {
  if (!listLines.value) return
  listBusy.value = true
  try {
    const s = await bridge.imports.fromList({
      text: listText.value,
      target: listTarget.value,
      playlistTitle: listTarget.value === 'playlist' ? listTitle.value.trim() || null : null,
    })
    reset()
    current.value = s
    active.value = true
    listText.value = ''
    listTitle.value = ''
    void loadHistory()
  }
  catch (e) {
    toast.error((e as Error).message || t('uploadFailed'))
  }
  finally {
    listBusy.value = false
  }
}

// ---------- Свои файлы ----------

interface FileRow {
  key: string
  file?: File
  name: string
  size: number
  state: 'waiting' | 'uploading' | 'sent' | 'error'
  share: number
  error?: string
  item?: ImportItem
}
const files = ref<FileRow[]>([])
const filesImport = ref<string | null>(null)
const AUDIO = /\.(flac|m4a|mp4|alac|mp3|opus|ogg|oga|webm|wav|aiff?)$/i
let uploading = false

async function addFiles(list: File[]) {
  const ok = list.filter(f => AUDIO.test(f.name))
  if (ok.length < list.length) toast.error(t('files.skippedSome', { n: list.length - ok.length }))
  if (!ok.length) return
  if (!filesImport.value || done.value) {
    reset()
    try {
      const s = await bridge.imports.startFiles()
      filesImport.value = s.id
      current.value = s
      active.value = true
    }
    catch (e) {
      toast.error((e as Error).message || t('uploadFailed'))
      return
    }
  }
  files.value = [...files.value, ...ok.map((f, i) => ({ key: `${Date.now()}-${i}-${f.name}`, file: f, name: f.name, size: f.size, state: 'waiting' as const, share: 0 }))]
  void pump()
}

/** Загрузка по одному: прогресс честный, сеть не забивается */
async function pump() {
  if (uploading) return
  uploading = true
  abort = abort ?? new AbortController()
  try {
    for (;;) {
      const row = files.value.find(r => r.state === 'waiting')
      const id = filesImport.value
      if (!row || !row.file || !id) break
      row.state = 'uploading'
      try {
        row.item = await bridge.imports.file(id, row.file, (s) => { row.share = s }, abort.signal)
        row.state = 'sent'
        row.share = 1
        row.file = undefined
      }
      catch (e) {
        if ((e as Error).name === 'AbortError') return
        row.state = 'error'
        row.error = (e as Error).message
      }
    }
    const id = filesImport.value
    if (id && files.value.length && !files.value.some(r => r.state === 'waiting' || r.state === 'uploading')) {
      current.value = await bridge.imports.finish(id)
      void loadHistory()
    }
  }
  catch (e) {
    toast.error((e as Error).message)
  }
  finally {
    uploading = false
  }
}

async function loadFileItems() {
  const id = current.value?.id
  if (!id) return
  try {
    const page = await bridge.imports.items(id, null, null, 200)
    const byId = new Map(page.items.map(i => [i.id, i]))
    if (!files.value.length) {
      files.value = page.items.map(i => ({ key: i.id, name: i.file?.name ?? i.title, size: i.file?.sizeBytes ?? 0, state: 'sent' as const, share: 1, item: i }))
    }
    else {
      for (const row of files.value) {
        const fresh = row.item && byId.get(row.item.id)
        if (fresh) row.item = fresh
      }
    }
  }
  catch {
    // Повторим со следующим опросом
  }
}
useIntervalFn(() => {
  if (active.value && current.value?.kind === 'files' && files.value.some(r => r.state === 'sent' && r.item?.status === 'pending')) void loadFileItems()
}, 2000)
watch(done, (d) => {
  if (d && current.value?.kind === 'files') void loadFileItems()
})

function fileState(row: FileRow): { text: string, tone: 'muted' | 'ok' | 'warn' | 'bad', icon: string } {
  if (row.state === 'waiting') return { text: t('files.waiting'), tone: 'muted', icon: 'ph:clock' }
  if (row.state === 'uploading') return { text: t('files.uploading', { pct: Math.round(row.share * 100) }), tone: 'muted', icon: 'ph:upload-simple' }
  if (row.state === 'error') return { text: row.error || t('uploadFailed'), tone: 'bad', icon: 'ph:warning-circle' }
  const item = row.item
  if (!item || item.status === 'pending') return { text: t('files.processing'), tone: 'muted', icon: 'ph:spinner-gap' }
  if (item.status === 'skipped') return { text: item.file?.error || t('files.skipped'), tone: 'warn', icon: 'ph:minus-circle' }
  if (item.status === 'unmatched') return { text: t('files.unmatched'), tone: 'warn', icon: 'ph:question' }
  const tr = item.track
  const where = tr ? `${artistNames(tr.artists)} — ${tr.title}` : ''
  const how = item.file?.upgraded ? t('files.upgraded') : t('files.kept')
  return { text: [where, how].filter(Boolean).join(' · '), tone: 'ok', icon: 'ph:check-circle' }
}
function fileQuality(row: FileRow): string {
  const f = row.item?.file
  if (!f?.codec) return ''
  const parts = [qualityLabel({ codec: f.codec, bitrateKbps: f.lossless ? undefined : f.bitrateKbps ?? undefined })]
  if (f.lossless && f.bitDepth && f.sampleRate) parts.push(`${f.bitDepth}/${Math.round(f.sampleRate / 100) / 10}`)
  return parts.filter(Boolean).join(' ')
}

// ---------- История ----------

const history = ref<ImportSummary[]>([])
async function loadHistory() {
  try {
    history.value = (await bridge.imports.list()).items
  }
  catch {
    // История — второстепенное: без неё страница работает
  }
}
onMounted(async () => {
  await loadHistory()
  const q = route.query.id
  const found = typeof q === 'string' ? history.value.find(h => h.id === q) : history.value.find(h => h.status === 'running')
  if (found) open(found)
})
onBeforeUnmount(() => abort?.abort())

const KIND_ICON: Record<ImportKind, string> = { spotify_archive: 'ph:spotify-logo', text: 'ph:list-bullets', csv: 'ph:table', m3u: 'ph:playlist', files: 'ph:waveform' }
function historySub(s: ImportSummary): string {
  const when = formatRelative(s.createdAt, lang.value)
  if (s.status === 'running') return `${when} · ${t('history.running')}`
  if (s.status === 'failed') return `${when} · ${t('history.failed')}`
  if (s.status === 'cancelled') return `${when} · ${t('history.cancelled')}`
  return `${when} · ${t('history.found', { found: formatNumber(s.matched, lang.value) })}`
}

const actionLink = computed(() => {
  const s = current.value
  if (!s || s.status !== 'done') return null
  if (s.playlistId) return { to: `/playlist/${s.playlistId}`, label: t('openPlaylist') }
  if (s.kind === 'spotify_archive' || s.stats.likes) return { to: '/library', label: t('openLibrary') }
  return null
})
const showHistory = computed(() => history.value.filter(h => h.id !== current.value?.id))
</script>

<template>
  <div class="imp">
    <MusicPhoneNav :title="t('title')" :condensed="scrollY > 44" fallback="/settings" />
    <h1 class="imp__title">{{ t('title') }}</h1>

    <template v-if="!active">
      <!-- Откуда переносить: один ряд, как сегменты, но с зоной касания 44 -->
      <div class="imp__modes" role="radiogroup" :aria-label="t('how')">
        <button
          v-for="m in MODES"
          :key="m.value"
          type="button"
          role="radio"
          class="imp__mode"
          :class="{ 'is-on': mode === m.value }"
          :aria-checked="mode === m.value"
          @click="mode = m.value"
        >
          <Icon :name="m.icon" class="imp__mode-icon" aria-hidden="true" />
          <span class="ellipsis">{{ t(`mode.${m.value}`) }}</span>
        </button>
      </div>

      <section v-if="mode === 'spotify'" class="imp__panel">
        <ImportDropZone
          :title="ui.isPhone ? t('spotify.dropPhone') : t('spotify.drop')"
          :hint="t('spotify.dropHint')"
          :button="t('spotify.pick')"
          icon="ph:file-zip"
          accept=".zip,.json,application/zip"
          @files="startSpotify"
        />
        <p class="imp__note">{{ t('spotify.where') }}</p>
      </section>

      <section v-else-if="mode === 'list'" class="imp__panel">
        <div
          class="imp__area"
          :class="{ 'is-over': listOver }"
          @dragenter.prevent="listOver = true"
          @dragover.prevent
          @dragleave="listOver = false"
          @drop.prevent="readListFile"
        >
          <textarea
            v-model="listText"
            class="imp__text"
            :placeholder="t('list.placeholder')"
            :aria-label="t('list.label')"
            spellcheck="false"
            rows="9"
          />
          <div class="imp__area-foot">
            <span class="tnum">{{ listLines ? t('list.lines', { n: formatNumber(listLines, lang) }) : t('list.formats') }}</span>
            <label class="imp__file-btn">
              <Icon name="ph:paperclip" aria-hidden="true" />{{ t('list.file') }}
              <input type="file" accept=".txt,.csv,.tsv,.m3u,.m3u8,text/plain,text/csv,audio/x-mpegurl" class="sr-only" @change="readListFile">
            </label>
          </div>
        </div>
        <div class="imp__target">
          <UiSegmentedControl
            v-model="listTarget"
            :options="[{ value: 'playlist', label: t('list.toPlaylist'), icon: 'ph:playlist' }, { value: 'likes', label: t('list.toLikes'), icon: 'ph:heart' }]"
            :label="t('list.where')"
            :size="ui.isPhone ? 'sm' : 'md'"
            :block="ui.isPhone"
          />
          <input
            v-if="listTarget === 'playlist'"
            v-model="listTitle"
            class="imp__input"
            :placeholder="t('list.titlePlaceholder')"
            :aria-label="t('list.titleLabel')"
            maxlength="200"
          >
        </div>
        <UiButton variant="primary" size="lg" icon="ph:magnifying-glass" :disabled="!listLines" :loading="listBusy" :block="ui.isPhone" class="imp__go" @click="startList">{{ t('list.go') }}</UiButton>
      </section>

      <section v-else class="imp__panel">
        <ImportDropZone
          :title="ui.isPhone ? t('files.dropPhone') : t('files.drop')"
          :hint="t('files.dropHint')"
          :button="t('files.pick')"
          icon="ph:file-audio"
          accept=".flac,.m4a,.mp4,.alac,.mp3,.opus,.ogg,.oga,.webm,.wav,.aif,.aiff,audio/*"
          multiple
          @files="addFiles"
        />
      </section>
    </template>

    <template v-else>
      <ImportStatus :summary="current" :upload="uploadShare" :file-name="fileName">
        <template #actions>
          <UiButton v-if="actionLink" variant="primary" :to="actionLink.to" icon-right="ph:arrow-right">{{ actionLink.label }}</UiButton>
          <UiButton v-if="done" icon="ph:plus" @click="reset">{{ t('again') }}</UiButton>
          <UiButton v-if="running && current?.kind !== 'files'" variant="ghost" icon="ph:stop-circle" @click="stop">{{ t('stop') }}</UiButton>
          <UiButton v-if="!current && uploadShare !== null" variant="ghost" icon="ph:x" @click="reset">{{ t('cancelUpload') }}</UiButton>
          <UiButton v-if="done" variant="ghost" icon="ph:trash" class="imp__forget" @click="forget">{{ t('forget') }}</UiButton>
        </template>
      </ImportStatus>

      <section v-if="current?.kind === 'files' && files.length" class="imp__files" :aria-label="t('files.list')">
        <div v-for="row in files" :key="row.key" class="imp__frow">
          <span class="imp__frow-icon" :class="`is-${fileState(row).tone}`" aria-hidden="true"><Icon :name="fileState(row).icon" /></span>
          <div class="imp__frow-main">
            <span class="imp__frow-name ellipsis">{{ row.name }}</span>
            <span class="imp__frow-state ellipsis" :class="`is-${fileState(row).tone}`">{{ fileState(row).text }}</span>
          </div>
          <span class="imp__frow-meta tnum">{{ fileQuality(row) || formatBytes(row.size, lang) }}</span>
          <div v-if="row.state === 'uploading'" class="imp__frow-bar"><i :style="{ width: `${row.share * 100}%` }" /></div>
        </div>
        <ImportDropZone
          v-if="current?.phase === 'uploading'"
          class="imp__more-files"
          :title="t('files.more')"
          :button="t('files.pick')"
          icon="ph:plus"
          accept=".flac,.m4a,.mp4,.alac,.mp3,.opus,.ogg,.oga,.webm,.wav,.aif,.aiff,audio/*"
          multiple
          @files="addFiles"
        />
      </section>

      <section v-if="current && current.unmatched > 0" class="imp__review">
        <div class="imp__section-head">
          <h2 class="imp__h2">{{ t('review.title', { n: formatNumber(current.unmatched, lang) }) }}</h2>
          <p class="imp__h2-sub">{{ done ? t('review.sub') : t('review.subRunning') }}</p>
        </div>
        <ImportReview :key="`${current.id}-${reviewKey}`" :import-id="current.id" @changed="refresh" />
      </section>
    </template>

    <section v-if="showHistory.length" class="imp__history">
      <h2 class="imp__h2">{{ t('history.title') }}</h2>
      <div class="imp__hlist">
        <button v-for="h in showHistory" :key="h.id" type="button" class="imp__hrow" @click="open(h)">
          <span class="imp__hrow-icon" aria-hidden="true"><Icon :name="KIND_ICON[h.kind]" /></span>
          <span class="imp__hrow-main">
            <span class="imp__hrow-title ellipsis">{{ h.title }}</span>
            <span class="imp__hrow-sub ellipsis tnum">{{ historySub(h) }}</span>
          </span>
          <span v-if="h.status === 'running'" class="imp__hrow-badge">{{ t('history.live') }}</span>
          <span v-else-if="h.unmatched" class="imp__hrow-badge is-warn">{{ t('history.check', { n: formatNumber(h.unmatched, lang) }) }}</span>
          <Icon name="ph:caret-right" class="imp__hrow-chev" aria-hidden="true" />
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.imp { display: flex; flex-direction: column; gap: 24px; width: 100%; max-width: 760px; margin: 0 auto; padding: 16px 0 48px; }
.imp__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }

/* Три способа — сегменты в одной подложке */
.imp__modes { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 4px; width: min(520px, 100%); padding: 4px; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.imp__mode {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-width: 0;
  height: 40px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--text-2);
  font: inherit;
  font-size: var(--fs-body);
  font-weight: 600;
  cursor: pointer;
  transition: background-color var(--dur-1), color var(--dur-1);
  -webkit-tap-highlight-color: transparent;
}
.imp__mode:hover { color: var(--text-1); }
.imp__mode:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.imp__mode.is-on { background: var(--fill-3); color: var(--text-1); }
.imp__mode-icon { flex: none; font-size: 18px; }

.imp__panel { display: flex; flex-direction: column; gap: 12px; }
.imp__note { margin: 0; font-size: var(--fs-small); line-height: 1.45; color: var(--text-3); text-wrap: pretty; }

/* Список: поле с подвалом и выбор, куда класть */
.imp__area { display: flex; flex-direction: column; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); transition: box-shadow var(--dur-2); }
.imp__area:focus-within { box-shadow: inset 0 0 0 1.5px var(--focus); }
.imp__area.is-over { box-shadow: inset 0 0 0 1.5px var(--accent), 0 0 0 4px var(--accent-soft); }
.imp__text {
  min-height: 220px;
  padding: 16px 18px;
  border: 0;
  border-radius: var(--r-4) var(--r-4) 0 0;
  background: transparent;
  color: var(--text-1);
  font: inherit;
  font-size: var(--fs-body);
  line-height: 1.6;
  resize: vertical;
}
.imp__text::placeholder { color: var(--text-4); }
.imp__text:focus { outline: none; }
.imp__area-foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 8px 8px 18px; box-shadow: inset 0 1px 0 var(--hair); font-size: var(--fs-small); color: var(--text-3); }
.imp__file-btn { display: inline-flex; align-items: center; gap: 6px; height: 36px; padding: 0 14px; border-radius: var(--r-pill); color: var(--text-2); font-weight: 600; cursor: pointer; transition: background-color var(--dur-1), color var(--dur-1); }
.imp__file-btn:hover { background: var(--fill-2); color: var(--text-1); }
.imp__file-btn:focus-within { outline: 2px solid var(--focus); outline-offset: 2px; }
.imp__target { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }
.imp__input {
  flex: 1;
  min-width: 220px;
  height: 40px;
  padding: 0 16px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
  font-size: var(--fs-body);
}
.imp__input::placeholder { color: var(--text-4); }
.imp__input:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.imp__input:focus-visible { outline: none; background: var(--fill-2); box-shadow: inset 0 0 0 1.5px var(--focus); }
.imp__go { align-self: flex-start; }

/* Файлы: строка на файл */
.imp__files { display: flex; flex-direction: column; }
.imp__frow { position: relative; display: flex; align-items: center; gap: 12px; min-height: 60px; padding: 10px 0; }
.imp__frow + .imp__frow { box-shadow: inset 0 1px 0 var(--hair); }
.imp__frow-icon { display: grid; flex: none; place-items: center; width: 36px; height: 36px; border-radius: var(--r-3); background: var(--fill-2); color: var(--text-3); font-size: 18px; }
.imp__frow-icon.is-ok { background: color-mix(in oklab, var(--success) 16%, transparent); color: var(--success); }
.imp__frow-icon.is-warn { background: color-mix(in oklab, var(--warning) 16%, transparent); color: var(--warning); }
.imp__frow-icon.is-bad { background: color-mix(in oklab, var(--danger) 16%, transparent); color: var(--danger); }
.imp__frow-main { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.imp__frow-name { font-size: var(--fs-body); font-weight: 600; }
.imp__frow-state { font-size: var(--fs-small); color: var(--text-3); }
.imp__frow-state.is-ok { color: var(--text-2); }
.imp__frow-state.is-warn { color: var(--warning); }
.imp__frow-state.is-bad { color: var(--danger); }
.imp__frow-meta { flex: none; font-size: var(--fs-small); color: var(--text-3); }
.imp__frow-bar { position: absolute; left: 48px; right: 0; bottom: 0; height: 2px; overflow: hidden; border-radius: 1px; background: var(--fill-2); }
.imp__frow-bar i { display: block; height: 100%; background: var(--accent); transition: width var(--dur-2); }
.imp__more-files { min-height: 0; margin-top: 8px; padding: 16px; }
.imp__more-files :deep(.dz__icon) { display: none; }
.imp__more-files :deep(.dz__button) { margin-top: 8px; }

.imp__review { display: flex; flex-direction: column; gap: 14px; }
.imp__section-head { display: flex; flex-direction: column; gap: 4px; }
.imp__h2 { margin: 0; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; }
.imp__h2-sub { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.imp__forget { margin-left: auto; }

/* Прошлые переносы */
.imp__history { display: flex; flex-direction: column; gap: 12px; }
.imp__hlist { display: flex; flex-direction: column; }
/* Строка нажимается целиком: подсветка выходит за колонку, текст остаётся на месте */
.imp__hrow { position: relative; display: flex; align-items: center; gap: 12px; min-height: 64px; margin: 0 -12px; padding: 10px 12px; border: 0; border-radius: var(--r-3); background: transparent; color: var(--text-1); text-align: left; cursor: pointer; transition: background-color var(--dur-1); }
.imp__hrow + .imp__hrow::before { content: ''; position: absolute; top: 0; left: 64px; right: 12px; height: 1px; background: var(--hair); }
.imp__hrow:active { background: var(--fill-2); }
.imp__hrow:hover { background: var(--fill-1); }
.imp__hrow:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.imp__hrow-icon { display: grid; flex: none; place-items: center; width: 40px; height: 40px; border-radius: var(--r-3); background: var(--fill-2); color: var(--text-2); font-size: 18px; }
.imp__hrow-main { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.imp__hrow-title { font-size: var(--fs-body); font-weight: 600; }
.imp__hrow-sub { font-size: var(--fs-small); color: var(--text-3); }
/* Состояние справа — просто словом, без плашки */
.imp__hrow-badge { flex: none; color: var(--accent); font-size: var(--fs-small); font-weight: 600; }
.imp__hrow-badge.is-warn { color: var(--warning); }
.imp__hrow-chev { flex: none; font-size: 16px; color: var(--text-4); }

@media (max-width: 699px) {
  .imp { gap: 20px; padding: 0 16px 32px; }
  .imp > .pn { margin-right: -16px; margin-bottom: -20px; margin-left: -16px; }
  .imp__mode { height: 44px; padding: 0 8px; }
  .imp__mode-icon { display: none; }
  .imp__title { font-size: 28px; }
  .imp__text { min-height: 180px; padding: 14px 16px; }
  .imp__target { flex-direction: column; align-items: stretch; }
  .imp__input { min-width: 0; height: 44px; }
  .imp__go { align-self: stretch; }
  .imp__frow-meta { display: none; }
  .imp__forget { margin-left: 0; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Перенести музыку",
    "how": "Откуда переносить",
    "mode": { "spotify": "Spotify", "list": "Список", "files": "Файлы" },
    "spotify": {
      "drop": "Перетащите сюда my_spotify_data.zip",
      "dropPhone": "Архив my_spotify_data.zip",
      "dropHint": "Подойдёт и архив расширенной истории",
      "pick": "Выбрать архив",
      "where": "Архив — в Spotify: Аккаунт → Конфиденциальность → «Скачать ваши данные». Приходит на почту за несколько дней.",
      "wrongFile": "Нужен архив my_spotify_data.zip из письма Spotify",
      "tooBig": "Архив больше 500 МБ — такого у Spotify не бывает. Проверьте файл"
    },
    "list": {
      "label": "Список треков",
      "placeholder": "По строке на трек, например:\nLowborn — Devil\nMzlff — Город\n\nИли таблица artist, title — или вставьте M3U",
      "formats": "«Артист — Трек», CSV или M3U",
      "lines": "Строк: {n}",
      "file": "Из файла",
      "where": "Куда добавить",
      "toPlaylist": "В новый плейлист",
      "toLikes": "В «Мне нравится»",
      "titleLabel": "Название плейлиста",
      "titlePlaceholder": "Название плейлиста",
      "go": "Найти треки",
      "tooBig": "Файл больше 4 МБ — разделите его на части"
    },
    "files": {
      "drop": "Перетащите сюда аудиофайлы",
      "dropPhone": "Аудиофайлы",
      "dropHint": "FLAC, ALAC, MP3, Opus, WAV и AIFF",
      "pick": "Выбрать файлы",
      "more": "Добавить ещё файлы",
      "list": "Файлы",
      "waiting": "В очереди",
      "uploading": "Загружается · {pct}\u00a0%",
      "processing": "Узнаём трек…",
      "unmatched": "Не узнали — выберите трек ниже",
      "skipped": "Пропущен",
      "upgraded": "заменил копию хуже",
      "kept": "уже был не хуже",
      "skippedSome": "Пропущено файлов не того формата: {n}"
    },
    "review": {
      "title": "Не нашлось: {n}",
      "sub": "Выберите сами или пропустите — найденное сразу ляжет на своё место",
      "subRunning": "Поиск ещё идёт — можно разбирать уже сейчас"
    },
    "history": {
      "title": "Прошлые переносы",
      "running": "идёт",
      "failed": "не получилось",
      "cancelled": "остановлен",
      "found": "найдено {found}",
      "live": "Идёт",
      "check": "Проверить {n}"
    },
    "again": "Перенести ещё",
    "stop": "Остановить",
    "stopped": "Перенос остановлен",
    "cancelUpload": "Отменить",
    "forget": "Убрать из истории",
    "forgotten": "Убрано из истории — медиатека не изменилась",
    "openPlaylist": "Открыть плейлист",
    "openLibrary": "Открыть медиатеку",
    "uploadFailed": "Не получилось загрузить — попробуйте ещё раз"
  },
  "en": {
    "title": "Bring your music",
    "how": "Where from",
    "mode": { "spotify": "Spotify", "list": "List", "files": "Files" },
    "spotify": {
      "drop": "Drop my_spotify_data.zip here",
      "dropPhone": "my_spotify_data.zip archive",
      "dropHint": "The extended history archive works too",
      "pick": "Choose archive",
      "where": "Get it in Spotify: Account → Privacy → “Download your data”. It arrives by email within a few days.",
      "wrongFile": "Use the my_spotify_data.zip archive from Spotify's email",
      "tooBig": "The archive is over 500 MB — Spotify never sends that much. Check the file"
    },
    "list": {
      "label": "Track list",
      "placeholder": "One track per line, for example:\nLowborn — Devil\nMzlff — Город\n\nOr an artist, title table — or paste an M3U",
      "formats": "“Artist — Track”, CSV or M3U",
      "lines": "Lines: {n}",
      "file": "From file",
      "where": "Add to",
      "toPlaylist": "New playlist",
      "toLikes": "Liked",
      "titleLabel": "Playlist name",
      "titlePlaceholder": "Playlist name",
      "go": "Find tracks",
      "tooBig": "The file is over 4 MB — split it into parts"
    },
    "files": {
      "drop": "Drop audio files here",
      "dropPhone": "Audio files",
      "dropHint": "FLAC, ALAC, MP3, Opus, WAV and AIFF",
      "pick": "Choose files",
      "more": "Add more files",
      "list": "Files",
      "waiting": "Queued",
      "uploading": "Uploading · {pct}%",
      "processing": "Recognising…",
      "unmatched": "Not recognised — pick the track below",
      "skipped": "Skipped",
      "upgraded": "replaced a worse copy",
      "kept": "already had one as good",
      "skippedSome": "Skipped files of the wrong format: {n}"
    },
    "review": {
      "title": "Not found: {n}",
      "sub": "Pick it yourself or skip — whatever you choose goes straight into place",
      "subRunning": "Still searching — you can start sorting now"
    },
    "history": {
      "title": "Earlier transfers",
      "running": "running",
      "failed": "failed",
      "cancelled": "stopped",
      "found": "{found} found",
      "live": "Running",
      "check": "Check {n}"
    },
    "again": "Bring more",
    "stop": "Stop",
    "stopped": "Transfer stopped",
    "cancelUpload": "Cancel",
    "forget": "Remove from history",
    "forgotten": "Removed from history — your library is unchanged",
    "openPlaylist": "Open playlist",
    "openLibrary": "Open library",
    "uploadFailed": "Upload failed — try again"
  }
}
</i18n>
