<script setup lang="ts">
import type { LyricsLine } from '~/api/types'
import type { SourceResult } from '~/bridge/types'
import type { StoryLength } from '~/lib/story/moment'
import { ApiError } from '~/api/types'
import { PREVIEW_MS, channelPeaks, clampStart, storyLines } from '~/lib/story/moment'
import { resamplePeaks } from '~/utils/peaks'

// «Поделиться историей»: вертикальное видео 10, 15 или 30 с для Instagram Stories и TikTok — обложка,
// название, строка текста и живая волна в цвет обложки под звук. Сначала человек выбирает момент
// (StoryMoment: окно по форме волны, по умолчанию — припев, можно послушать), потом видео собирается
// на устройстве (lib/story/render.ts); готовое — в системное «Поделиться» или файлом
type Phase = 'pick' | 'prepare' | 'render' | 'upload' | 'ready' | 'error'

/** Звук с сервера: Deezer или хранилище и ffmpeg */
const PREPARE_MS = 60_000
const DECODE_MS = 20_000
/** Видео без звука туда и готовое обратно */
const UPLOAD_MS = 120_000
/** Столбиков в форме волны редактора */
const BARS = 96
/** Прослушивание: мягкий вход и затухание в конце, мс — как у «Послушать припев» */
const FADE_IN_MS = 250
const FADE_OUT_MS = 900

const { track, open } = useStory()
const bridge = useBridge()
const player = usePlayerStore()
const ui = useUiStore()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })

const phase = ref<Phase>('pick')
const progress = ref(0)
const preview = ref<string | null>(null)
const videoUrl = ref<string | null>(null)
const error = ref('')
/** Звука для истории нет совсем — повторять нечего */
const noAudio = computed(() => error.value === t('noAudio'))
let blob: Blob | null = null
let abort: AbortController | null = null

// ---------- Момент ----------

/** Что известно о звуке трека: целиком на сервере (`full`) или только превью Deezer */
interface MomentInfo {
  full: boolean
  totalMs: number
  chorusMs: number | null
  previewUrl: string | null
  lines: LyricsLine[]
}
const info = shallowRef<MomentInfo | null>(null)
const peaks = ref<number[]>([])
const startMs = ref(0)
const lengthMs = ref<StoryLength>(15_000)
let loadRun = 0

const lyric = computed(() => {
  const i = info.value
  if (!i?.full || !i.lines.length) return undefined
  return storyLines(i.lines, startMs.value, lengthMs.value).find(l => l.text.trim())?.text ?? null
})

async function loadMoment() {
  const tr = track.value
  if (!tr) return
  const run = ++loadRun
  info.value = null
  peaks.value = []
  phase.value = 'pick'
  error.value = ''
  try {
    const [hook, lyrics] = await Promise.all([
      bridge.catalog.hook(tr.id),
      bridge.catalog.lyrics(tr.id).catch(() => null),
    ])
    if (run !== loadRun) return
    if (!hook.ready && !hook.previewUrl) throw new ApiError(404, 'no_audio', 'no audio')
    const full = hook.ready
    const totalMs = full ? tr.durationMs : PREVIEW_MS
    info.value = {
      full,
      totalMs,
      chorusMs: full ? hook.startMs : null,
      previewUrl: hook.previewUrl ?? null,
      // Время строк известно, только когда звук — весь трек (у превью Deezer начало неизвестно)
      lines: full && lyrics?.kind === 'synced' ? lyrics.lines : [],
    }
    startMs.value = clampStart(full ? hook.startMs : 0, lengthMs.value, totalMs)
    peaks.value = await momentPeaks(tr, full)
  }
  catch (e) {
    if (run !== loadRun) return
    console.warn('История: момент не загрузился', e)
    error.value = e instanceof ApiError && e.code === 'no_audio' ? t('noAudio') : t('failed')
    phase.value = 'error'
  }
}

