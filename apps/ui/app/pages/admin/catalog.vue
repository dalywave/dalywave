<script setup lang="ts">
import type { AdminRecording, AudioStatus } from '~/api/types'
import type { ViewPreset, ViewState } from '~/components/admin/SavedViews.vue'
import type { MenuEntry } from '~/types/ui'

// Каталог: записи на сервере с техническими данными — файл, формат, битрейт, громкость, источник,
// оценка совпадения. Фильтры по состоянию, источнику, проверке и поиск; перекачка поиском
// или заменой по ссылке
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const server = useServerStore()
const lang = useLang()
const L = useAdminLabels()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const items = ref<AdminRecording[] | null>(null)
const cursor = ref<string | null>(null)
const failed = ref(false)
// Поиск по админке ведёт сюда с готовым запросом: /admin/catalog?q=…
const route = useRoute()
const q = ref(typeof route.query.q === 'string' ? route.query.q : '')
watch(() => route.query.q, (v) => {
  if (typeof v === 'string' && v !== q.value) q.value = v
})
const status = ref<'' | AudioStatus>(q.value ? '' : 'ready')
const source = ref('')
const verified = ref<'all' | 'yes' | 'no'>('all')

const query = () => ({
  q: q.value.trim() || undefined,
  status: status.value || undefined,
  source: source.value || undefined,
  verified: verified.value === 'all' ? undefined : verified.value === 'yes',
  limit: 50,
})
let seq = 0
async function load() {
  const my = ++seq
  failed.value = false
  try {
    const p = await bridge.admin.recordings(query())
    if (my !== seq) return
    items.value = p.items
    cursor.value = p.nextCursor
  }
  catch {
    if (my === seq) failed.value = true
  }
}
onMounted(load)
watchDebounced(q, () => void load(), { debounce: 300 })

// Виды: готовые и свои (сохранённые фильтры)
const viewState = computed<ViewState>(() => ({ q: q.value.trim(), status: status.value, source: source.value, verified: verified.value === 'all' ? '' : verified.value }))
const viewPresets = computed<ViewPreset[]>(() => [
  { key: 'ready', label: t('view.ready'), state: { status: 'ready' } },
  { key: 'unverified', label: t('view.unverified'), state: { status: 'ready', verified: 'no' } },
  { key: 'failed', label: t('view.failed'), state: { status: 'failed' } },
  { key: 'disputed', label: t('view.disputed'), state: { status: 'disputed' } },
  { key: 'all', label: t('view.all'), state: {} },
])
function applyView(s: ViewState) {
  q.value = typeof s.q === 'string' ? s.q : ''
  status.value = (typeof s.status === 'string' ? s.status : '') as '' | AudioStatus
  source.value = typeof s.source === 'string' ? s.source : ''
  verified.value = s.verified === 'yes' || s.verified === 'no' ? s.verified : 'all'
}
watch([status, source, verified], () => void load())

const more = ref(false)
async function loadMore() {
  if (!cursor.value) return
  more.value = true
  try {
    const p = await bridge.admin.recordings({ ...query(), cursor: cursor.value })
    items.value = [...(items.value ?? []), ...p.items]
    cursor.value = p.nextCursor
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    more.value = false
  }
}

const STATUSES: AudioStatus[] = ['ready', 'queued', 'acquiring', 'disputed', 'unavailable', 'failed', 'none']
const SOURCES = ['local', 'youtube_music', 'youtube', 'soundcloud', 'bandcamp', 'upload', 'other']
const sourceLabel = (s: string) => t(`source.${s}`)
const statusOptions = computed(() => [{ value: '' as const, label: t('anyStatus') }, ...STATUSES.map(s => ({ value: s, label: L.audio(s) }))])
const sourceOptions = computed(() => [{ value: '', label: t('anySource') }, ...SOURCES.map(s => ({ value: s, label: sourceLabel(s) }))])

