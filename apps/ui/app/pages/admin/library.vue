<script setup lang="ts">
import type { LibraryProblem, LibraryStatus } from '~/api/types'

// Библиотека: своя музыка из папок на сервере — какие папки, как идёт сканирование, какие файлы не
// попали в медиатеку и почему. «Пересканировать» — полный проход; изменения сервер ловит и сам
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const lang = useLang()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const { data, state, load } = useLoad<LibraryStatus>(() => bridge.admin.library())
const scan = computed(() => data.value?.scan ?? null)
const running = computed(() => scan.value?.status === 'running')
const configured = computed(() => (data.value?.dirs.length ?? 0) > 0)
const pct = computed(() => {
  const s = scan.value
  return s && s.filesTotal ? Math.min(100, Math.round((s.filesDone / s.filesTotal) * 100)) : 0
})
const unreadable = computed(() => (data.value?.errors ?? 0) + (data.value?.unsupported ?? 0))

// Идущий проход — часто, иначе изредка: изменения в папках сервер подхватывает сам
let quiet = 0
useIntervalFn(() => {
  quiet += 1
  if (running.value || quiet >= 8) {
    quiet = 0
    void refresh()
  }
}, 2000)

async function refresh() {
  try {
    data.value = await bridge.admin.library()
  }
  catch {
    // Следующая попытка — по таймеру
  }
}

const starting = ref(false)
async function rescan() {
  starting.value = true
  try {
    await bridge.admin.rescanLibrary()
    toast.show({ text: t('queued'), icon: 'ph:arrows-clockwise' })
    await refresh()
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    starting.value = false
  }
}

/** «+39 · изменено 2 · переехало 1 · удалено 3» — только ненулевое */
const summary = computed(() => {
  const s = scan.value
  if (!s) return ''
  const parts: string[] = []
  if (s.added) parts.push(t('sum.added', { n: formatNumber(s.added, lang.value) }))
  if (s.updated) parts.push(t('sum.updated', { n: formatNumber(s.updated, lang.value) }))
  if (s.moved) parts.push(t('sum.moved', { n: formatNumber(s.moved, lang.value) }))
  if (s.removed) parts.push(t('sum.removed', { n: formatNumber(s.removed, lang.value) }))
  if (s.failed) parts.push(t('sum.failed', { n: formatNumber(s.failed, lang.value) }))
  const line = parts.join(' · ')
  return line ? line.charAt(0).toUpperCase() + line.slice(1) : t('sum.nothing')
})

