<script setup lang="ts">
import type { ImportSummary } from '~/api/types'

// Ход переноса одной карточкой: загрузка файла → поиск в каталоге → раскладка по медиатеке → итог.
// Пока идёт — шаги и полоса; готово — фраза «Добавлено 1 240 лайков, 38 плейлистов…» и цифры.
// Действия (остановить, убрать, ещё раз) — в слоте #actions
const props = defineProps<{
  /** Импорт с сервера; нет — идёт загрузка файла */
  summary: ImportSummary | null
  /** Доля загруженного файла 0..1, пока сервер его не принял */
  upload?: number | null
  fileName?: string
}>()

const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const n = (v: number) => formatNumber(v, lang.value)

type Step = 'upload' | 'match' | 'apply' | 'done'
const step = computed<Step>(() => {
  const s = props.summary
  if (!s) return 'upload'
  if (s.status !== 'running') return 'done'
  if (s.phase === 'uploading') return 'upload'
  if (s.phase === 'matching') return 'match'
  if (s.phase === 'applying') return 'apply'
  return 'done'
})
const STEPS: Step[] = ['upload', 'match', 'apply', 'done']
const stepIndex = computed(() => STEPS.indexOf(step.value))

const share = computed(() => {
  const s = props.summary
  if (!s) return props.upload ?? 0
  if (s.status !== 'running') return 1
  if (s.phase === 'applying') return 1
  return s.total ? s.processed / s.total : 0
})
const failed = computed(() => props.summary?.status === 'failed')
const cancelled = computed(() => props.summary?.status === 'cancelled')
const isFiles = computed(() => props.summary?.kind === 'files')
// Один плейлист по ссылке итожится как список: «В плейлист „…“ добавлено N треков»
const isList = computed(() => !!props.summary && ['text', 'csv', 'm3u', 'youtube_playlist'].includes(props.summary.kind))

/** Слово после числа в нужной форме: [одна, две, пять] — для русского и украинского, [one, many] — для английского */
function count(v: number, ru: [string, string, string], uk: [string, string, string], en: [string, string]): string {
  return lang.value === 'en' ? `${n(v)}\u00a0${v === 1 ? en[0] : en[1]}` : `${n(v)}\u00a0${plural(v, lang.value === 'uk' ? uk : ru)}`
}

function joinRu(parts: string[]): string {
  if (parts.length <= 1) return parts.join('')
  const and = lang.value === 'en' ? ' and ' : lang.value === 'uk' ? ' і ' : ' и '
  return `${parts.slice(0, -1).join(', ')}${and}${parts.at(-1)}`
}

/** «Добавлено 1 240 лайков, 38 плейлистов и 52 000 прослушиваний» */
const sentence = computed(() => {
  const s = props.summary
  if (!s) return ''
  const st = s.stats
  if (isFiles.value) {
    if (!st.files) return t('result.noFiles')
    const accepted = count(st.files, ['файл', 'файла', 'файлов'], ['файл', 'файли', 'файлів'], ['file', 'files'])
    return st.upgraded
      ? t('result.files', { files: accepted, upgraded: n(st.upgraded) })
      : t('result.filesOnly', { files: accepted })
  }
  if (isList.value) {
    const tracks = st.playlistTracks || st.likes
    if (!tracks) return t('result.nothing')
    const what = count(tracks, ['трек', 'трека', 'треков'], ['трек', 'треки', 'треків'], ['track', 'tracks'])
    return st.likes ? t('result.listLikes', { tracks: what }) : t('result.listPlaylist', { tracks: what, title: s.title })
  }
  const parts: string[] = []
  if (st.likes) parts.push(count(st.likes, ['лайк', 'лайка', 'лайков'], ['вподобання', 'вподобання', 'вподобань'], ['like', 'likes']))
  if (st.playlists) parts.push(count(st.playlists, ['плейлист', 'плейлиста', 'плейлистов'], ['плейлист', 'плейлисти', 'плейлистів'], ['playlist', 'playlists']))
  // Плейлисты уже были с прошлого переноса — дополнились
  else if (st.playlistTracks) parts.push(t('result.intoPlaylists', { tracks: count(st.playlistTracks, ['трек', 'трека', 'треков'], ['трек', 'треки', 'треків'], ['track', 'tracks']) }))
  if (st.albums) parts.push(count(st.albums, ['альбом', 'альбома', 'альбомов'], ['альбом', 'альбоми', 'альбомів'], ['album', 'albums']))
  if (st.artists) parts.push(count(st.artists, ['подписка', 'подписки', 'подписок'], ['підписка', 'підписки', 'підписок'], ['follow', 'follows']))
  if (st.plays) parts.push(count(st.plays, ['прослушивание', 'прослушивания', 'прослушиваний'], ['прослуховування', 'прослуховування', 'прослуховувань'], ['play', 'plays']))
  return parts.length ? t('result.added', { what: joinRu(parts) }) : t('result.nothingNew')
})

