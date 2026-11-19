<script setup lang="ts">
import { ApiError } from '~/api/types'
import type { ImportItem, ImportKind, ImportSummary } from '~/api/types'

// Перенос музыки: архив данных Spotify, «Яндекс Музыка», YouTube Music, список треков или свои файлы.
// Один спокойный путь: выбрать способ → отдать файл, ссылку или текст → видеть ход → итог одной фразой
// и «не нашлось» с поиском. Сопоставление идёт на сервере в фоне, страницу можно закрыть; прошлые
// переносы — внизу. Логотипов чужих сервисов нет: значок из набора и название словом
type Mode = 'spotify' | 'yandex' | 'youtube' | 'list' | 'files'

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
  { value: 'yandex', icon: 'ph:music-notes' },
  { value: 'youtube', icon: 'ph:play-circle' },
  { value: 'list', icon: 'ph:list-bullets' },
  { value: 'files', icon: 'ph:waveform' },
]
// YouTube Music переносится через серверный загрузчик: без него этого способа нет
const server = useServerStore()
const modes = computed(() => MODES.filter(m => m.value !== 'youtube' || server.downloader))
const initial = route.query.mode
const mode = ref<Mode>(modes.value.find(m => m.value === initial)?.value ?? 'spotify')
watch(mode, m => void navigateTo({ query: { ...route.query, mode: m } }, { replace: true }))

// На телефоне ряд способов листается вбок: выбранный всегда целиком на виду
const modesEl = ref<HTMLElement | null>(null)
function revealMode(smooth: boolean) {
  const row = modesEl.value
  const on = row?.querySelector<HTMLElement>('.is-on')
  if (!row || !on || row.scrollWidth <= row.clientWidth) return
  const pad = 16
  // Ряд позиционирован: offsetLeft кнопки — от его левого края
  const left = on.offsetLeft
  const target = left < row.scrollLeft + pad
    ? left - pad
    : left + on.offsetWidth > row.scrollLeft + row.clientWidth - pad
      ? left + on.offsetWidth - row.clientWidth + pad
      : row.scrollLeft
  row.scrollTo({ left: target, behavior: smooth ? 'smooth' : 'auto' })
}
watch(mode, () => nextTick(() => revealMode(true)))

function modeOf(kind: ImportKind): Mode {
  switch (kind) {
    case 'spotify_archive': return 'spotify'
    case 'yandex': return 'yandex'
    case 'youtube_playlist':
    case 'youtube_takeout': return 'youtube'
    case 'files': return 'files'
    default: return 'list'
  }
}

