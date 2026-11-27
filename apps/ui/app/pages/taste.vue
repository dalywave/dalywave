<script setup lang="ts">
import type { BlockedItem, TasteFacet, TasteOverview } from '~/api/types'

// «Мой вкус»: из чего волна знает слушателя и как это поправить. Отпечаток вкуса (доли граней), грани
// с обложками и «Меньше такого», месяц в цифрах, «Не рекомендовать», что знает волна, сброс.
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const ui = useUiStore()
const { t } = useScopedI18n('pages.taste')
useHead({ title: () => t('title') })

// Цвета долей — токены --chart-*: на светлой теме у них свои, плотнее
const COLORS = [1, 2, 3, 4, 5, 6].map(n => `var(--chart-${n})`)

const { data, state, load } = useLoad<TasteOverview>(() => bridge.taste.overview())
const blocked = ref<BlockedItem[] | null>(null)
async function loadBlocked() {
  try {
    blocked.value = (await bridge.library.blocked()).items
  }
  catch {
    blocked.value = []
  }
}
onMounted(loadBlocked)

const facets = computed(() => data.value?.facets ?? [])
const facetsWord = computed(() => {
  const n = facets.value.length
  return lang.value === 'en' ? (n === 1 ? 'facet' : 'facets') : plural(n, lang.value === 'uk' ? ['грань', 'грані', 'граней'] : ['грань', 'грани', 'граней'])
})
const colorOf = (i: number) => COLORS[i % COLORS.length]!
const stats = computed(() => data.value?.stats ?? null)
const knows = computed(() => data.value?.knows ?? null)

// ---------- «Меньше такого» ----------

const busy = ref<string | null>(null)
async function less(f: TasteFacet) {
  busy.value = f.id
  try {
    const d = await bridge.taste.less(f.id)
    await load()
    toast.show({ text: t('lessDone', { label: f.label }), icon: 'ph:minus-circle', undo: () => void undo(d.id, false) })
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = null
  }
}
async function undo(damperId: string, notify = true) {
  busy.value = damperId
  try {
    await bridge.taste.undoLess(damperId)
    await load()
    if (notify) toast.show({ text: t('undoDone'), icon: 'ph:arrow-counter-clockwise' })
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = null
  }
}
/** Приглушения, у которых грани больше нет (вкус пересобрался) — отдельным списком */
const orphanDampers = computed(() => {
  const used = new Set(facets.value.map(f => f.damperId).filter(Boolean))
  return (data.value?.dampers ?? []).filter(d => !used.has(d.id))
})

// ---------- Месяц в цифрах ----------

const monthName = computed(() => {
  if (!stats.value) return ''
  const m = new Intl.DateTimeFormat(intlLocale(lang.value, 'en-GB'), { month: 'long' }).format(new Date(new Date(stats.value.since).getTime() + 2 * 86_400_000))
  return m.charAt(0).toUpperCase() + m.slice(1)
})
const hours = computed(() => {
  const min = stats.value?.minutes ?? 0
  return { h: Math.floor(min / 60), m: min % 60 }
})
const delta = computed(() => {
  const s = stats.value
  if (!s || !s.previousMinutes) return null
  const diff = s.minutes - s.previousMinutes
  const pct = Math.round((diff / s.previousMinutes) * 100)
  return Math.abs(pct) < 1 ? null : pct
})
const maxArtistMin = computed(() => Math.max(1, ...(stats.value?.topArtists.map(a => a.minutes) ?? [1])))
const maxPart = computed(() => Math.max(0.01, ...(stats.value?.dayParts.map(p => p.share) ?? [0.01])))
const PART_ICONS: Record<string, string> = { morning: 'ph:sun-horizon', day: 'ph:sun', evening: 'ph:moon-stars', night: 'ph:moon' }
// Сервер отдаёт жанр Deezer (id) и его русское название (name); украинские — здесь, незнакомый остаётся как у Deezer
const GENRE_UK: Record<string, string> = {
  'Pop': 'Поп',
  'Rap/Hip Hop': 'Реп і хіп-хоп',
  'Rock': 'Рок',
  'Alternative': 'Альтернатива',
  'Indie Rock': 'Інді-рок',
  'Indie Pop': 'Інді-поп',
  'Dance': 'Танцювальна',
  'Electro': 'Електроніка',
  'Techno/House': 'Техно і хаус',
  'R&B': 'R&B',
  'Soul & Funk': 'Соул і фанк',
  'Metal': 'Метал',
  'Jazz': 'Джаз',
  'Classical': 'Класика',
  'Films/Games': 'Кіно та ігри',
  'Reggae': 'Реґі',
  'Reggaeton': 'Реґетон',
  'Folk': 'Фолк',
  'Blues': 'Блюз',
  'Country': 'Кантрі',
  'Latin Music': 'Латино',
  'Asian Music': 'Азійська',
  'African Music': 'Африканська',
  'Kids': 'Дитяча',
  'Chanson': 'Шансон',
  'Variété Internationale': 'Естрада',
}
const genreName = (g: { id: string, name: string }) => (lang.value === 'en' ? g.id : lang.value === 'uk' ? (GENRE_UK[g.id] ?? g.id) : g.name)