const title = computed(() => {
  if (failed.value) return t('title.failed')
  if (cancelled.value) return t('title.cancelled')
  return t(`title.${step.value}`)
})
const detail = computed(() => {
  const s = props.summary
  if (failed.value) return s?.error || t('detail.failed')
  if (cancelled.value) return t('detail.cancelled')
  switch (step.value) {
    case 'upload':
      if (isFiles.value && s) return t('detail.files', { n: n(s.total) })
      return props.upload != null && props.upload < 1 ? t('detail.upload', { pct: Math.round((props.upload ?? 0) * 100) }) : t('detail.reading')
    case 'match':
      return s?.total ? t('detail.match', { done: n(s.processed), total: n(s.total) }) : t('detail.matchStart')
    case 'apply':
      return t('detail.apply')
    default:
      return sentence.value
  }
})
const icon = computed(() => {
  if (failed.value) return 'ph:warning-circle'
  if (cancelled.value) return 'ph:pause'
  return { upload: 'ph:upload-simple', match: 'ph:magnifying-glass', apply: 'ph:stack', done: 'ph:check' }[step.value]
})
const running = computed(() => !failed.value && !cancelled.value && step.value !== 'done')

/** Цифры итога плиткой — только ненулевые */
const tiles = computed(() => {
  const s = props.summary
  if (!s || s.status !== 'done') return []
  const st = s.stats
  const list: { key: string, value: number, label: string }[] = isFiles.value
    ? [
        { key: 'files', value: st.files, label: t('tile.files') },
        { key: 'upgraded', value: st.upgraded, label: t('tile.upgraded') },
      ]
    : [
        { key: 'likes', value: st.likes, label: t('tile.likes') },
        { key: 'playlists', value: st.playlists, label: t('tile.playlists') },
        { key: 'albums', value: st.albums, label: t('tile.albums') },
        { key: 'artists', value: st.artists, label: t('tile.artists') },
        { key: 'plays', value: st.plays, label: t('tile.plays') },
      ]
  if (isList.value) return [{ key: 'tracks', value: st.playlistTracks || st.likes, label: t('tile.tracks') }].filter(x => x.value)
  return list.filter(x => x.value)
})
</script>

<template>
  <section class="is" :class="{ 'is--done': step === 'done' && !failed && !cancelled, 'is--bad': failed }" aria-live="polite">
    <div class="is__head">
      <span class="is__icon" :class="{ 'is-running': running }" aria-hidden="true"><Icon :name="icon" /></span>
      <div class="is__text">
        <h2 class="is__title">{{ title }}</h2>
        <p class="is__detail tnum">{{ detail }}</p>
      </div>
    </div>

    <template v-if="running">
      <div class="is__bar" role="progressbar" :aria-valuenow="Math.round(share * 100)" aria-valuemin="0" aria-valuemax="100" :aria-label="title">
        <i :class="{ 'is-indeterminate': step === 'apply' || (step === 'match' && !summary?.total) }" :style="{ width: `${Math.max(3, share * 100)}%` }" />
      </div>
      <ol class="is__steps" :aria-label="t('stepsLabel')">
        <li v-for="(s, i) in STEPS.slice(0, 3)" :key="s" :class="{ 'is-past': i < stepIndex, 'is-now': i === stepIndex }">
          <Icon :name="i < stepIndex ? 'ph:check-bold' : 'ph:circle-fill'" class="is__dot" />
          <span>{{ t(`step.${s}`) }}</span>
        </li>
      </ol>
      <p class="is__calm">{{ fileName && step === 'upload' && !summary ? t('calmUpload') : t('calm') }}</p>
    </template>

    <div v-if="tiles.length" class="is__tiles">
      <div v-for="x in tiles" :key="x.key" class="is__tile">
        <b class="tnum">{{ formatNumber(x.value, lang) }}</b>
        <span>{{ x.label }}</span>
      </div>
    </div>

    <div v-if="$slots.actions" class="is__actions"><slot name="actions" /></div>
  </section>
