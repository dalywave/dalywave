<script setup lang="ts">
import type { ImageSet, ImportItem, ImportItemFix } from '~/api/types'

// Что не нашлось: строка из источника, подсказка «Похоже: …» одним нажатием и поиск по каталогу,
// чтобы выбрать самому. Выбранное сразу ложится в медиатеку; «Пропустить» убирает строку
const props = defineProps<{ importId: string }>()
const emit = defineEmits<{ changed: [] }>()

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const items = ref<ImportItem[]>([])
const cursor = ref<string | null>(null)
const state = ref<'loading' | 'ready' | 'error'>('loading')
const loadingMore = ref(false)
const busy = ref<Record<string, boolean>>({})

async function load() {
  state.value = 'loading'
  try {
    const page = await bridge.imports.items(props.importId, 'unmatched', null, 30)
    items.value = page.items
    cursor.value = page.nextCursor
    state.value = 'ready'
  }
  catch {
    state.value = 'error'
  }
}
async function more() {
  if (!cursor.value) return
  loadingMore.value = true
  try {
    const page = await bridge.imports.items(props.importId, 'unmatched', cursor.value, 30)
    items.value = [...items.value, ...page.items]
    cursor.value = page.nextCursor
  }
  catch {
    toast.error(t('loadFailed'))
  }
  finally {
    loadingMore.value = false
  }
}
watch(() => props.importId, load, { immediate: true })

// ---------- Поиск, чтобы выбрать самому ----------

const open = ref<string | null>(null)
const query = ref('')
const results = ref<{ id: string, title: string, sub: string, image: ImageSet | null, round?: boolean }[]>([])
const searching = ref(false)

function toggleSearch(item: ImportItem) {
  if (open.value === item.id) {
    open.value = null
    return
  }
  open.value = item.id
  query.value = item.kind === 'artist' ? item.artist : [item.artist, item.title].filter(Boolean).join(' ')
}

let seq = 0
const runSearch = useDebounceFn(async () => {
  const item = items.value.find(i => i.id === open.value)
  const q = query.value.trim()
  if (!item || q.length < 2) {
    results.value = []
    return
  }
  const my = ++seq
  searching.value = true
  try {
    const r = await bridge.catalog.search(q, 'catalog', 6)
    if (my !== seq) return
    if (item.kind === 'album') {
      results.value = r.albums.slice(0, 6).map(a => ({ id: a.id, title: a.title, sub: a.artist.name, image: a.image ?? null }))
    }
    else if (item.kind === 'artist') {
      results.value = r.artists.slice(0, 6).map(a => ({ id: a.id, title: a.name, sub: t('artist'), image: a.image ?? null, round: true }))
    }
    else {
      results.value = r.tracks.slice(0, 6).map(tr => ({
        id: tr.id,
        title: tr.version ? `${tr.title} ${tr.version}` : tr.title,
        sub: [artistNames(tr.artists), tr.album?.title, formatDuration(tr.durationMs)].filter(Boolean).join(' · '),
        image: tr.album?.image ?? null,
      }))
    }
  }
  catch {
    if (my === seq) results.value = []
  }
  finally {
    if (my === seq) searching.value = false
  }
}, 250)
watch(query, () => void runSearch())
watch(open, () => {
  results.value = []
  void runSearch()
})

async function fix(item: ImportItem, fixBody: ImportItemFix, done: string) {
  busy.value = { ...busy.value, [item.id]: true }
  try {
    const updated = await bridge.imports.fix(props.importId, item.id, fixBody)
    items.value = items.value.filter(i => i.id !== item.id)
    if (open.value === item.id) open.value = null
    toast.show({ text: done, icon: updated.status === 'skipped' ? 'ph:arrow-bend-up-right' : 'ph:check-circle-fill' })
    emit('changed')
  }
  catch (e) {
    toast.error((e as Error).message || t('fixFailed'))
  }
  finally {
    busy.value = { ...busy.value, [item.id]: false }
  }
}

function pick(item: ImportItem, id: string) {
  const body: ImportItemFix = item.kind === 'album' ? { albumId: id } : item.kind === 'artist' ? { artistId: id } : { trackId: id }
  return fix(item, body, item.kind === 'file' ? t('fileQueued') : t('added'))
}