const fileName = (p: string) => p.split('/').pop() ?? p
/** Папка файла — от папки музыки, а не от корня диска */
function folderOf(p: string) {
  const dir = p.split('/').slice(0, -1).join('/')
  const root = data.value?.dirs.find(d => dir === d.path || dir.startsWith(`${d.path}/`))
  if (!root) return dir
  const rest = dir.slice(root.path.length).replace(/^\//, '')
  return rest || t('problems.root')
}
const reason = (p: LibraryProblem) => p.message ?? t(p.problem === 'unsupported' ? 'problem.unsupported' : 'problem.error')
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <UiButton
        v-if="configured"
        variant="secondary"
        size="sm"
        icon="ph:arrows-clockwise"
        :loading="starting"
        :disabled="running"
        @click="rescan"
      >
        {{ t('rescan') }}
      </UiButton>
    </header>

    <AdminState
      :loading="state === 'loading'"
      :error="state === 'error'"
      :empty="!!data && !configured"
      empty-icon="ph:folder-open"
      :empty-title="t('none.title')"
      :empty-text="t('none.text')"
      :rows="5"
      @retry="load"
    >
      <template v-if="data">
        <div class="ap-stats" style="--n: 4">
          <div class="ap-stat"><span class="ap-stat__value">{{ formatNumber(data.tracks, lang) }}</span><span class="ap-stat__label">{{ t('stats.tracks') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ formatNumber(data.analyzing, lang) }}</span><span class="ap-stat__label">{{ t('stats.analyzing') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ formatNumber(data.transcoded, lang) }}</span><span class="ap-stat__label">{{ t('stats.transcoded') }}</span></div>
          <div class="ap-stat" :class="{ 'is-warn': unreadable }"><span class="ap-stat__value">{{ formatNumber(unreadable, lang) }}</span><span class="ap-stat__label">{{ t('stats.unreadable') }}</span></div>
        </div>

        <AdminSection :title="t('scan.title')">
          <div v-if="scan" class="lb-scan">
            <div class="lb-scan__line">
              <AdminStatus :tone="running ? 'on' : scan.status === 'failed' ? 'bad' : 'ok'" :pulse="running">
                {{ running ? t('scan.running') : scan.status === 'failed' ? t('scan.failed') : t('scan.done') }}
              </AdminStatus>
              <span class="ap-muted">{{ running ? formatRelative(scan.startedAt, lang) : formatRelative(scan.finishedAt ?? scan.startedAt, lang) }}</span>
              <span class="ap-muted">· {{ t(`kind.${scan.kind}`) }}</span>
            </div>
            <template v-if="running">
              <span class="lb-bar" role="progressbar" :aria-valuenow="pct" aria-valuemin="0" aria-valuemax="100"><i :style="{ width: `${pct}%` }" /></span>
              <p class="ap-hint">{{ t('scan.progress', { done: formatNumber(scan.filesDone, lang), total: formatNumber(scan.filesTotal, lang) }) }}</p>
            </template>
            <p v-else class="lb-scan__sum">{{ summary }}</p>
            <p v-if="scan.message" class="ap-hint ap-warn">{{ scan.message }}</p>
          </div>
          <p v-else class="ap-hint">{{ t('scan.never') }}</p>
        </AdminSection>

        <AdminSection :title="t('dirs.title')" :hint="t('dirs.hint')">
          <div class="ap-table lb-dirs">
            <div v-for="d in data.dirs" :key="d.path" class="ap-tr">
              <span class="ap-cell"><b class="lb-path">{{ d.path }}</b><small :class="{ 'ap-bad': !d.available }">{{ d.available ? t('dirs.ok') : t('dirs.missing') }}</small></span>
              <span class="ap-num ap-muted">{{ formatCount(d.files, 'file', lang) }}</span>
              <span class="ap-num">{{ formatBytes(d.sizeBytes, lang) }}</span>
            </div>
          </div>
        </AdminSection>

        <AdminSection :title="t('problems.title')" :hint="unreadable ? t('problems.hint') : undefined">
          <div v-if="data.problems.length" class="ap-table lb-probs">
            <div v-for="p in data.problems" :key="p.path" class="ap-tr">
              <span class="ap-cell"><b>{{ fileName(p.path) }}</b><small>{{ folderOf(p.path) }}</small></span>
              <span class="ap-cell"><span :class="p.problem === 'error' ? 'ap-bad' : 'ap-warn'">{{ t(`problem.${p.problem}Short`) }}</span><small>{{ reason(p) }}</small></span>
              <span class="ap-num ap-muted ap-hide-phone">{{ formatRelative(p.updatedAt, lang) }}</span>
            </div>
          </div>
          <p v-else class="ap-hint">{{ t('problems.none') }}</p>
        </AdminSection>
      </template>
    </AdminState>
  </div>
</template>

<style scoped>
.lb-scan { display: grid; gap: 10px; }
.lb-scan__line { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.lb-scan__sum { margin: 0; color: var(--text-2); font-variant-numeric: tabular-nums; }
.lb-bar { display: block; width: 100%; max-width: 420px; height: 4px; overflow: hidden; border-radius: 2px; background: var(--fill-2); }
.lb-bar i { display: block; height: 100%; border-radius: inherit; background: var(--accent); transition: width 0.4s ease; }
.lb-dirs { --cols: minmax(0, 1fr) 96px 88px; }
.lb-dirs .ap-tr, .lb-probs .ap-tr { min-height: 52px; }
.lb-path { overflow: hidden; font-family: var(--font-mono, ui-monospace, monospace); font-size: 13px; font-weight: 500; text-overflow: ellipsis; white-space: nowrap; }
.lb-probs { --cols: minmax(0, 1.4fr) minmax(0, 1fr) 120px; }
@media (max-width: 1099px) {
  .ap-stats { --n: 2 !important; row-gap: 16px; }
  .ap-stat:nth-child(3) { padding-left: 0; border-left: 0; }
}
@media (max-width: 699px) {
  .lb-dirs { --cols: minmax(0, 1fr) 72px; }
  .lb-dirs .ap-tr > :nth-child(2) { display: none; }
  .lb-probs { --cols: minmax(0, 1fr); row-gap: 4px; }
  .lb-probs .ap-tr { padding-block: 10px; }
  .lb-probs small { white-space: normal; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Библиотека",
    "rescan": "Пересканировать",
    "queued": "Папки пересканируются",
    "failed": "Не получилось начать сканирование",
    "none": {
      "title": "Своя музыка не подключена",
      "text": "Укажите папки с музыкой в настройках сервера (DALYWAVE_MUSIC_DIRS) и перезапустите его"
    },
    "stats": {
      "tracks": "треков из папок",
      "analyzing": "ждут громкости и анализа",
      "transcoded": "играются через копию",
      "unreadable": "не прочитались"
    },
    "scan": {
      "title": "Сканирование",
      "running": "Идёт",
      "done": "Готово",
      "failed": "Прервалось",
      "never": "Папки ещё не сканировались",
      "progress": "Разобрано {done} из {total} файлов"
    },
    "kind": { "full": "все папки", "changes": "изменения" },
    "sum": {
      "added": "+{n} новых",
      "updated": "изменено {n}",
      "moved": "переехало {n}",
      "removed": "удалено {n}",
      "failed": "не прочитано {n}",
      "nothing": "Ничего не изменилось"
    },
    "dirs": {
      "title": "Папки",
      "hint": "Сервер только читает их: файлы не копируются и не меняются",
      "ok": "Читается, изменения подхватываются сами",
      "missing": "Папка недоступна — её файлы остаются в медиатеке, пока она не вернётся"
    },
    "problems": {
      "title": "Не попали в медиатеку",
      "hint": "Исправленный файл подхватится при следующем сканировании",
      "none": "Все файлы прочитались",
      "root": "В корне папки"
    },
    "problem": {
      "error": "Файл не прочитался",
      "unsupported": "Формат не поддерживается",
      "errorShort": "Ошибка чтения",
      "unsupportedShort": "Не тот формат"
    }
  },
  "en": {
    "title": "Library",
    "rescan": "Rescan",
    "queued": "Rescanning the folders",
    "failed": "Could not start the scan",
    "none": {
      "title": "No music folders yet",
      "text": "Set the music folders in the server settings (DALYWAVE_MUSIC_DIRS) and restart it"
    },
    "stats": {
      "tracks": "tracks from folders",
      "analyzing": "awaiting loudness and analysis",
      "transcoded": "played via a copy",
      "unreadable": "could not be read"
    },
    "scan": {
      "title": "Scanning",
      "running": "Running",
      "done": "Done",
      "failed": "Interrupted",
      "never": "The folders have not been scanned yet",
      "progress": "{done} of {total} files processed"
    },
    "kind": { "full": "all folders", "changes": "changes" },
    "sum": {
      "added": "+{n} new",
      "updated": "{n} changed",
      "moved": "{n} moved",
      "removed": "{n} removed",
      "failed": "{n} unreadable",
      "nothing": "Nothing changed"
    },
    "dirs": {
      "title": "Folders",
      "hint": "The server only reads them: files are never copied or changed",
      "ok": "Readable, changes are picked up automatically",
      "missing": "Folder is unavailable — its files stay in the library until it is back"
    },
    "problems": {
      "title": "Not in the library",
      "hint": "A fixed file is picked up on the next scan",
      "none": "Every file was read",
      "root": "In the folder root"
    },
    "problem": {
      "error": "The file could not be read",
      "unsupported": "Unsupported format",
      "errorShort": "Read error",
      "unsupportedShort": "Unsupported"
    }
  }
}
</i18n>
