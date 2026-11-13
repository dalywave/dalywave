<script setup lang="ts">
import type { StoryLine } from '~/lib/story/render'
import { ApiError } from '~/api/types'

// «Поделиться историей»: вертикальное видео 15 с для Instagram Stories и TikTok — обложка, название,
// строка текста и живая волна в цвет обложки под звук припева. Собирается на устройстве
// (lib/story/render.ts); готовое — в системное «Поделиться» или файлом
type Phase = 'prepare' | 'render' | 'upload' | 'ready' | 'error'

const { track, open } = useStory()
const bridge = useBridge()
const ui = useUiStore()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })

const phase = ref<Phase>('prepare')
const progress = ref(0)
const preview = ref<string | null>(null)
const videoUrl = ref<string | null>(null)
const error = ref('')
let blob: Blob | null = null
let abort: AbortController | null = null

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

function reset() {
  abort?.abort()
  abort = null
  if (videoUrl.value) URL.revokeObjectURL(videoUrl.value)
  videoUrl.value = null
  preview.value = null
  blob = null
  progress.value = 0
  error.value = ''
}

/** Строки текста относительно начала истории: та, что звучит на старте, — с нуля */
function storyLines(lines: { tMs: number, text: string }[], startMs: number): StoryLine[] {
  const out: StoryLine[] = []
  for (const l of lines) {
    const at = l.tMs - startMs
    if (at < 0) {
      out.length = 0
      out.push({ atMs: 0, text: l.text })
    }
    else if (at < 15_000) out.push({ atMs: at, text: l.text })
  }
  return out
}

async function build() {
  const tr = track.value
  if (!tr) return
  reset()
  const ctl = new AbortController()
  abort = ctl
  phase.value = 'prepare'
  try {
    const story = await import('~/lib/story/render')
    const [wav, hook, lyrics] = await Promise.all([
      bridge.catalog.storyAudio(tr.id),
      bridge.catalog.hook(tr.id).catch(() => null),
      bridge.catalog.lyrics(tr.id).catch(() => null),
    ])
    const audio = await story.decodeStoryAudio(wav)
    const cover = tr.album?.image?.large ? bridge.resolveUrl(tr.album.image.large) : null
    const colors = await coverColors(tr.album?.image?.small ? bridge.resolveUrl(tr.album.image.small) : null)
    const input = {
      title: tr.title,
      artist: tr.artists.map(a => a.name).join(', '),
      coverUrl: cover,
      palette: colors ? [colors.vivid, colors.deep] : null,
      audio,
      // Время строк известно, только когда отрывок — из звука на сервере (у превью Deezer начало неизвестно)
      lines: hook?.ready && lyrics?.kind === 'synced' ? storyLines(lyrics.lines, hook.startMs) : [],
      watermark: 'DalyWave',
    }
    if (ctl.signal.aborted) return
    preview.value = (await story.previewFrame(input)).toDataURL('image/jpeg', 0.86)
    phase.value = 'render'
    const res = await story.renderStory(input, p => (progress.value = p), ctl.signal)
    let out = res.blob
    if (res.needsAudio) {
      phase.value = 'upload'
      out = await bridge.catalog.storyVideo(tr.id, res.blob)
    }
    if (ctl.signal.aborted) return
    blob = out
    videoUrl.value = URL.createObjectURL(out)
    phase.value = 'ready'
    bridge.haptic('success')
  }
  catch (e) {
    if (ctl.signal.aborted || (e instanceof DOMException && e.name === 'AbortError')) return
    error.value = e instanceof ApiError && e.code === 'no_audio' ? t('noAudio') : t('failed')
    phase.value = 'error'
  }
}

watch(open, (v) => {
  if (v) void build()
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
    <div class="story">
      <div class="story__frame">
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
        <UiButton v-else-if="phase === 'error'" size="lg" icon="ph:arrow-clockwise" block @click="build">{{ t('retry') }}</UiButton>
      </div>
    </div>
  </UiModal>
</template>

<style scoped>
.story { display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 0 20px 20px; }
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