const KIND_ICON: Record<string, string> = { track: 'ph:music-note', album: 'ph:vinyl-record', artist: 'ph:user', file: 'ph:file-audio' }
function meta(item: ImportItem): string {
  const parts: string[] = []
  if (item.kind === 'file' && item.file?.error) parts.push(item.file.error)
  if (item.album && item.kind !== 'album') parts.push(item.album)
  if (item.kind === 'album') parts.push(t('album'))
  if (item.kind === 'artist') parts.push(t('follow'))
  if (item.liked) parts.push(t('liked'))
  if (item.plays) {
    const n = formatNumber(item.plays, lang.value)
    parts.push(lang.value === 'en' ? `${n} ${item.plays === 1 ? 'play' : 'plays'}` : `${n}\u00a0${plural(item.plays, lang.value === 'uk' ? ['прослуховування', 'прослуховування', 'прослуховувань'] : ['прослушивание', 'прослушивания', 'прослушиваний'])}`)
  }
  if (item.kind === 'file' && item.file?.name) parts.push(item.file.name)
  return parts.join(' · ')
}
function heading(item: ImportItem): string {
  if (item.kind === 'artist') return item.artist
  return item.artist ? `${item.artist} — ${item.title}` : item.title
}
</script>

<template>
  <section class="rv" :aria-label="t('title')">
    <MusicPageState v-if="state === 'loading'" state="loading" class="rv__state" />
    <UiEmptyState v-else-if="state === 'error'" icon="ph:warning-circle" tone="error" :title="t('loadFailed')" compact>
      <UiButton size="sm" @click="load">{{ t('retry') }}</UiButton>
    </UiEmptyState>
    <UiEmptyState v-else-if="!items.length" icon="ph:check-circle" :title="t('allDone')" :text="t('allDoneText')" compact />
    <ul v-else class="rv__list">
      <li v-for="item in items" :key="item.id" class="rv__item" :class="{ 'is-open': open === item.id, 'is-busy': busy[item.id] }">
        <div class="rv__row">
          <span class="rv__kind" aria-hidden="true"><Icon :name="KIND_ICON[item.kind] ?? 'ph:music-note'" /></span>
          <div class="rv__main">
            <span class="rv__name ellipsis">{{ heading(item) }}</span>
            <span v-if="meta(item)" class="rv__meta ellipsis">{{ meta(item) }}</span>
          </div>
          <div class="rv__acts">
            <UiButton size="sm" :icon="open === item.id ? 'ph:x' : 'ph:magnifying-glass'" @click="toggleSearch(item)">{{ open === item.id ? t('close') : t('find') }}</UiButton>
            <UiIconButton icon="ph:arrow-bend-up-right" :label="t('skip')" size="sm" :disabled="busy[item.id]" @click="fix(item, { skip: true }, t('skipped'))" />
          </div>
        </div>

        <div v-for="c in item.candidates.slice(0, 1)" :key="c.deezerId" class="rv__guess">
          <span class="rv__guess-text">
            <span class="rv__guess-label">{{ t('guess') }}</span>
            <span class="ellipsis">{{ c.artist }} — {{ c.title }}<template v-if="c.album"> · {{ c.album }}</template></span>
          </span>
          <UiButton size="sm" variant="primary" icon="ph:check" :loading="busy[item.id]" @click="fix(item, { deezerId: c.deezerId }, item.kind === 'file' ? t('fileQueued') : t('added'))">{{ t('yes') }}</UiButton>
        </div>

        <div v-if="open === item.id" class="rv__search">
          <UiSearchField v-model="query" :placeholder="t('searchPlaceholder')" autofocus />
          <div v-if="searching && !results.length" class="rv__hint">{{ t('searching') }}</div>
          <div v-else-if="!results.length && query.trim().length >= 2" class="rv__hint">{{ t('nothing') }}</div>
          <div v-else class="rv__results">
            <SearchItemRow
              v-for="r in results"
              :key="r.id"
              :title="r.title"
              :sub="r.sub"
              :image="r.image"
              :round="r.round"
              @open="pick(item, r.id)"
            >
              <template #end><Icon name="ph:plus-circle" class="rv__add" /></template>
            </SearchItemRow>
          </div>
        </div>
      </li>
    </ul>
    <UiButton v-if="cursor" class="rv__more" :loading="loadingMore" @click="more">{{ t('more') }}</UiButton>
  </section>
</template>

