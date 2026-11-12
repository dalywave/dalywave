<script setup lang="ts">
import type { PlaylistDetail, Track } from '~/api/types'

// «Добавить треки» внутри своего плейлиста: поиск, а пока запроса нет — «Подходит к плейлисту»
// (похожее по звучанию на треки плейлиста) или, если он пуст, любимые. «+» добавляет сразу
const props = defineProps<{ detail: PlaylistDetail }>()
const emit = defineEmits<{ added: [] }>()
const open = defineModel<boolean>('open', { default: false })

const bridge = useBridge()
const toast = useToast()
const player = usePlayerStore()
const { t } = useI18n()
const { tg } = useGlobalT()

const q = ref('')
const results = ref<Track[] | null>(null)
const searching = ref(false)
const recs = ref<Track[] | null>(null)
const recsTitle = ref('')
/** id треков, которые уже в плейлисте или добавлены сейчас */
const inside = ref(new Set<string>())
const busy = ref(new Set<string>())
let addedAny = false

watch(open, (v) => {
  if (v) {
    q.value = ''
    results.value = null
    inside.value = new Set(props.detail.items.map(i => i.track.id))
    addedAny = false
    void loadRecs()
  }
  else if (addedAny) emit('added')
})

/** Похожее на несколько треков плейлиста — из разных его мест, чтобы не крутиться вокруг одного */
async function loadRecs() {
  recs.value = null
  const items = props.detail.items
  try {
    if (!items.length) {
      recsTitle.value = t('pl.fromLiked')
      const page = await bridge.library.liked(null, 30)
      recs.value = page.items.map(i => i.track)
      return
    }
    recsTitle.value = t('pl.fits')
    const n = Math.min(5, items.length)
    const seeds = Array.from({ length: n }, (_, k) => items[Math.floor((k * items.length) / n)]!.track.id)
    const lists = await Promise.all(seeds.map(id => bridge.catalog.similar(id, 10).then(r => r.items.map(i => i.track)).catch(() => [] as Track[])))
    // По одному из каждого списка по кругу: разнообразнее, чем подряд
    // Одна песня бывает в нескольких релизах — сравниваем ещё и по названию с артистом
    const sig = (tr: Track) => `${tr.title.toLocaleLowerCase()}|${tr.artists[0]?.id ?? ''}`
    const seen = new Set(inside.value)
    const seenSig = new Set(items.map(i => sig(i.track)))
    const out: Track[] = []
    for (let k = 0; k < 10; k++) {
      for (const l of lists) {
        const tr = l[k]
        if (tr && !seen.has(tr.id) && !seenSig.has(sig(tr))) {
          seen.add(tr.id)
          seenSig.add(sig(tr))
          out.push(tr)
        }
      }
    }
    recs.value = out.slice(0, 30)
  }
  catch {
    recs.value = []
  }
}

let seq = 0
const runSearch = useDebounceFn(async (text: string) => {
  const my = ++seq
  if (!text) {
    results.value = null
    searching.value = false
    return
  }
  try {
    const r = await bridge.catalog.search(text, 'catalog', 30)
    if (my === seq) results.value = r.tracks
  }
  catch {
    if (my === seq) results.value = []
  }
  finally {
    if (my === seq) searching.value = false
  }
}, 250)
watch(q, (v) => {
  searching.value = !!v.trim()
  void runSearch(v.trim())
})

const shown = computed(() => (q.value.trim() ? results.value : recs.value))

async function add(tr: Track) {
  if (inside.value.has(tr.id) || busy.value.has(tr.id)) return
  busy.value.add(tr.id)
  try {
    await bridge.playlists.addItems(props.detail.playlist.id, { trackIds: [tr.id] })
    inside.value.add(tr.id)
    addedAny = true
    bridge.haptic('light')
  }
  catch {
    toast.error(t('pl.addFailed'))
  }
  finally {
    busy.value.delete(tr.id)
  }
}

function preview(tr: Track) {
  if (player.current?.track.id === tr.id) void player.toggle()
  else void player.playTracks([tr], 0, { type: 'playlist', id: props.detail.playlist.id, title: props.detail.playlist.title })
}
</script>