</template>

<style scoped>
.is {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 24px;
  border-radius: var(--r-5);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
}
.is__head { display: flex; align-items: flex-start; gap: 16px; }
.is__icon {
  display: grid;
  flex: none;
  place-items: center;
  width: 48px;
  height: 48px;
  border-radius: 16px;
  background: var(--fill-2);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font-size: 22px;
}
.is__icon.is-running { color: var(--accent); }
.is--done .is__icon { background: color-mix(in oklab, var(--success) 20%, transparent); color: var(--success); box-shadow: none; }
.is--bad .is__icon { background: color-mix(in oklab, var(--danger) 18%, transparent); color: var(--danger); box-shadow: none; }
.is__text { display: flex; flex-direction: column; gap: 4px; min-width: 0; padding-top: 2px; }
.is__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.is__detail { margin: 0; font-size: var(--fs-body); color: var(--text-2); text-wrap: pretty; overflow-wrap: anywhere; }
.is--done .is__detail { color: var(--text-1); }

.is__bar { height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-2); }
.is__bar i { display: block; height: 100%; border-radius: inherit; background: var(--accent); transition: width var(--dur-3) var(--ease-out); }
.is__bar i.is-indeterminate { width: 40% !important; animation: slide 1.4s var(--ease-in-out) infinite; }
@keyframes slide { from { transform: translateX(-100%); } to { transform: translateX(250%); } }

.is__steps { display: flex; flex-wrap: wrap; gap: 8px 20px; margin: 0; padding: 0; list-style: none; font-size: var(--fs-small); color: var(--text-4); }
.is__steps li { display: inline-flex; align-items: center; gap: 8px; }
.is__steps li.is-past { color: var(--text-2); }
.is__steps li.is-now { color: var(--text-1); font-weight: 600; }
.is__dot { font-size: 8px; }
.is__steps li.is-past .is__dot { font-size: 12px; color: var(--success); }
.is__steps li.is-now .is__dot { color: var(--accent); }
.is__calm { margin: -4px 0 0; font-size: var(--fs-caption); color: var(--text-3); }

/* Цифры итога — просто цифры в ряд, без плиток внутри карточки */
.is__tiles { display: flex; flex-wrap: wrap; gap: 12px 32px; }
.is__tile { display: flex; flex-direction: column; gap: 2px; }
.is__tile b { font-size: var(--fs-h3); font-weight: 700; letter-spacing: -0.02em; }
.is__tile span { font-size: var(--fs-caption); color: var(--text-3); }
.is__actions { display: flex; flex-wrap: wrap; gap: 8px; }