// Кодек — строкой выше, битрейт и частота — подписью: колонка остаётся узкой и читается сверху вниз
function format(r: AdminRecording) {
  const f = r.file
  if (!f) return null
  return qualityLabel({ codec: f.codec }) ?? f.codec
}
function formatSub(r: AdminRecording) {
  const f = r.file
  if (!f) return ''
  const rate = f.lossless ? null : f.bitrateKbps ? `${f.bitrateKbps}\u00a0${t('kbps')}` : null
  const khz = f.sampleRate ? `${num(f.sampleRate / 1000)}\u00a0${t('khz')}` : f.container
  return [rate, khz].filter(Boolean).join(' · ') + (r.files > 1 ? ` · ${t('files', { n: r.files })}` : '')
}
// Настоящий минус, а не дефис: «−5,8 LUFS»
const num = (v: number, digits = 1) => v.toLocaleString(intlLocale(lang.value, 'en-US'), { maximumFractionDigits: digits, minimumFractionDigits: digits }).replace('-', '−')
function sourceLink(r: AdminRecording) {
  const ref = r.file?.sourceRef
  if (!ref) return null
  if (/^https?:\/\//.test(ref)) return ref
  return /^[\w-]{11}$/.test(ref) ? `https://music.youtube.com/watch?v=${ref}` : null
}

// ---------- Перекачка ----------

const replacing = ref<string | null>(null)
const replaceUrl = ref('')
const busy = ref<string | null>(null)
async function reacquire(r: AdminRecording, url?: string) {
  busy.value = r.id
  try {
    await bridge.admin.reacquire(r.id, url)
    toast.show({ text: url ? t('replaceQueued', { title: r.track?.title ?? '' }) : t('reacquireQueued', { title: r.track?.title ?? '' }), icon: 'ph:download-simple' })
    replacing.value = null
    replaceUrl.value = ''
    await load()
  }
  catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : t('failed'))
  }
  finally {
    busy.value = null
  }
}
function menu(r: AdminRecording): MenuEntry[] {
  const link = sourceLink(r)
  return [
    { kind: 'heading', label: r.track?.title ?? t('noTrack'), hint: r.isrc ?? undefined },
    // Скачать заново и заменить по ссылке — работа загрузчика
    ...(server.downloader
      ? [
          { label: t('replace'), icon: 'ph:link', onSelect: () => (replacing.value = r.id) },
          { label: t('reacquire'), icon: 'ph:arrow-clockwise', onSelect: () => reacquire(r) },
        ]
      : []),
    ...(link ? [{ label: t('openSource'), icon: 'ph:arrow-square-out', onSelect: () => void window.open(link, '_blank', 'noopener') }] : []),
    ...(r.track?.album ? [{ label: t('openAlbum'), icon: 'ph:vinyl-record', onSelect: () => navigateTo(`/album/${r.track!.album!.id}`) }] : []),
  ]
}
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminSavedViews storage-key="catalog" :presets="viewPresets" :state="viewState" class="cat__views" @apply="applyView" />
    <div class="ap-filters cat__filters">
      <input v-model="q" class="ap-input cat__search" type="search" :placeholder="t('search')" :aria-label="t('search')">
      <AdminSelect v-model="status" :options="statusOptions" :label="t('status')" />
      <AdminSelect v-model="source" :options="sourceOptions" :label="t('sourceLabel')" />
      <UiSegmentedControl v-model="verified" :options="[{ value: 'all', label: t('verifiedAll') }, { value: 'yes', label: t('verifiedYes') }, { value: 'no', label: t('verifiedNo') }]" :label="t('verifiedLabel')" size="sm" />
    </div>

    <AdminState :loading="!items && !failed" :error="failed" :empty="!!items && !items.length" empty-icon="ph:vinyl-record" :empty-title="t('empty')" :rows="8" @retry="load">
      <div class="ap-table cat">
        <div class="ap-tr ap-tr--head">
          <span>{{ t('col.track') }}</span>
          <span>{{ t('col.status') }}</span>
          <span>{{ t('col.format') }}</span>
          <span class="ap-num">{{ t('col.loudness') }}</span>
          <span class="cat__src">{{ t('col.source') }}</span>
          <span class="ap-num">{{ t('col.match') }}</span>
          <span class="ap-num">{{ t('col.size') }}</span>
          <span />
        </div>
        <template v-for="r in items" :key="r.id">
          <div class="ap-tr">
            <span class="ap-cell">
              <b>{{ r.track?.title ?? t('noTrack') }}</b>
              <small>{{ r.track ? artistNames(r.track.artists) : r.isrc }}<template v-if="r.tracks > 1"> · {{ t('sharedBy', { n: r.tracks }) }}</template></small>
            </span>
            <AdminStatus :color="AUDIO_COLOR[r.audioStatus]" :tone="r.audioStatus === 'failed' ? 'bad' : r.audioStatus === 'none' || r.audioStatus === 'unavailable' ? 'idle' : 'ok'">{{ L.audio(r.audioStatus) }}</AdminStatus>
            <span class="ap-cell">
              <b>{{ format(r) ?? '—' }}</b>
              <small v-if="r.file">{{ formatSub(r) }}</small>
            </span>
            <span class="ap-num ap-cell">
              <b>{{ r.file?.loudnessLufs != null ? `${num(r.file.loudnessLufs)}\u00a0LUFS` : '—' }}</b>
              <small v-if="r.file?.truePeakDb != null">{{ t('peak', { v: num(r.file.truePeakDb) }) }}</small>
            </span>
            <span class="ap-cell cat__src">
              <b>{{ r.file ? sourceLabel(r.file.source) : '—' }}</b>
              <small v-if="r.file?.sourceTitle" :title="r.file.sourceTitle">{{ r.file.sourceTitle }}</small>
            </span>
            <span class="ap-num ap-cell">
              <b :class="r.file?.verified ? 'ap-ok' : ''">{{ r.file?.matchScore != null ? `${Math.round(r.file.matchScore * 100)}` : '—' }}</b>
              <small v-if="r.file">{{ r.file.verified ? t('verified') : t('unverified') }}</small>
            </span>
            <span class="ap-num ap-muted">{{ r.file ? formatBytes(r.file.sizeBytes, lang) : '—' }}</span>
            <span class="ap-end">
              <UiMenu :items="menu(r)"><UiIconButton icon="ph:dots-three" :label="t('actions')" size="sm" :tooltip="false" /></UiMenu>
            </span>
          </div>
          <form v-if="replacing === r.id" class="cat__replace" @submit.prevent="reacquire(r, replaceUrl.trim())">
            <input v-model="replaceUrl" class="ap-input" type="url" autofocus :placeholder="t('replacePlaceholder')" :aria-label="t('replace')">
            <UiButton type="submit" :disabled="!replaceUrl.trim()" :loading="busy === r.id">{{ t('replaceGo') }}</UiButton>
            <UiButton variant="ghost" @click="replacing = null">{{ t('cancel') }}</UiButton>
          </form>
        </template>
        <div v-if="cursor" class="ap-more"><UiButton variant="ghost" size="sm" :loading="more" @click="loadMore">{{ t('more') }}</UiButton></div>
      </div>
    </AdminState>
  </div>