<template>
  <UiModal v-model:open="open" :title="t('pl.addTracks')" detents="expand" tall :width="520">
    <div class="at">
      <UiSearchField v-model="q" :placeholder="t('pl.searchTracks')" autofocus class="at__search" />
      <h3 v-if="!q.trim() && recsTitle" class="at__h">{{ recsTitle }}</h3>
      <div v-if="shown === null || (searching && !results)" class="at__list" aria-hidden="true">
        <div v-for="n in 6" :key="n" class="at__row at__row--skel"><UiSkeleton width="44px" height="44px" /><span class="at__text"><UiSkeleton variant="line" width="60%" /><UiSkeleton variant="line" width="35%" /></span></div>
      </div>
      <div v-else-if="shown.length" class="at__list">
        <div v-for="tr in shown" :key="tr.id" class="at__row" :class="{ 'is-current': player.current?.track.id === tr.id }">
          <button type="button" class="at__main" :aria-label="`${tg('common.play')}: ${tr.title}`" @click="preview(tr)">
            <MusicCoverArt :image="tr.album?.image" size="small" :alt="''" radius="6px" class="at__art" />
            <span class="at__text">
              <span class="at__name ellipsis">{{ tr.title }}</span>
              <span class="at__sub ellipsis">{{ tr.artists.map(a => a.name).join(', ') }}</span>
            </span>
          </button>
          <button
            type="button"
            class="at__add"
            :class="{ 'is-in': inside.has(tr.id) }"
            :disabled="inside.has(tr.id) || busy.has(tr.id)"
            :aria-label="inside.has(tr.id) ? t('pl.inPlaylist') : `${t('pl.addTrack')}: ${tr.title}`"
            @click="add(tr)"
          >
            <span v-if="busy.has(tr.id)" class="at__spin" aria-hidden="true" />
            <Icon v-else :name="inside.has(tr.id) ? 'ph:check-circle-fill' : 'ph:plus-circle'" />
          </button>
        </div>
      </div>
      <p v-else class="at__none">{{ t('pl.noMatch') }}</p>
    </div>
  </UiModal>
</template>

<style scoped>
.at { display: flex; flex-direction: column; padding: 0 12px 16px; }
.at__search { margin: 0 8px 8px; }
.at__h { margin: 12px 8px 6px; font-size: var(--fs-small); font-weight: 650; color: var(--text-2); }
.at__list { display: flex; flex-direction: column; }
.at__row { display: flex; align-items: center; gap: 4px; border-radius: var(--r-3); transition: background-color var(--dur-1); }
.at__row:hover { background: var(--fill-1); }
.at__row--skel { gap: 12px; min-height: 60px; padding: 8px; }
.at__main { display: flex; flex: 1; align-items: center; gap: 12px; min-width: 0; min-height: 60px; padding: 8px; border: 0; background: none; color: var(--text-1); font: inherit; text-align: left; cursor: pointer; }
.at__main:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); border-radius: var(--r-3); }
.at__art { width: 44px; height: 44px; flex: none; }
.at__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.at__name { font-weight: 550; }
.at__row.is-current .at__name { color: var(--accent); }
.at__sub { font-size: var(--fs-small); color: var(--text-2); }
.at__add { display: grid; place-items: center; width: 44px; height: 44px; flex: none; margin-right: 4px; padding: 0; border: 0; border-radius: 50%; background: none; color: var(--text-1); font-size: 26px; cursor: pointer; transition: color var(--dur-1), transform var(--dur-2) var(--ease-out); }
.at__add:hover:not(:disabled) { transform: scale(1.08); }
.at__add:active:not(:disabled) { transform: scale(0.94); }
.at__add:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.at__add.is-in { color: var(--accent); cursor: default; }
.at__spin { width: 18px; height: 18px; border: 2px solid var(--fill-3); border-top-color: var(--text-1); border-radius: 50%; animation: spin 0.8s linear infinite; }
.at__none { margin: 24px 8px; color: var(--text-3); text-align: center; }
@media (max-width: 699px) {
  .at { padding: 0 8px 12px; }
}
</style>