// ---------- Не рекомендовать ----------

async function unblock(item: BlockedItem) {
  const before = blocked.value
  blocked.value = (blocked.value ?? []).filter(b => !(b.kind === item.kind && b.id === item.id))
  try {
    await bridge.library.unblock(item.kind, item.id)
    toast.show({
      text: t('unblocked', { name: item.track?.title ?? item.artist?.name ?? '' }),
      icon: 'ph:check',
      undo: () => void bridge.library.block(item.kind, item.id).then(loadBlocked).catch(() => toast.error(t('failed'))),
    })
  }
  catch {
    blocked.value = before
    toast.error(t('failed'))
  }
}

// ---------- Сброс ----------

const confirming = ref(false)
const resetting = ref(false)
async function reset() {
  resetting.value = true
  try {
    await bridge.taste.reset()
    confirming.value = false
    await load()
    toast.show({ text: t('resetDone'), icon: 'ph:arrow-counter-clockwise' })
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    resetting.value = false
  }
}

// Телефон: название уехало под верхнюю строку — она показывает его сама
const titleEl = ref<HTMLElement | null>(null)
const condensed = ref(false)
let io: IntersectionObserver | null = null
watch(titleEl, (el) => {
  io?.disconnect()
  if (!el) return
  io = new IntersectionObserver(([e]) => {
    if (e) condensed.value = !e.isIntersecting && e.boundingClientRect.top < 100
  }, { rootMargin: '-100px 0px 0px 0px' })
  io.observe(el)
})
onBeforeUnmount(() => io?.disconnect())

const analyzedShare = computed(() => {
  const k = knows.value
  if (!k) return 0
  const all = Math.max(k.likes, k.listened, 1)
  return Math.min(100, Math.round((k.analyzed / all) * 100))
})
</script>

