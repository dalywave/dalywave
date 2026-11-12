<script setup lang="ts">
import type { HistoryItem } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// «Недавно прослушанное»: прослушивания по дням (сегодня, вчера, даты), у каждого — время и откуда
// играл. Поиск по истории, переход к дате календарём, «Очистить историю». История из архива Spotify — тоже
const bridge = useBridge()
const ui = useUiStore()
const player = usePlayerStore()
const toast = useToast()
const lang = useLang()
const pui = usePlaylistUi()
const { t } = useI18n({ useScope: 'local' })
const { tg, te } = useGlobalT()
useHead({ title: () => t('title') })

const PAGE = 60
const items = ref<HistoryItem[]>([])
const cursor = ref<string | null>(null)
const state = ref<'loading' | 'ready' | 'error'>('loading')
const more = ref(false)
const query = ref('')
const field = ref('')
/** Переход к дате: `YYYY-MM-DD` по местному времени */
const day = ref<string | null>(null)
let seq = 0

const pad = (n: number) => String(n).padStart(2, '0')
const keyOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const dateOf = (key: string) => {
  const [y, m, d] = key.split('-').map(Number) as [number, number, number]
  return new Date(y, m - 1, d)
}
/** Начало следующего дня: всё, что раньше, — этот день и прошлые */
const beforeOf = (key: string) => {
  const d = dateOf(key)
  d.setDate(d.getDate() + 1)
  return d.toISOString()
}

async function load() {
  const my = ++seq
  if (!items.value.length) state.value = 'loading'
  try {
    const page = await bridge.library.history(null, { q: query.value, before: day.value ? beforeOf(day.value) : undefined, limit: PAGE })
    if (my !== seq) return
    items.value = page.items
    cursor.value = page.nextCursor ?? null
    state.value = 'ready'
    if (import.meta.client) window.scrollTo({ top: 0 })
  }
  catch {
    if (my === seq) state.value = 'error'
  }
}
onMounted(load)
watch([query, day], () => void load())

async function loadMore() {
  if (!cursor.value || more.value) return
  const my = seq
  more.value = true
  try {
    const page = await bridge.library.history(cursor.value, { q: query.value, limit: PAGE })
    if (my !== seq) return
    items.value.push(...page.items)
    cursor.value = page.nextCursor ?? null
  }
  catch {
    // Следующая попытка — при следующей прокрутке
  }
  finally {
    more.value = false
  }
}

// Подгрузка, когда низ списка близко
const sentinel = ref<HTMLElement | null>(null)
let io: IntersectionObserver | null = null
watch(sentinel, (el) => {
  io?.disconnect()
  if (!el) return
  io = new IntersectionObserver(([e]) => {
    if (e?.isIntersecting) void loadMore()
  }, { rootMargin: '600px 0px' })
  io.observe(el)
})
onBeforeUnmount(() => io?.disconnect())

let timer: ReturnType<typeof setTimeout> | undefined
watch(field, (v) => {
  clearTimeout(timer)
  timer = setTimeout(() => (query.value = v.trim()), 250)
})

// ---------- Дни ----------

const locale = computed(() => (lang.value === 'en' ? 'en-GB' : 'ru-RU'))
function dayLabel(key: string) {
  const now = new Date()
  if (key === keyOf(now)) return t('today')
  const y = new Date(now)
  y.setDate(y.getDate() - 1)
  if (key === keyOf(y)) return t('yesterday')
  const d = dateOf(key)
  return new Intl.DateTimeFormat(locale.value, d.getFullYear() === now.getFullYear() ? { day: 'numeric', month: 'long' } : { day: 'numeric', month: 'long', year: 'numeric' })
    .format(d).replace(/\s*г\.$/, '')
}
const groups = computed(() => {
  const out: { key: string, label: string, items: HistoryItem[] }[] = []
  for (const it of items.value) {
    const key = keyOf(new Date(it.startedAt))
    let g = out[out.length - 1]
    if (!g || g.key !== key) {
      g = { key, label: dayLabel(key), items: [] }
      out.push(g)
    }
    g.items.push(it)
  }
  return out
})