/** Форма волны: настоящая с сервера; у превью — из его звука; не вышло — по структуре песни */
async function momentPeaks(tr: NonNullable<typeof track.value>, full: boolean): Promise<number[]> {
  try {
    if (full) {
      const res: SourceResult = await bridge.media.source(tr)
      if (res.kind === 'ready' && res.peaks?.length) return resamplePeaks(res.peaks, BARS)
    }
    else {
      const story = await import('~/lib/story/render')
      const audio = await story.decodeStoryAudio(await bridge.catalog.storyAudio(tr.id, { startMs: 0, lengthMs: PREVIEW_MS }))
      const ch = Array.from({ length: audio.numberOfChannels }, (_, i) => audio.getChannelData(i))
      return channelPeaks(ch, BARS)
    }
  }
  catch (e) {
    console.warn('История: форма волны — по структуре песни', e)
  }
  return await bridge.media.waveform(tr, BARS)
}

// ---------- Прослушать выбранное (тот же плеер, громкость — как у плеера) ----------

const listening = ref(false)
const playheadMs = ref<number | null>(null)
let listenRun = 0
let raf = 0
let startedAt = 0

function stopClock() {
  if (raf) cancelAnimationFrame(raf)
  raf = 0
}

function tick() {
  const elapsed = performance.now() - startedAt
  const len = Math.min(lengthMs.value, info.value?.totalMs ?? lengthMs.value)
  playheadMs.value = Math.min(len, elapsed)
  player.previewGain = Math.min(1, elapsed / FADE_IN_MS, Math.max(0, (len - elapsed) / FADE_OUT_MS))
  if (elapsed >= len) {
    stopListen()
    return
  }
  raf = requestAnimationFrame(tick)
}

async function startListen() {
  const tr = track.value
  const i = info.value
  if (!tr || !i) return
  const run = ++listenRun
  stopClock()
  listening.value = true
  playheadMs.value = 0
  const ok = i.full
    ? await player.startPreview(tr, startMs.value)
    : await player.startPreview(tr, startMs.value, { resolved: { kind: 'ready', source: { url: i.previewUrl!, durationMs: PREVIEW_MS, preview: true }, peaks: null } })
  if (run !== listenRun) return
  if (!ok) {
    stopListen()
    toast.show({ text: t('listenFailed'), icon: 'ph:speaker-simple-slash' })
    return
  }
  player.previewGain = 0
  startedAt = performance.now()
  raf = requestAnimationFrame(tick)
}

function stopListen() {
  listenRun++
  stopClock()
  listening.value = false
  playheadMs.value = null
  if (player.previewing) player.stopPreview(true)
}

function toggleListen() {
  if (listening.value) stopListen()
  else void startListen()
}

// Окно сдвинули, пока звучит, — слушаем уже новое место
let moveTimer: ReturnType<typeof setTimeout> | undefined
function onMoved() {
  if (!listening.value) return
  clearTimeout(moveTimer)
  moveTimer = setTimeout(() => listening.value && void startListen(), 250)
}
watch(lengthMs, onMoved)

// ---------- Сборка ----------

const fileName = computed(() => `${(track.value?.title ?? 'story').replace(/[\\/:*?"<>|]+/g, ' ').trim() || 'story'} — DalyWave.mp4`)
const canShareFiles = computed(() => {
  if (typeof navigator === 'undefined' || typeof navigator.canShare !== 'function') return false
  try {
    return navigator.canShare({ files: [new File([new Blob()], 'story.mp4', { type: 'video/mp4' })] })
  }
  catch {
    return false
  }
})
const percent = computed(() => Math.round(progress.value * 100))
const status = computed(() => {
  if (phase.value === 'prepare') return t('prepare')
  if (phase.value === 'upload') return t('upload')
  return t('render', { n: percent.value })
})

function resetVideo() {
  abort?.abort()
  abort = null
  if (videoUrl.value) URL.revokeObjectURL(videoUrl.value)
  videoUrl.value = null
  preview.value = null
  blob = null
  progress.value = 0
  error.value = ''
}

function reset() {
  stopListen()
  clearTimeout(moveTimer)
  loadRun++
  resetVideo()
  info.value = null
  peaks.value = []
  phase.value = 'pick'
}