<template>
  <div class="taste">
    <MusicPhoneNav :title="t('title')" :condensed="condensed" />
    <MusicPageState v-if="state !== 'ready'" :state="state === 'missing' ? 'error' : state" @retry="load" />

    <template v-else-if="data">
      <!-- Отпечаток: доли граней одной полосой -->
      <header class="head">
        <div class="head__kicker">{{ t('kicker') }}</div>
        <h1 ref="titleEl" class="head__title">{{ t('title') }}</h1>
        <p v-if="facets.length" class="head__sub">{{ t('sub', { n: facets.length, word: facetsWord }) }}</p>
        <p v-else class="head__sub">{{ t('subEmpty') }}</p>
        <div v-if="facets.length" class="print" role="img" :aria-label="facets.map(f => `${f.label} ${Math.round(f.share * 100)} %`).join(', ')">
          <span
            v-for="(f, i) in facets"
            :key="f.id"
            class="print__seg"
            :class="{ 'is-damped': f.damped > 0.05 }"
            :style="{ flexGrow: Math.max(f.share, 0.02), '--c': colorOf(i) }"
          />
        </div>
      </header>

      <section v-if="facets.length" class="sec" :aria-label="t('facets')">
        <div class="facets">
          <TasteFacetCard
            v-for="(f, i) in facets"
            :key="f.id"
            :facet="f"
            :color="colorOf(i)"
            :busy="busy === f.id || (!!f.damperId && busy === f.damperId)"
            @less="less(f)"
            @undo="f.damperId && undo(f.damperId)"
          />
        </div>
        <div v-if="orphanDampers.length" class="dampers">
          <span class="dampers__label">{{ t('lessList') }}</span>
          <span v-for="d in orphanDampers" :key="d.id" class="chip">
            {{ d.label }}
            <button type="button" class="chip__x" :aria-label="t('undo')" @click="undo(d.id)"><Icon name="ph:x-bold" /></button>
          </span>
        </div>
      </section>
      <UiEmptyState v-else icon="ph:fingerprint" :title="t('emptyTitle')" :text="t('emptyText')" />

      <!-- Итоги года с прошлыми годами — пока событие открыто -->
      <RewindCard past-years class="sec" />

      <!-- Месяц в цифрах -->
      <section v-if="stats" class="sec">
        <h2 class="sec__title">{{ monthName }}</h2>
        <div class="month">
          <div class="big">
            <div class="big__main tnum">
              <template v-if="hours.h">{{ hours.h }}<small>{{ t('h') }}</small></template>{{ hours.m }}<small>{{ t('min') }}</small>
            </div>
            <div class="big__sub">
              <span>{{ formatCount(stats.tracks, 'track', lang) }} · {{ formatCount(stats.artists, 'artist', lang) }}</span>
              <span v-if="delta !== null" class="big__delta" :class="delta > 0 ? 'is-up' : 'is-down'">
                <Icon :name="delta > 0 ? 'ph:arrow-up-right-bold' : 'ph:arrow-down-right-bold'" />{{ Math.abs(delta) }}&#8239;%
              </span>
            </div>
            <div class="parts" role="img" :aria-label="stats.dayParts.map(p => `${t(`part.${p.part}`)} ${Math.round(p.share * 100)} %`).join(', ')">
              <div v-for="p in stats.dayParts" :key="p.part" class="parts__col" :class="{ 'is-peak': p.share > 0 && p.share >= maxPart }">
                <span class="parts__bar"><i :style="{ height: `${Math.max(4, (p.share / maxPart) * 100)}%` }" /></span>
                <Icon :name="PART_ICONS[p.part] ?? 'ph:clock'" class="parts__icon" />
                <span class="parts__label">{{ t(`part.${p.part}`) }}</span>
              </div>
            </div>
          </div>

          <div v-if="stats.topArtists.length" class="card">
            <h3 class="card__title">{{ t('topArtists') }}</h3>
            <ol class="tops">
              <li v-for="(a, i) in stats.topArtists" :key="a.artist.id">
                <NuxtLink :to="`/artist/${a.artist.id}`" class="top">
                  <span class="top__n tnum">{{ i + 1 }}</span>
                  <MusicCoverArt :image="a.artist.image" size="small" round class="top__img" fallback-icon="ph:user" />
                  <span class="top__body">
                    <span class="top__name ellipsis">{{ a.artist.name }}</span>
                    <span class="top__bar"><i :style="{ width: `${(a.minutes / maxArtistMin) * 100}%` }" /></span>
                  </span>
                  <span class="top__min tnum">{{ formatLongDuration(a.minutes * 60000, lang) }}</span>
                </NuxtLink>
              </li>
            </ol>
          </div>

          <div v-if="stats.topGenres.length" class="card">
            <h3 class="card__title">{{ t('topGenres') }}</h3>
            <ul class="genres">
              <li v-for="(g, i) in stats.topGenres" :key="g.id" class="genre">
                <span class="genre__name ellipsis">{{ genreName(g) }}</span>
                <span class="genre__pct tnum">{{ Math.round(g.share * 100) }}&#8239;%</span>
                <span class="genre__bar"><i :style="{ width: `${Math.max(2, g.share * 100)}%`, background: colorOf(i) }" /></span>
              </li>
            </ul>
          </div>
        </div>
        <p v-if="!stats.plays" class="quiet">{{ t('noPlays') }}</p>
      </section>

      <!-- Не рекомендовать -->
      <section class="sec">
        <h2 class="sec__title">{{ t('blocked') }}</h2>
        <div v-if="blocked && blocked.length" class="blocked">
          <div v-for="b in blocked" :key="`${b.kind}:${b.id}`" class="brow">
            <MusicCoverArt
              :image="b.kind === 'artist' ? b.artist?.image ?? null : b.track?.album?.image ?? null"
              size="small"
              :round="b.kind === 'artist'"
              class="brow__img"
              :fallback-icon="b.kind === 'artist' ? 'ph:user' : 'ph:music-notes-simple'"
            />
            <span class="brow__body">
              <span class="brow__name ellipsis">{{ b.kind === 'artist' ? b.artist?.name : b.track?.title }}</span>
              <span class="brow__meta ellipsis">{{ b.kind === 'artist' ? t('artist') : (b.track?.artists.map(a => a.name).join(', ') ?? t('track')) }}</span>
            </span>
            <UiIconButton icon="ph:x" :label="t('unblock')" :size="ui.isTouchUi ? 'md' : 'sm'" @click="unblock(b)" />
          </div>
        </div>
        <p v-else-if="blocked" class="quiet">{{ t('blockedEmpty') }}</p>
      </section>

      <!-- Что знает волна -->
      <section v-if="knows" class="sec">
        <h2 class="sec__title">{{ t('knows') }}</h2>
        <div class="knows">
          <div class="kn"><span class="kn__n tnum">{{ formatNumber(knows.likes, lang) }}</span><span class="kn__l">{{ t('kLikes') }}</span></div>
          <div class="kn"><span class="kn__n tnum">{{ formatNumber(knows.listened, lang) }}</span><span class="kn__l">{{ t('kListened') }}</span></div>
          <div class="kn">
            <span class="kn__n tnum">{{ analyzedShare }}&#8239;%</span><span class="kn__l">{{ t('kAnalyzed') }}</span>
          </div>
          <div class="kn"><span class="kn__n tnum">{{ formatNumber(knows.reactions, lang) }}</span><span class="kn__l">{{ t('kReactions') }}</span></div>
          <div class="kn"><span class="kn__n tnum">{{ formatNumber(knows.dislikes + knows.blocked, lang) }}</span><span class="kn__l">{{ t('kNo') }}</span></div>
        </div>
        <p class="quiet">
          {{ knows.updatedAt ? t('updated', { when: formatRelative(knows.updatedAt, lang) }) : t('notYet') }}
          <template v-if="knows.resetAt"> · {{ t('wasReset', { when: formatRelative(knows.resetAt, lang) }) }}</template>
        </p>
      </section>

      <!-- Сброс -->
      <section class="sec">
        <div class="reset" :class="{ 'is-open': confirming }">
          <div class="reset__text">
            <h2 class="reset__title">{{ confirming ? t('resetSure') : t('reset') }}</h2>
            <p class="reset__desc">{{ t('resetDesc') }}</p>
          </div>
          <div class="reset__actions">
            <template v-if="confirming">
              <UiButton variant="ghost" :disabled="resetting" @click="confirming = false">{{ t('cancel') }}</UiButton>
              <UiButton variant="danger" icon="ph:arrow-counter-clockwise" :loading="resetting" @click="reset">{{ t('resetBtn') }}</UiButton>
            </template>
            <UiButton v-else variant="secondary" @click="confirming = true">{{ t('resetBtn') }}</UiButton>
          </div>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.taste { display: flex; flex-direction: column; gap: 48px; width: 100%; max-width: 1120px; margin: 0 auto; padding-top: 16px; }