// ---------- Откуда играл ----------

function contextOf(it: HistoryItem): string | null {
  if (it.imported) return 'Spotify'
  switch (it.contextType) {
    case 'wave': return tg('wave.title')
    case 'radio': return t('ctx.radio')
    case 'search': return t('ctx.search')
    case 'album':
    case 'playlist':
    case 'artist':
      return it.contextTitle ? t('ctx.from', { title: it.contextTitle }) : null
    case 'mix': {
      // Полки главной: id полки — название из общего словаря
      const key = `home.shelf.${it.contextId}`
      return it.contextId && te(key) ? t('ctx.from', { title: tg(key) }) : null
    }
    default: return null
  }
}

function sourceLink(it: HistoryItem): string | null {
  const id = it.contextId
  if (!id || it.imported) return null
  if (it.contextType === 'album') return `/album/${id}`
  if (it.contextType === 'playlist') return `/playlist/${id}`
  if (it.contextType === 'artist') return `/artist/${id}`
  return null
}

// ---------- Действия ----------

/** Нажатие — трек и дальше по этому дню */
function play(group: HistoryItem[], i: number) {
  const tr = group[i]?.track
  if (!tr) return
  if (player.current?.track.id === tr.id && i === 0) return player.toggle()
  const list = group.slice(i).map(x => x.track)
  return player.playTracks(list, 0, { type: 'library', id: 'history', title: t('title') })
}

async function hide(it: HistoryItem) {
  const at = items.value.indexOf(it)
  if (at >= 0) items.value.splice(at, 1)
  try {
    await bridge.library.hidePlay(it.id)
    toast.show({ text: t('hidden'), icon: 'ph:clock-counter-clockwise' })
  }
  catch {
    if (at >= 0) items.value.splice(at, 0, it)
    toast.error(t('hideFailed'))
  }
}

function extraFor(it: HistoryItem): () => MenuEntry[] {
  return () => {
    const out: MenuEntry[] = []
    const link = sourceLink(it)
    if (link && it.contextTitle) out.push({ label: t('openSource'), icon: 'ph:arrow-square-out', onSelect: () => void ui.navigate(link) })
    out.push({ label: t('hide'), icon: 'ph:eye-slash', onSelect: () => void hide(it) })
    return out
  }
}

async function clearAll() {
  const ok = await pui.confirm({ title: t('clearTitle'), text: t('clearText'), action: t('clear'), danger: true })
  if (!ok) return
  try {
    await bridge.library.clearHistory()
    items.value = []
    cursor.value = null
    day.value = null
    toast.show({ text: t('cleared'), icon: 'ph:trash' })
  }
  catch {
    toast.error(t('clearFailed'))
  }
}
const moreItems = computed<MenuEntry[]>(() => [
  { label: t('clear'), icon: 'ph:trash', danger: true, disabled: !items.value.length && !query.value && !day.value, onSelect: () => void clearAll() },
])

// ---------- Переход к дате ----------

const calOpen = ref(false)
const days = ref<Set<string>>(new Set())
watch(calOpen, async (open) => {
  if (!open) return
  try {
    const r = await bridge.library.historyDays()
    days.value = new Set(r.days.map(d => d.day))
  }
  catch {
    // Без календаря дней — все дни неактивны, переход недоступен
  }
})
function pick(key: string) {
  calOpen.value = false
  day.value = key === keyOf(new Date()) ? null : key
}
const dayChip = computed(() => (day.value ? dayLabel(day.value) : null))