async function build() {
  const tr = track.value
  const i = info.value
  if (!tr || !i) return
  stopListen()
  resetVideo()
  const ctl = new AbortController()
  abort = ctl
  phase.value = 'prepare'
  const moment = { startMs: startMs.value, lengthMs: Math.min(lengthMs.value, i.totalMs) as StoryLength }
  try {
    const story = await import('~/lib/story/render')
    // Каждый шаг — с пределом: вместо вечной полосы — «Не получилось собрать видео» и «Попробовать ещё раз»
    const wav = await story.withTimeout(bridge.catalog.storyAudio(tr.id, moment), PREPARE_MS, 'story audio')
    const audio = await story.withTimeout(story.decodeStoryAudio(wav), DECODE_MS, 'decode')
    const cover = tr.album?.image?.large ? bridge.resolveUrl(tr.album.image.large) : null
    const colors = await coverColors(tr.album?.image?.small ? bridge.resolveUrl(tr.album.image.small) : null)
    const input = {
      title: tr.title,
      artist: tr.artists.map(a => a.name).join(', '),
      coverUrl: cover,
      palette: colors ? [colors.vivid, colors.deep] : null,
      audio,
      durationMs: moment.lengthMs,
      // Текст — тот, что звучит в выбранном куске
      lines: storyLines(i.lines, moment.startMs, moment.lengthMs),
      watermark: 'DalyWave',
    }
    if (ctl.signal.aborted) return
    preview.value = (await story.previewFrame(input)).toDataURL('image/jpeg', 0.86)
    phase.value = 'render'
    const res = await story.renderStory(input, p => (progress.value = p), ctl.signal)
    let out = res.blob
    if (res.needsAudio) {
      phase.value = 'upload'
      out = await story.withTimeout(bridge.catalog.storyVideo(tr.id, res.blob, moment), UPLOAD_MS, 'story upload')
    }
    if (ctl.signal.aborted) return
    blob = out
    videoUrl.value = URL.createObjectURL(out)
    phase.value = 'ready'
    bridge.haptic('success')
  }
  catch (e) {
    if (ctl.signal.aborted || (e instanceof DOMException && e.name === 'AbortError')) return
    console.warn('История не собралась', e)
    error.value = e instanceof ApiError && e.code === 'no_audio' ? t('noAudio') : t('failed')
    phase.value = 'error'
  }
}

/** «Другой момент»: обратно к выбору, выбранное остаётся */
function repick() {
  resetVideo()
  phase.value = 'pick'
  if (!info.value) void loadMoment()
}

function retry() {
  if (info.value) void build()
  else void loadMoment()
}

watch(open, (v) => {
  if (v) void loadMoment()
  else reset()
})
onBeforeUnmount(reset)

async function share() {
  if (!blob) return
  const res = await shareOrDownload(blob, fileName.value, track.value?.title ?? 'DalyWave', 'video/mp4')
  if (res === 'downloaded') toast.show({ text: t('saved'), icon: 'ph:download-simple' })
}

function save() {
  if (!blob) return
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = fileName.value
  document.body.appendChild(a)
  a.click()
  a.remove()
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
  toast.show({ text: t('saved'), icon: 'ph:download-simple' })
}
</script>