/** Код ошибки сервера — для понятной подсказки на месте, а не всплывашки */
function errorCode(e: unknown): string {
  return e instanceof ApiError ? e.code : ''
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

// ---------- «Яндекс Музыка» ----------

// Вход «Яндекс Музыки», как у известных клиентов: после входа Яндекс возвращает на свою страницу
// с ключом в адресе. Человек копирует адрес целиком — сервер достанет ключ сам и нигде его не сохранит
const YANDEX_LOGIN = 'https://oauth.yandex.ru/authorize?response_type=token&client_id=23cabbbdc6cd418abb4b39c32c41195d'
const ymUrl = ref('')
const ymKey = ref('')
const ymLogin = ref(false)
const ymBusy = ref(false)
const ymError = ref('')
const ymReady = computed(() => !!ymUrl.value.trim() || (ymLogin.value && !!ymKey.value.trim()))

async function startYandex() {
  if (!ymReady.value || ymBusy.value) return
  ymError.value = ''
  ymBusy.value = true
  try {
    const s = await bridge.imports.yandex({
      url: ymUrl.value.trim() || null,
      token: ymLogin.value ? ymKey.value.trim() || null : null,
    })
    reset()
    current.value = s
    active.value = true
    ymUrl.value = ''
    ymKey.value = ''
    ymLogin.value = false
    void loadHistory()
  }
  catch (e) {
    const code = errorCode(e)
    if (code === 'yandex_login_required') {
      ymLogin.value = true
      ymError.value = t('yandex.needLogin')
    }
    else if (code === 'yandex_token' || code === 'yandex_token_rejected') ymError.value = t('yandex.badKey')
    else if (code === 'yandex_link') ymError.value = t('yandex.badLink')
    else if (code === 'yandex_not_found') ymError.value = t('yandex.notFound')
    else if (code === 'yandex_region') ymError.value = t('yandex.region')
    else toast.error((e as Error).message || t('uploadFailed'))
  }
  finally {
    ymBusy.value = false
  }
}
watch([ymUrl, ymKey], () => { ymError.value = '' })

// ---------- YouTube Music ----------

const ytUrl = ref('')
const ytBusy = ref(false)
const ytError = ref('')

async function startYoutube() {
  if (!ytUrl.value.trim() || ytBusy.value) return
  ytError.value = ''
  ytBusy.value = true
  try {
    const s = await bridge.imports.youtube(ytUrl.value.trim())
    reset()
    current.value = s
    active.value = true
    ytUrl.value = ''
    void loadHistory()
  }
  catch (e) {
    const code = errorCode(e)
    if (code === 'youtube_link') ytError.value = t('youtube.badLink')
    else if (code === 'youtube_not_found') ytError.value = t('youtube.notFound')
    else toast.error((e as Error).message || t('uploadFailed'))
  }
  finally {
    ytBusy.value = false
  }
}
watch(ytUrl, () => { ytError.value = '' })

async function startTakeout(list: File[]) {
  const file = list[0]
  if (!file) return
  if (!/\.zip$/i.test(file.name)) {
    toast.error(t('youtube.wrongFile'))
    return
  }
  if (file.size > 500 * 1024 * 1024) {
    toast.error(t('youtube.tooBig'))
    return
  }
  reset()
  active.value = true
  fileName.value = file.name
  uploadShare.value = 0
  abort = new AbortController()
  try {
    current.value = await bridge.imports.takeout(file, (s) => { uploadShare.value = s }, abort.signal)
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
  revealMode(false)
  await loadHistory()
  const q = route.query.id
  const found = typeof q === 'string' ? history.value.find(h => h.id === q) : history.value.find(h => h.status === 'running')
  if (found) open(found)
})
onBeforeUnmount(() => abort?.abort())

const KIND_ICON: Record<ImportKind, string> = {
  spotify_archive: 'ph:spotify-logo',
  yandex: 'ph:music-notes',
  youtube_playlist: 'ph:play-circle',
  youtube_takeout: 'ph:file-zip',
  text: 'ph:list-bullets',
  csv: 'ph:table',
  m3u: 'ph:playlist',
  files: 'ph:waveform',
}
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
  if (s.kind === 'spotify_archive' || s.kind === 'yandex' || s.kind === 'youtube_takeout' || s.stats.likes) return { to: '/library', label: t('openLibrary') }
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
      <div ref="modesEl" class="imp__modes" role="radiogroup" :aria-label="t('how')">
        <button
          v-for="m in modes"
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

      <section v-else-if="mode === 'yandex'" class="imp__panel">
        <form class="imp__link" @submit.prevent="startYandex">
          <input
            v-model="ymUrl"
            class="imp__input"
            type="text"
            inputmode="url"
            autocomplete="off"
            spellcheck="false"
            :placeholder="t('yandex.placeholder')"
            :aria-label="t('yandex.label')"
          >
          <UiButton v-if="!ymLogin" type="submit" variant="primary" :loading="ymBusy" :disabled="!ymReady" :block="ui.isPhone" class="imp__link-go">{{ t('go') }}</UiButton>
        </form>
        <p v-if="ymError && !ymLogin" class="imp__error" role="alert">{{ ymError }}</p>

        <!-- Вход нужен, если Яндекс не отдаёт плейлист без него, и для всего аккаунта сразу -->
        <button v-if="!ymLogin" type="button" class="imp__more" @click="ymLogin = true">
          <Icon name="ph:user-circle" aria-hidden="true" />{{ t('yandex.all') }}
        </button>
        <div v-else class="imp__login">
          <p v-if="ymError" class="imp__error imp__error--top" role="alert">{{ ymError }}</p>
          <ol class="imp__steps">
            <li>
              <span>{{ t('yandex.step1') }}</span>
              <UiButton size="sm" icon-right="ph:arrow-up-right" :href="YANDEX_LOGIN" target="_blank" rel="noopener noreferrer">{{ t('yandex.open') }}</UiButton>
            </li>
            <li><span>{{ t('yandex.step2') }}</span></li>
          </ol>
          <input
            v-model="ymKey"
            class="imp__input"
            type="password"
            autocomplete="off"
            spellcheck="false"
            :placeholder="t('yandex.keyPlaceholder')"
            :aria-label="t('yandex.keyLabel')"
            @keydown.enter.prevent="startYandex"
          >
          <p class="imp__note">{{ t('yandex.keyNote') }}</p>
          <!-- С входом кнопка — после всех шагов, по порядку чтения -->
          <UiButton variant="primary" size="lg" :loading="ymBusy" :disabled="!ymReady" :block="ui.isPhone" class="imp__go" @click="startYandex">{{ t('go') }}</UiButton>
        </div>
      </section>

      <section v-else-if="mode === 'youtube'" class="imp__panel">
        <form class="imp__link" @submit.prevent="startYoutube">
          <input
            v-model="ytUrl"
            class="imp__input"
            type="text"
            inputmode="url"
            autocomplete="off"
            spellcheck="false"
            :placeholder="t('youtube.placeholder')"
            :aria-label="t('youtube.label')"
          >
          <UiButton type="submit" variant="primary" :loading="ytBusy" :disabled="!ytUrl.trim()" :block="ui.isPhone" class="imp__link-go">{{ t('go') }}</UiButton>
        </form>
        <p v-if="ytError" class="imp__error" role="alert">{{ ytError }}</p>
        <div class="imp__or" role="separator"><span>{{ t('or') }}</span></div>
        <ImportDropZone
          :title="ui.isPhone ? t('youtube.dropPhone') : t('youtube.drop')"
          :hint="t('youtube.dropHint')"
          :button="t('youtube.pick')"
          icon="ph:file-zip"
          accept=".zip,application/zip"
          @files="startTakeout"
        />
        <p class="imp__note">{{ t('youtube.where') }}</p>
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

/* Способы — сегменты в одной подложке; на телефоне — ряд, который листается вбок */
.imp__modes { position: relative; display: flex; gap: 4px; width: fit-content; max-width: 100%; padding: 4px; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.imp__mode {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  gap: 8px;
  white-space: nowrap;
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

/* Ссылка: поле и кнопка в строку */
.imp__link { display: flex; gap: 8px; }
.imp__link .imp__input { flex: 1 1 auto; min-width: 0; height: 44px; }
.imp__link-go { flex: none; height: 44px; }
.imp__error { margin: -4px 0 0; padding-left: 16px; font-size: var(--fs-small); color: var(--warning); text-wrap: pretty; }
.imp__error--top { margin: 0; padding-left: 0; }
.imp__more {
  display: inline-flex;
  align-self: flex-start;
  align-items: center;
  gap: 8px;
  height: 36px;
  margin-left: -6px;
  padding: 0 10px;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--text-2);
  font: inherit;
  font-size: var(--fs-small);
  font-weight: 600;
  cursor: pointer;
  transition: background-color var(--dur-1), color var(--dur-1);
}
.imp__more:hover { background: var(--fill-1); color: var(--text-1); }
.imp__more:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.imp__more :deep(svg) { font-size: 18px; }
/* Вход в Яндекс: два шага и поле — без отдельной карточки, под волоском */
.imp__login { display: flex; flex-direction: column; gap: 12px; padding-top: 16px; box-shadow: inset 0 1px 0 var(--hair); }
.imp__login .imp__input { flex: none; height: 44px; }
.imp__steps { display: flex; flex-direction: column; gap: 10px; margin: 0; padding: 0; list-style: none; counter-reset: step; }
.imp__steps li { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; min-height: 32px; font-size: var(--fs-body); color: var(--text-2); counter-increment: step; }
.imp__steps li::before { content: counter(step); display: grid; flex: none; place-items: center; width: 24px; height: 24px; border-radius: 50%; background: var(--fill-2); color: var(--text-2); font-size: var(--fs-caption); font-weight: 650; }
.imp__steps li > span { flex: 1 1 0; min-width: 160px; text-wrap: pretty; }
/* «или» между ссылкой и архивом */
.imp__or { display: flex; align-items: center; gap: 12px; margin: 8px 0 4px; font-size: var(--fs-small); color: var(--text-3); }
.imp__or::before, .imp__or::after { content: ''; flex: 1; height: 1px; background: var(--hair); }

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
  /* Пять способов в ширину телефона не помещаются: ряд листается вбок, подписи целиком */
  .imp__modes { width: auto; max-width: none; margin: 0 -16px; padding: 0 16px; overflow-x: auto; gap: 8px; border-radius: 0; background: none; box-shadow: none; scrollbar-width: none; overscroll-behavior-x: contain; }
  .imp__modes::-webkit-scrollbar { display: none; }
  .imp__mode { height: 44px; padding: 0 18px; box-shadow: inset 0 0 0 1px var(--line); }
  .imp__mode.is-on { box-shadow: none; }
  .imp__mode-icon { display: none; }
  .imp__link { flex-direction: column; }
  .imp__link .imp__input { flex: none; }
  .imp__more { height: 44px; }
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
    "mode": { "spotify": "Spotify", "yandex": "Яндекс Музыка", "youtube": "YouTube Music", "list": "Список", "files": "Файлы" },
    "go": "Перенести",
    "or": "или",
    "yandex": {
      "label": "Ссылка на плейлист «Яндекс Музыки»",
      "placeholder": "Ссылка на плейлист",
      "all": "Перенести всё: лайки и плейлисты",
      "step1": "Войдите в Яндекс",
      "open": "Открыть",
      "step2": "Скопируйте адрес страницы, куда вас вернёт Яндекс, и вставьте сюда",
      "keyLabel": "Адрес после входа",
      "keyPlaceholder": "Адрес после входа",
      "keyNote": "Без ссылки перенесутся «Мне нравится» и все ваши плейлисты. Вход нужен один раз — мы его не сохраняем.",
      "needLogin": "Яндекс отдаёт музыку только после входа — это два шага ниже",
      "badKey": "Вход не подошёл — войдите ещё раз и скопируйте адрес целиком",
      "badLink": "Это не ссылка на плейлист «Яндекс Музыки»",
      "notFound": "Плейлист не открылся — он скрыт или удалён",
      "region": "Яндекс сейчас не отдаёт музыку — попробуйте позже"
    },
    "youtube": {
      "label": "Ссылка на плейлист YouTube Music",
      "placeholder": "Ссылка на плейлист",
      "badLink": "Нужна ссылка на плейлист — в ней есть «list=»",
      "notFound": "Плейлист не открылся — он закрыт или удалён",
      "drop": "Перетащите сюда архив Google Takeout",
      "dropPhone": "Архив Google Takeout",
      "dropHint": "Лайки, библиотека и все плейлисты",
      "pick": "Выбрать архив",
      "where": "Архив — на takeout.google.com: оставьте только «YouTube и YouTube Music», а в его данных — плейлисты и музыкальную библиотеку. Приходит на почту.",
      "wrongFile": "Нужен ZIP-архив из Google Takeout",
      "tooBig": "Архив больше 500 МБ — оставьте в Takeout только плейлисты и музыкальную библиотеку"
    },
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
    "mode": { "spotify": "Spotify", "yandex": "Yandex Music", "youtube": "YouTube Music", "list": "List", "files": "Files" },
    "go": "Bring over",
    "or": "or",
    "yandex": {
      "label": "Yandex Music playlist link",
      "placeholder": "Playlist link",
      "all": "Bring everything: likes and playlists",
      "step1": "Sign in to Yandex",
      "open": "Open",
      "step2": "Copy the address of the page Yandex returns you to and paste it here",
      "keyLabel": "Address after sign-in",
      "keyPlaceholder": "Address after sign-in",
      "keyNote": "Without a link, your Liked tracks and all your playlists come over. Sign-in is used once — we don't keep it.",
      "needLogin": "Yandex only shares music after you sign in — two steps below",
      "badKey": "Sign-in didn't work — sign in again and copy the whole address",
      "badLink": "That's not a Yandex Music playlist link",
      "notFound": "The playlist didn't open — it's hidden or deleted",
      "region": "Yandex isn't sharing music right now — try later"
    },
    "youtube": {
      "label": "YouTube Music playlist link",
      "placeholder": "Playlist link",
      "badLink": "Use a playlist link — it has “list=” in it",
      "notFound": "The playlist didn't open — it's private or deleted",
      "drop": "Drop your Google Takeout archive here",
      "dropPhone": "Google Takeout archive",
      "dropHint": "Likes, library and all playlists",
      "pick": "Choose archive",
      "where": "Get it at takeout.google.com: keep only “YouTube and YouTube Music”, and in its data — playlists and music library. It arrives by email.",
      "wrongFile": "Use the ZIP archive from Google Takeout",
      "tooBig": "The archive is over 500 MB — keep only playlists and the music library in Takeout"
    },
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