// Телефон: название уехало под верхнюю строку — она показывает его сама
const titleEl = ref<HTMLElement | null>(null)
const condensed = ref(false)
let tio: IntersectionObserver | null = null
watch(titleEl, (el) => {
  tio?.disconnect()
  if (!el) return
  tio = new IntersectionObserver(([e]) => {
    if (e) condensed.value = !e.isIntersecting && e.boundingClientRect.top < 100
  }, { rootMargin: '-100px 0px 0px 0px' })
  tio.observe(el)
})
onBeforeUnmount(() => tio?.disconnect())

const empty = computed(() => state.value === 'ready' && !items.value.length)
</script>

<template>
  <div class="hist">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="t('title')" :condensed="condensed" fallback="/library">
      <UiIconButton icon="ph:calendar-blank" :label="t('jump')" size="lg" :tooltip="false" @click="calOpen = true" />
      <UiMenu :items="moreItems">
        <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
      </UiMenu>
    </MusicPhoneNav>

    <header class="head">
      <h1 ref="titleEl" class="head__title">{{ t('title') }}</h1>
      <div v-if="!ui.isTouchUi" class="head__tools">
        <UiPopover v-model:open="calOpen" side="bottom" align="end" :width="320" no-autofocus>
          <template #trigger>
            <UiButton size="sm" variant="secondary" icon="ph:calendar-blank">{{ t('jump') }}</UiButton>
          </template>
          <HistoryCalendar :days="days" :selected="day" @pick="pick" />
        </UiPopover>
        <UiMenu :items="moreItems">
          <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="sm" />
        </UiMenu>
      </div>
    </header>

    <div class="bar">
      <UiSearchField v-model="field" class="bar__search" :placeholder="t('search')" />
      <button v-if="dayChip" type="button" class="chip" :aria-label="t('resetDay')" @click="day = null">
        <span>{{ t('until', { day: dayChip }) }}</span>
        <Icon name="ph:x-bold" class="chip__x" />
      </button>
    </div>

    <UiSheet v-if="ui.isTouchUi" v-model:open="calOpen" :title="t('jump')">
      <div class="calsheet">
        <HistoryCalendar :days="days" :selected="day" @pick="pick" />
      </div>
    </UiSheet>

    <MusicPageState v-if="state === 'error' && !items.length" state="error" @retry="load" />
    <div v-else-if="state === 'loading' && !items.length" class="skel" aria-busy="true">
      <UiSkeleton variant="line" width="96px" height="18px" />
      <div v-for="n in 8" :key="n" class="skel__row">
        <UiSkeleton class="skel__art" />
        <div class="skel__lines"><UiSkeleton variant="line" width="48%" /><UiSkeleton variant="line" width="30%" /></div>
      </div>
    </div>
    <UiEmptyState v-else-if="empty && (query || day)" icon="ph:magnifying-glass" :title="t('nothing')" :text="query ? t('nothingFor', { q: query }) : undefined" />
    <UiEmptyState v-else-if="empty" icon="ph:clock-counter-clockwise" :title="t('empty')" :text="t('emptyText')">
      <UiButton icon="ph:play-fill" @click="player.startWave()">{{ tg('wave.listen') }}</UiButton>
    </UiEmptyState>

    <div v-else class="days" :class="{ 'is-stale': state === 'loading' }">
      <section v-for="g in groups" :key="g.key" class="day" :aria-label="g.label">
        <h2 class="day__title">{{ g.label }}</h2>
        <div class="day__list" role="table" :aria-label="g.label">
          <HistoryRow
            v-for="(it, i) in g.items"
            :key="it.id"
            :item="it"
            :context="contextOf(it)"
            :extra="extraFor(it)"
            @play="play(g.items, i)"
          />
        </div>
      </section>
      <div ref="sentinel" class="sentinel" aria-hidden="true" />
      <div v-if="more" class="more" aria-busy="true">
        <div v-for="n in 3" :key="n" class="skel__row">
          <UiSkeleton class="skel__art" />
          <div class="skel__lines"><UiSkeleton variant="line" width="44%" /><UiSkeleton variant="line" width="26%" /></div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hist { display: flex; flex-direction: column; gap: 20px; padding-top: 16px; }