</template>

<style scoped>
.cat__search { flex: 1 1 240px; max-width: 360px; }
.cat { --cols: minmax(160px, 1.6fr) minmax(120px, 0.9fr) minmax(120px, 0.9fr) 104px minmax(110px, 1fr) 96px 72px 40px; }
.cat__src { padding-left: 16px; }
.cat .ap-cell b:only-child { font-weight: 500; }
.cat__replace { display: flex; flex-wrap: wrap; gap: 8px; padding: 0 0 12px; }
.cat__replace .ap-input { flex: 1 1 280px; }
@media (max-width: 1399px) {
  .cat { --cols: minmax(150px, 1.5fr) minmax(110px, 0.9fr) minmax(110px, 0.8fr) 96px minmax(100px, 1fr) 88px 40px; }
  .cat .ap-tr > :nth-child(7) { display: none; }
}
@media (max-width: 1199px) {
  .cat { --cols: minmax(150px, 1.5fr) minmax(110px, 0.9fr) minmax(90px, 0.8fr) minmax(100px, 1fr) 88px 40px; }
  .cat .ap-tr > :nth-child(4), .cat .ap-tr > :nth-child(7) { display: none; }
}
@media (max-width: 699px) {
  .cat__filters { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); margin-bottom: -8px; }
  .cat__filters .ap-input, .cat__filters .asl { width: 100%; height: 44px; }
  .cat__search { grid-column: 1 / -1; max-width: none; }
  .cat__filters > :last-child { grid-column: 1 / -1; justify-self: start; }
  .cat { --cols: minmax(0, 1fr) minmax(0, 0.8fr) 36px; }
  .cat .ap-tr > :nth-child(n+3):nth-child(-n+7) { display: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "view": { "ready": "Готовые", "unverified": "Не проверены", "failed": "С ошибкой", "disputed": "Спорные", "all": "Все записи" },
    "title": "Каталог",
    "search": "Название или артист",
    "status": "Состояние",
    "anyStatus": "С файлом или в работе",
    "sourceLabel": "Источник",
    "anySource": "Любой источник",
    "verifiedLabel": "Проверка",
    "verifiedAll": "Все",
    "verifiedYes": "Проверены",
    "verifiedNo": "Без проверки",
    "empty": "Таких записей нет",
    "failed": "Не получилось — попробуйте ещё раз",
    "more": "Показать ещё",
    "noTrack": "Трек удалён из каталога",
    "sharedBy": "треков: {n}",
    "files": "файлов: {n}",
    "kbps": "кбит/с",
    "khz": "кГц",
    "peak": "пик {v} дБ",
    "verified": "сверен",
    "unverified": "без проверки",
    "actions": "Действия",
    "replace": "Заменить по ссылке",
    "replacePlaceholder": "Ссылка: YouTube, SoundCloud, Bandcamp…",
    "replaceGo": "Заменить",
    "cancel": "Отмена",
    "reacquire": "Скачать заново",
    "openSource": "Открыть источник",
    "openAlbum": "Открыть альбом",
    "replaceQueued": "«{title}» скачается по ссылке и заменит файл",
    "reacquireQueued": "«{title}» поищется заново; файл заменится, если найдётся лучше",
    "col": { "track": "Запись", "status": "Состояние", "format": "Формат", "loudness": "Громкость", "source": "Источник", "match": "Совпадение", "size": "Размер" },
    "source": { "youtube_music": "YouTube Music", "youtube": "YouTube", "soundcloud": "SoundCloud", "bandcamp": "Bandcamp", "upload": "Загружен", "local": "Своя папка", "other": "Другое" }
  },
  "uk": {
    "view": { "ready": "Готові", "unverified": "Не перевірені", "failed": "З помилкою", "disputed": "Спірні", "all": "Усі записи" },
    "title": "Каталог",
    "search": "Назва або виконавець",
    "status": "Стан",
    "anyStatus": "З файлом або в роботі",
    "sourceLabel": "Джерело",
    "anySource": "Будь-яке джерело",
    "verifiedLabel": "Перевірка",
    "verifiedAll": "Усі",
    "verifiedYes": "Перевірені",
    "verifiedNo": "Без перевірки",
    "empty": "Таких записів немає",
    "failed": "Не вдалося — спробуйте ще раз",
    "more": "Показати ще",
    "noTrack": "Трек видалено з каталогу",
    "sharedBy": "треків: {n}",
    "files": "файлів: {n}",
    "kbps": "кбіт/с",
    "khz": "кГц",
    "peak": "пік {v} дБ",
    "verified": "звірено",
    "unverified": "без перевірки",
    "actions": "Дії",
    "replace": "Замінити за посиланням",
    "replacePlaceholder": "Посилання: YouTube, SoundCloud, Bandcamp…",
    "replaceGo": "Замінити",
    "cancel": "Скасувати",
    "reacquire": "Завантажити наново",
    "openSource": "Відкрити джерело",
    "openAlbum": "Відкрити альбом",
    "replaceQueued": "«{title}» завантажиться за посиланням і замінить файл",
    "reacquireQueued": "«{title}» шукатиметься наново; файл заміниться, якщо знайдеться кращий",
    "col": { "track": "Запис", "status": "Стан", "format": "Формат", "loudness": "Гучність", "source": "Джерело", "match": "Збіг", "size": "Розмір" },
    "source": { "youtube_music": "YouTube Music", "youtube": "YouTube", "soundcloud": "SoundCloud", "bandcamp": "Bandcamp", "upload": "Завантажено", "local": "Власна тека", "other": "Інше" }
  },
  "en": {
    "view": { "ready": "Ready", "unverified": "Not verified", "failed": "Failed", "disputed": "Disputed", "all": "All recordings" },
    "title": "Catalog",
    "search": "Title or artist",
    "status": "State",
    "anyStatus": "With a file or in progress",
    "sourceLabel": "Source",
    "anySource": "Any source",
    "verifiedLabel": "Verification",
    "verifiedAll": "All",
    "verifiedYes": "Verified",
    "verifiedNo": "Unverified",
    "empty": "No such recordings",
    "failed": "Didn't work — try again",
    "more": "Show more",
    "noTrack": "Track removed from the catalog",
    "sharedBy": "tracks: {n}",
    "files": "files: {n}",
    "kbps": "kbps",
    "khz": "kHz",
    "peak": "peak {v} dB",
    "verified": "verified",
    "unverified": "unverified",
    "actions": "Actions",
    "replace": "Replace from a link",
    "replacePlaceholder": "Link: YouTube, SoundCloud, Bandcamp…",
    "replaceGo": "Replace",
    "cancel": "Cancel",
    "reacquire": "Download again",
    "openSource": "Open the source",
    "openAlbum": "Open the album",
    "replaceQueued": "“{title}” will download from the link and replace the file",
    "reacquireQueued": "“{title}” will be searched again; the file is replaced if a better one turns up",
    "col": { "track": "Recording", "status": "State", "format": "Format", "loudness": "Loudness", "source": "Source", "match": "Match", "size": "Size" },
    "source": { "youtube_music": "YouTube Music", "youtube": "YouTube", "soundcloud": "SoundCloud", "bandcamp": "Bandcamp", "upload": "Uploaded", "local": "Music folder", "other": "Other" }
  }
}
</i18n>
