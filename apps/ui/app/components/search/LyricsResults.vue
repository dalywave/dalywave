<script setup lang="ts">
import type { LyricsHit } from '~/api/types'

// «По словам песни»: запрос похож на строчку (четыре слова и больше или в кавычках) — песни, где она
// есть, и сама строчка с найденным выделенным. Ничего не нашлось — блока нет
const props = withDefaults(defineProps<{ text: string, limit?: number }>(), { limit: 6 })

const bridge = useBridge()
const player = usePlayerStore()
const { t } = useI18n({ useScope: 'local' })

/** Строчка без кавычек вокруг */
const line = computed(() => props.text.trim().replace(/^["«“„']+|["»”“']+$/g, '').trim())
const hits = ref<LyricsHit[]>([])
let seq = 0
watch(line, async (q) => {
  const my = ++seq
  if (q.length < 3) {
    hits.value = []
    return
  }
  try {
    const r = await bridge.catalog.searchLyrics(q, props.limit)
    if (my === seq) hits.value = r.items
  }
  catch {
    if (my === seq) hits.value = []
  }
}, { immediate: true })

// Совпадение без регистра и диакритики, «ё» = «е» — как ищет сервер
const fold = (c: string) => c.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/ё/g, 'е')
function parts(text: string, needle: string): [string, string, string] {
  let folded = ''
  const at: number[] = []
  for (let i = 0; i < text.length; i++) {
    for (const ch of fold(text[i]!)) {
      folded += ch
      at.push(i)
    }
  }
  const n = [...needle].map(fold).join('')
  const i = n ? folded.indexOf(n) : -1
  if (i < 0) return [text, '', '']
  const s = at[i]!
  const e = at[i + n.length - 1]! + 1
  // Длинная строка: начало срезается, чтобы найденное было видно
  const lead = s > 40 ? `…${text.slice(s - 24, s).replace(/^\S*\s/, '')}` : text.slice(0, s)
  return [lead, text.slice(s, e), text.slice(e)]
}

const ctx = computed(() => ({ type: 'search' as const, id: line.value, title: t('title') }))
function play(i: number) {
  const tr = hits.value[i]?.track
  if (!tr) return
  if (player.current?.track.id === tr.id) return player.toggle()
  return player.playTracks(hits.value.map(h => h.track), i, ctx.value)
}
const artists = (h: LyricsHit) => h.track.artists.map(a => a.name).join(', ')
const rows = computed(() => hits.value.map(h => ({ hit: h, parts: parts(h.line, line.value) })))
</script>

<template>
  <section v-if="hits.length" class="lyr" :aria-label="t('title')">
    <header class="lyr__head"><h2 class="lyr__title">{{ t('title') }}</h2></header>
    <div class="lyr__list">
      <button v-for="({ hit: h, parts: p }, i) in rows" :key="h.track.id" type="button" class="lyr__row" :class="{ 'is-current': player.current?.track.id === h.track.id }" :aria-label="`${h.track.title} — ${artists(h)}`" @click="play(i)">
        <MusicCoverArt :image="h.track.album?.image" size="small" :alt="h.track.album?.title ?? ''" radius="6px" class="lyr__art" />
        <span class="lyr__main">
          <span class="lyr__line">{{ p[0] }}<mark class="lyr__mark">{{ p[1] }}</mark>{{ p[2] }}</span>
          <span class="lyr__sub ellipsis">{{ h.track.title }} · {{ artists(h) }}</span>
        </span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.lyr { display: flex; flex-direction: column; gap: 4px; }
.lyr__head { display: flex; align-items: center; min-height: 36px; margin-bottom: 4px; }
.lyr__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.2; }
.lyr__list { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 0 16px; margin: 0 -8px; }
.lyr__row { display: grid; grid-template-columns: 40px minmax(0, 1fr); align-items: center; gap: 12px; min-height: 64px; padding: 8px; border: 0; border-radius: var(--r-3); background: none; color: inherit; font: inherit; text-align: left; transition: background-color var(--dur-1); }
.lyr__row:hover { background: var(--fill-1); }
.lyr__row:active { background: var(--fill-2); }
.lyr__row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.lyr__main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.lyr__line { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; font-weight: 550; line-height: 1.35; }
.lyr__mark { background: none; color: var(--accent); }
.lyr__row.is-current .lyr__sub { color: var(--accent); }
.lyr__sub { font-size: var(--fs-small); color: var(--text-2); }
@media (max-width: 699px) {
  .lyr { padding: 0 8px; }
  .lyr__head { padding: 0 8px; margin-bottom: 0; }
  .lyr__title { font-size: 20px; }
  .lyr__list { grid-template-columns: minmax(0, 1fr); margin: 0; }
  .lyr__row { grid-template-columns: 48px minmax(0, 1fr); }
}
</style>

<i18n lang="json">
{
  "ru": { "title": "По словам песни" },
  "en": { "title": "From the lyrics" }
}
</i18n>