.head { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 44px; }
.head__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.head__tools { display: flex; align-items: center; gap: 8px; }
.bar { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.bar__search { width: min(360px, 100%); }
.chip { display: inline-flex; align-items: center; gap: 8px; height: 36px; padding: 0 12px 0 14px; border: 0; border-radius: var(--r-pill); background: var(--accent); color: var(--ink-1); font: inherit; font-size: var(--fs-small); font-weight: 600; }
.chip:hover { filter: brightness(1.06); }
.chip:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.chip__x { font-size: 12px; }
.calsheet { padding: 0 16px 16px; }

.days { display: flex; flex-direction: column; gap: 24px; transition: opacity var(--dur-2); }
.days.is-stale { opacity: 0.6; }
.day { display: flex; flex-direction: column; gap: 4px; }
.day__title { margin: 0; padding: 8px 0 4px; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); color: var(--text-1); }
.day__list { display: flex; flex-direction: column; margin: 0 -8px; }
.sentinel { height: 1px; }
.more { display: flex; flex-direction: column; }

.skel { display: flex; flex-direction: column; gap: 4px; }
.skel__row { display: flex; align-items: center; gap: 12px; min-height: 56px; }
.skel__art { width: 40px; height: 40px; flex: none; border-radius: 6px; }
.skel__lines { display: flex; flex: 1; flex-direction: column; gap: 8px; }

@media (max-width: 699px) {
  .hist { gap: 16px; padding: 0 16px; }
  .hist > .pn { margin: calc(var(--safe-top) * -1) -16px -8px; }
  .head__title { font-size: 30px; }
  .bar__search { width: 100%; }
  .bar :deep(.sf) { height: 40px; border-radius: 10px; background: var(--fill-2); box-shadow: none; }
  .bar :deep(.sf__input) { font-size: 16px; }
  .day__title { font-size: 20px; }
  .day__list { margin: 0; }
  .skel__art { width: 48px; height: 48px; }
  .skel__row { min-height: 64px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Недавно прослушанное",
    "search": "Поиск в истории",
    "jump": "Перейти к дате",
    "until": "По {day}",
    "resetDay": "Показать с начала",
    "today": "Сегодня",
    "yesterday": "Вчера",
    "ctx": { "from": "из «{title}»", "radio": "Радио", "search": "из поиска" },
    "openSource": "Перейти к источнику",
    "hide": "Убрать из истории",
    "hidden": "Убрано из истории",
    "hideFailed": "Не получилось убрать",
    "clear": "Очистить историю",
    "clearTitle": "Очистить историю?",
    "clearText": "Рекомендации и миксы это не изменит",
    "cleared": "История очищена",
    "clearFailed": "Не получилось очистить историю",
    "empty": "Здесь появится то, что вы слушаете",
    "emptyText": "Включите волну или любимый альбом",
    "nothing": "Ничего не нашлось",
    "nothingFor": "По запросу «{q}»"
  },
  "en": {
    "title": "Recently played",
    "search": "Search history",
    "jump": "Go to date",
    "until": "Up to {day}",
    "resetDay": "Show from the start",
    "today": "Today",
    "yesterday": "Yesterday",
    "ctx": { "from": "from “{title}”", "radio": "Radio", "search": "from search" },
    "openSource": "Go to source",
    "hide": "Remove from history",
    "hidden": "Removed from history",
    "hideFailed": "Couldn't remove",
    "clear": "Clear history",
    "clearTitle": "Clear history?",
    "clearText": "Recommendations and mixes won't change",
    "cleared": "History cleared",
    "clearFailed": "Couldn't clear history",
    "empty": "What you play will show up here",
    "emptyText": "Start the wave or a favourite album",
    "nothing": "Nothing found",
    "nothingFor": "For “{q}”"
  }
}
</i18n>