@media (max-width: 699px) {
  .is { padding: 20px 16px; border-radius: var(--r-4); }
  .is__icon { width: 44px; height: 44px; border-radius: 14px; font-size: 20px; }
  .is__tiles { gap: 12px 24px; }
  .is__tile b { font-size: var(--fs-h4); }
  .is__actions > :first-child { flex: 1 1 100%; }
}
@media (prefers-reduced-motion: reduce) {
  .is__bar i.is-indeterminate { animation: none; width: 100% !important; opacity: 0.5; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "stepsLabel": "Шаги переноса",
    "step": { "upload": "Загрузка", "match": "Поиск в каталоге", "apply": "Медиатека" },
    "title": {
      "upload": "Загружаем",
      "match": "Ищем в каталоге",
      "apply": "Раскладываем по медиатеке",
      "done": "Готово",
      "failed": "Не получилось",
      "cancelled": "Перенос остановлен"
    },
    "detail": {
      "upload": "Отправлено {pct}\u00a0%",
      "reading": "Читаем архив…",
      "files": "Файлов: {n}",
      "matchStart": "Начинаем…",
      "match": "{done} из {total}",
      "apply": "Лайки, плейлисты и история ложатся на свои места",
      "failed": "Попробуйте ещё раз чуть позже",
      "cancelled": "Найденное до остановки осталось в медиатеке"
    },
    "calm": "Можно закрыть страницу — перенос продолжится",
    "calmUpload": "Не закрывайте страницу, пока файл загружается",
    "result": {
      "added": "Добавлено: {what}",
      "nothingNew": "Всё это уже было в медиатеке",
      "nothing": "Ни один трек не нашёлся — проверьте список ниже",
      "listPlaylist": "В плейлист «{title}» добавлено {tracks}",
      "listLikes": "В «Мне нравится» добавлено {tracks}",
      "intoPlaylists": "{tracks} в плейлисты",
      "files": "Принято {files}, {upgraded} заменили копии хуже качеством",
      "filesOnly": "Принято {files}",
      "noFiles": "Ни один файл не подошёл — проверьте список ниже"
    },
    "tile": {
      "likes": "лайки",
      "playlists": "плейлисты",
      "albums": "альбомы",
      "artists": "подписки",
      "plays": "прослушивания",
      "tracks": "треки",
      "files": "файлы",
      "upgraded": "лучше прежних"
    }
  },
  "uk": {
    "stepsLabel": "Кроки перенесення",
    "step": { "upload": "Завантаження", "match": "Пошук у каталозі", "apply": "Медіатека" },
    "title": {
      "upload": "Завантажуємо",
      "match": "Шукаємо в каталозі",
      "apply": "Розкладаємо по медіатеці",
      "done": "Готово",
      "failed": "Не вдалося",
      "cancelled": "Перенесення зупинено"
    },
    "detail": {
      "upload": "Надіслано {pct}\u00a0%",
      "reading": "Читаємо архів…",
      "files": "Файлів: {n}",
      "matchStart": "Починаємо…",
      "match": "{done} з {total}",
      "apply": "Вподобання, плейлисти й історія стають на свої місця",
      "failed": "Спробуйте ще раз трохи згодом",
      "cancelled": "Знайдене до зупинки залишилося в медіатеці"
    },
    "calm": "Можна закрити сторінку — перенесення продовжиться",
    "calmUpload": "Не закривайте сторінку, поки файл завантажується",
    "result": {
      "added": "Додано: {what}",
      "nothingNew": "Усе це вже було в медіатеці",
      "nothing": "Жоден трек не знайшовся — перевірте список нижче",
      "listPlaylist": "До плейлиста «{title}» додано {tracks}",
      "listLikes": "До «Вподобаного» додано {tracks}",
      "intoPlaylists": "{tracks} до плейлистів",
      "files": "Прийнято {files}, {upgraded} замінили копії гіршої якості",
      "filesOnly": "Прийнято {files}",
      "noFiles": "Жоден файл не підійшов — перевірте список нижче"
    },
    "tile": {
      "likes": "вподобання",
      "playlists": "плейлисти",
      "albums": "альбоми",
      "artists": "підписки",
      "plays": "прослуховування",
      "tracks": "треки",
      "files": "файли",
      "upgraded": "кращі за попередні"
    }
  },
  "en": {
    "stepsLabel": "Transfer steps",
    "step": { "upload": "Upload", "match": "Catalog search", "apply": "Library" },
    "title": {
      "upload": "Uploading",
      "match": "Searching the catalog",
      "apply": "Filling your library",
      "done": "Done",
      "failed": "Something went wrong",
      "cancelled": "Transfer stopped"
    },
    "detail": {
      "upload": "{pct}% sent",
      "reading": "Reading the archive…",
      "files": "Files: {n}",
      "matchStart": "Starting…",
      "match": "{done} of {total}",
      "apply": "Likes, playlists and history are falling into place",
      "failed": "Try again a bit later",
      "cancelled": "Whatever was found before stopping stays in your library"
    },
    "calm": "You can close this page — the transfer keeps going",
    "calmUpload": "Keep this page open while the file uploads",
    "result": {
      "added": "Added {what}",
      "nothingNew": "All of it was already in your library",
      "nothing": "No tracks were found — check the list below",
      "listPlaylist": "Added {tracks} to “{title}”",
      "listLikes": "Added {tracks} to Liked",
      "intoPlaylists": "{tracks} to playlists",
      "files": "Accepted {files}, {upgraded} replaced lower-quality copies",
      "filesOnly": "Accepted {files}",
      "noFiles": "No files matched — check the list below"
    },
    "tile": {
      "likes": "likes",
      "playlists": "playlists",
      "albums": "albums",
      "artists": "follows",
      "plays": "plays",
      "tracks": "tracks",
      "files": "files",
      "upgraded": "better than before"
    }
  }
}
</i18n>