.head { display: flex; flex-direction: column; gap: 8px; }
.head__kicker { font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.head__title { margin: 0; font-size: var(--fs-display); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1; }
.head__sub { margin: 4px 0 0; font-size: var(--fs-h4); color: var(--text-2); }
.print { display: flex; gap: 4px; height: 12px; margin-top: 20px; }
.print__seg { min-width: 6px; border-radius: var(--r-pill); background: var(--c); transition: flex-grow var(--dur-4) var(--ease-out), opacity var(--dur-3); }
.print__seg.is-damped { opacity: 0.35; }

.sec { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
.sec__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.quiet { margin: 0; font-size: var(--fs-small); color: var(--text-3); }

.facets { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.dampers { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.dampers__label { font-size: var(--fs-small); color: var(--text-3); }
.chip { display: inline-flex; align-items: center; gap: 4px; padding: 4px 4px 4px 12px; border-radius: var(--r-pill); background: var(--fill-2); font-size: var(--fs-small); }
.chip__x { display: grid; place-items: center; width: 24px; height: 24px; border: 0; border-radius: 50%; background: transparent; color: var(--text-2); cursor: pointer; }
.chip__x:hover { background: var(--fill-3); color: var(--text-1); }
.chip__x:focus-visible { outline: 2px solid var(--focus); outline-offset: 1px; }

.month { display: grid; grid-template-columns: 1.1fr 1fr 1fr; gap: 16px; }
.big, .card { display: flex; flex-direction: column; gap: 12px; min-width: 0; padding: 20px; border-radius: var(--r-5); background: var(--ink-2); box-shadow: inset 0 0 0 1px var(--hair); }
.big__main { font-size: 56px; font-weight: 750; letter-spacing: var(--tr-display); line-height: 1; }
.big__main small { margin: 0 10px 0 4px; font-size: 20px; font-weight: 650; letter-spacing: 0; color: var(--text-2); }
.big__sub { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; font-size: var(--fs-small); color: var(--text-2); }
.big__delta { display: inline-flex; align-items: center; gap: 2px; font-weight: 650; }
.big__delta.is-up { color: var(--success); }
.big__delta.is-down { color: var(--text-2); }
.parts { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; margin-top: auto; padding-top: 8px; }
.parts__col { display: flex; flex-direction: column; align-items: center; gap: 6px; }
.parts__bar { display: flex; align-items: flex-end; width: 100%; height: 56px; overflow: hidden; border-radius: var(--r-2); background: var(--fill-1); }
.parts__bar i { width: 100%; border-radius: var(--r-2); background: var(--fill-3); }
.parts__col.is-peak .parts__bar i { background: var(--accent); }
.parts__col.is-peak .parts__icon, .parts__col.is-peak .parts__label { color: var(--text-1); }
.parts__icon { font-size: 16px; color: var(--text-2); }
.parts__label { font-size: var(--fs-micro); color: var(--text-3); }

.card__title { margin: 0; font-size: var(--fs-small); font-weight: 650; color: var(--text-2); }
.tops { display: flex; flex-direction: column; gap: 2px; margin: 0; padding: 0; list-style: none; }
.top { display: grid; grid-template-columns: 16px 36px minmax(0, 1fr) auto; align-items: center; gap: 10px; padding: 6px 8px; margin: 0 -8px; border-radius: var(--r-3); color: inherit; text-decoration: none; }
.top:hover { background: var(--fill-1); }
.top:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.top__n { font-size: var(--fs-small); color: var(--text-3); text-align: center; }
.top__img { width: 36px; }
.top__body { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.top__name { font-size: var(--fs-body); font-weight: 600; }
.top__bar { height: 3px; overflow: hidden; border-radius: var(--r-pill); background: var(--fill-1); }
.top__bar i { display: block; height: 100%; border-radius: inherit; background: var(--text-2); }
.top__min { font-size: var(--fs-caption); color: var(--text-3); white-space: nowrap; }
.genres { display: flex; flex-direction: column; gap: 14px; margin: 0; padding: 0; list-style: none; }
.genre { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 6px 12px; }
.genre__name { font-size: var(--fs-body); font-weight: 600; }
.genre__pct { font-size: var(--fs-small); color: var(--text-2); }
.genre__bar { grid-column: 1 / -1; height: 6px; overflow: hidden; border-radius: var(--r-pill); background: var(--fill-1); }
.genre__bar i { display: block; height: 100%; border-radius: inherit; }

.blocked { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px 24px; }
.brow { display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; align-items: center; gap: 12px; min-height: 60px; padding: 0 4px; border-bottom: 1px solid var(--hair); }
.brow__img { width: 44px; }
.brow__body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.brow__name { font-size: var(--fs-body); font-weight: 600; }
.brow__meta { font-size: var(--fs-small); color: var(--text-3); }

.knows { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 12px; }
.kn { display: flex; flex-direction: column; gap: 4px; padding: 16px; border-radius: var(--r-4); background: var(--fill-1); }
.kn__n { font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.1; }
.kn__l { font-size: var(--fs-small); color: var(--text-3); }

.reset { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 20px 24px; border-radius: var(--r-5); box-shadow: inset 0 0 0 1px var(--line); transition: background var(--dur-3) var(--ease-out), box-shadow var(--dur-3) var(--ease-out); }
.reset.is-open { background: color-mix(in oklab, var(--danger) 8%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--danger) 40%, transparent); }
.reset__text { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.reset__title { margin: 0; font-size: var(--fs-h4); font-weight: 700; }
.reset__desc { margin: 0; font-size: var(--fs-small); color: var(--text-2); text-wrap: pretty; }
.reset__actions { display: flex; flex: none; gap: 8px; }

@container main (max-width: 980px) {
  .month { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .big { grid-column: 1 / -1; }
  .knows { grid-template-columns: repeat(3, minmax(0, 1fr)); }
}
@container main (max-width: 760px) {
  .facets, .blocked { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 699px) {
  .taste { gap: 36px; padding: 0 16px; }
  /* Верхняя строка — во всю ширину и вплотную к заголовку */
  .taste > .pn { margin: calc(var(--safe-top) * -1) -16px -24px; }
  .head__title { font-size: 34px; }
  .head__sub { font-size: 15px; }
  .facets, .month, .blocked { grid-template-columns: minmax(0, 1fr); }
  .facets { gap: 12px; }
  .big__main { font-size: 44px; }
  .knows { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .kn { padding: 14px; }
  .reset { flex-direction: column; align-items: stretch; gap: 16px; padding: 16px; }
  .reset__actions { justify-content: flex-end; }
  .reset__actions :deep(.btn) { min-height: 44px; }
}
</style>