<template>
  <UiModal v-model:open="open" :title="t('title')" :description="track?.title" :width="400">
    <div class="story" :class="`story--${phase}`">
      <template v-if="phase === 'pick'">
        <div v-if="track" class="story__track">
          <img v-if="track.album?.image?.small" :src="bridge.resolveUrl(track.album.image.small)" alt="" class="story__cover">
          <span v-else class="story__cover" />
          <span class="story__meta">
            <span class="story__name">{{ track.title }}</span>
            <span class="story__artist">{{ track.artists.map(a => a.name).join(', ') }}</span>
          </span>
        </div>
        <MusicStoryMoment
          v-model:start="startMs"
          v-model:length="lengthMs"
          :peaks="peaks"
          :total-ms="info?.totalMs ?? 0"
          :chorus-ms="info?.chorusMs ?? null"
          :playing="listening"
          :playhead-ms="playheadMs"
          :lyric="lyric"
          :disabled="!info"
          @toggle="toggleListen"
          @moved="onMoved"
        />
        <div class="story__actions">
          <UiButton variant="primary" size="lg" icon="ph:film-strip" block :disabled="!info" @click="build">{{ t('make') }}</UiButton>
        </div>
      </template>

      <template v-else>
        <div v-if="phase !== 'error' || preview" class="story__frame">
          <video
            v-if="videoUrl"
            :src="videoUrl"
            class="story__media"
            autoplay
            loop
            muted
            playsinline
            :aria-label="t('title')"
          />
          <img v-else-if="preview" :src="preview" alt="" class="story__media">
          <div v-else class="story__media story__blank" />
          <div v-if="phase !== 'ready' && phase !== 'error'" class="story__veil">
            <span class="story__bar" role="progressbar" :aria-valuenow="percent" aria-valuemin="0" aria-valuemax="100"><span :style="{ width: `${phase === 'render' ? percent : phase === 'upload' ? 100 : 0}%` }" /></span>
            <span class="story__status">{{ status }}</span>
          </div>
        </div>

        <p v-if="phase === 'error'" class="story__error" role="alert">{{ error }}</p>

        <div class="story__actions">
          <template v-if="phase === 'ready'">
            <UiButton v-if="canShareFiles" variant="primary" size="lg" icon="ph:export" block @click="share">{{ t('share') }}</UiButton>
            <UiButton v-if="!canShareFiles || !ui.isPhone" size="lg" icon="ph:download-simple" :variant="canShareFiles ? 'secondary' : 'primary'" block @click="save">{{ t('save') }}</UiButton>
          </template>
          <UiButton v-else-if="phase === 'error' && !noAudio" size="lg" icon="ph:arrow-clockwise" block @click="retry">{{ t('retry') }}</UiButton>
          <UiButton v-if="(phase === 'ready' || phase === 'error') && info" variant="ghost" size="lg" block @click="repick">{{ t('repick') }}</UiButton>
        </div>
      </template>
    </div>
  </UiModal>
</template>

<style scoped>
.story { display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 0 20px 20px; }
.story--pick { gap: 20px; padding-top: 4px; }
.story__track { display: flex; align-items: center; gap: 12px; width: 100%; min-width: 0; }
.story__cover { flex: none; width: 44px; height: 44px; border-radius: var(--r-1, 6px); background: var(--fill-2); object-fit: cover; }
.story__meta { display: flex; flex-direction: column; min-width: 0; }
.story__name, .story__artist { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.story__name { font-size: var(--fs-body); font-weight: 600; color: var(--text-1); }
.story__artist { font-size: var(--fs-small); color: var(--text-2); }
.story__frame { position: relative; width: min(100%, 260px, 44dvh * 9 / 16); aspect-ratio: 9 / 16; overflow: hidden; border-radius: var(--r-3, 18px); background: #07050f; box-shadow: inset 0 0 0 1px var(--hair); }
.story__media { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.story__blank { background: #07050f; }
.story__veil { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; background: rgba(7, 5, 15, 0.55); color: #fff; }
.story__status { font-size: var(--fs-small); font-variant-numeric: tabular-nums; color: rgba(255, 255, 255, 0.82); }
.story__error { margin: 0; font-size: var(--fs-small); color: var(--text-2); text-align: center; }
.story__actions { display: flex; flex-direction: column; gap: 8px; width: 100%; }
.story__bar { position: relative; width: 120px; height: 3px; overflow: hidden; border-radius: 2px; background: rgba(255, 255, 255, 0.22); }
.story__bar > span { position: absolute; inset: 0 auto 0 0; border-radius: inherit; background: #fff; transition: width var(--dur-2) linear; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "История",
    "make": "Собрать видео",
    "repick": "Другой момент",
    "listenFailed": "Не получилось включить отрывок",
    "prepare": "Готовим звук",
    "render": "Собираем видео · {n} %",
    "upload": "Добавляем звук",
    "share": "Поделиться",
    "save": "Сохранить видео",
    "saved": "Видео сохранено",
    "retry": "Попробовать ещё раз",
    "noAudio": "У этого трека пока нет звука для истории",
    "failed": "Не получилось собрать видео"
  },
  "en": {
    "title": "Story",
    "make": "Make the video",
    "repick": "Another moment",
    "listenFailed": "Couldn't play the clip",
    "prepare": "Preparing audio",
    "render": "Making the video · {n}%",
    "upload": "Adding audio",
    "share": "Share",
    "save": "Save video",
    "saved": "Video saved",
    "retry": "Try again",
    "noAudio": "This track has no audio for a story yet",
    "failed": "Couldn't make the video"
  }
}
</i18n>