<style scoped>
.rv { display: flex; flex-direction: column; gap: 12px; }
.rv__state { min-height: 160px; }
.rv__list { display: flex; flex-direction: column; margin: 0; padding: 0; list-style: none; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); }
.rv__item { display: flex; flex-direction: column; gap: 10px; padding: 14px 16px; transition: opacity var(--dur-2); }
.rv__item + .rv__item { box-shadow: inset 0 1px 0 var(--hair); }
.rv__item.is-busy { opacity: 0.6; }
.rv__item.is-open { background: var(--fill-1); }
.rv__row { display: flex; align-items: center; gap: 12px; min-height: 44px; }
.rv__kind { display: grid; flex: none; place-items: center; width: 40px; height: 40px; border-radius: var(--r-3); background: var(--fill-2); color: var(--text-3); font-size: 18px; }
.rv__main { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.rv__name { font-size: var(--fs-body); font-weight: 600; }
.rv__meta { font-size: var(--fs-small); color: var(--text-3); }
.rv__acts { display: flex; flex: none; align-items: center; gap: 4px; }
.rv__guess { display: flex; align-items: center; gap: 12px; margin-left: 52px; padding: 8px 8px 8px 14px; border-radius: var(--r-3); background: var(--accent-soft); box-shadow: inset 0 0 0 1px var(--accent-line); }
.rv__guess-text { display: flex; flex: 1; flex-direction: column; min-width: 0; font-size: var(--fs-small); }
.rv__guess-label { font-size: var(--fs-caption); color: var(--text-3); }
.rv__search { display: flex; flex-direction: column; gap: 8px; margin-left: 52px; }
.rv__results { display: flex; flex-direction: column; }
.rv__hint { padding: 8px 4px; font-size: var(--fs-small); color: var(--text-3); }
.rv__add { font-size: 22px; color: var(--text-3); }
.rv__more { align-self: center; }

@media (max-width: 699px) {
  .rv__list { border-radius: 0; margin: 0 -16px; box-shadow: inset 0 1px 0 var(--hair), inset 0 -1px 0 var(--hair); }
  .rv__item { padding: 12px 16px; }
  .rv__kind { width: 36px; height: 36px; }
  .rv__acts :deep(.btn__label) { display: none; }
  .rv__acts :deep(.btn--sm) { width: 36px; padding: 0; }
  .rv__guess, .rv__search { margin-left: 0; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Не нашлось",
    "find": "Найти",
    "close": "Закрыть",
    "skip": "Пропустить",
    "skipped": "Пропущено",
    "guess": "Похоже на",
    "yes": "Да, это он",
    "added": "Готово — добавлено в медиатеку",
    "fileQueued": "Файл кладём к выбранному треку",
    "fixFailed": "Не получилось — попробуйте ещё раз",
    "loadFailed": "Не удалось загрузить список",
    "retry": "Повторить",
    "allDone": "Всё разобрано",
    "allDoneText": "Ненайденного больше нет",
    "searchPlaceholder": "Артист и название",
    "searching": "Ищем…",
    "nothing": "Ничего не нашлось — попробуйте написать иначе",
    "more": "Показать ещё",
    "album": "альбом",
    "artist": "Артист",
    "follow": "подписка",
    "liked": "в лайках"
  },
  "uk": {
    "title": "Не знайшлося",
    "find": "Знайти",
    "close": "Закрити",
    "skip": "Пропустити",
    "skipped": "Пропущено",
    "guess": "Схоже на",
    "yes": "Так, це він",
    "added": "Готово — додано до медіатеки",
    "fileQueued": "Файл додаємо до вибраного трека",
    "fixFailed": "Не вдалося — спробуйте ще раз",
    "loadFailed": "Не вдалося завантажити список",
    "retry": "Повторити",
    "allDone": "Усе розібрано",
    "allDoneText": "Незнайденого більше немає",
    "searchPlaceholder": "Виконавець і назва",
    "searching": "Шукаємо…",
    "nothing": "Нічого не знайшлося — спробуйте написати інакше",
    "more": "Показати ще",
    "album": "альбом",
    "artist": "Виконавець",
    "follow": "підписка",
    "liked": "у вподобаних"
  },
  "en": {
    "title": "Not found",
    "find": "Find",
    "close": "Close",
    "skip": "Skip",
    "skipped": "Skipped",
    "guess": "Looks like",
    "yes": "Yes, that's it",
    "added": "Done — added to your library",
    "fileQueued": "Attaching the file to the chosen track",
    "fixFailed": "That didn't work — try again",
    "loadFailed": "Couldn't load the list",
    "retry": "Retry",
    "allDone": "All sorted",
    "allDoneText": "Nothing left unmatched",
    "searchPlaceholder": "Artist and title",
    "searching": "Searching…",
    "nothing": "Nothing found — try different words",
    "more": "Show more",
    "album": "album",
    "artist": "Artist",
    "follow": "follow",
    "liked": "liked"
  }
}
</i18n>
