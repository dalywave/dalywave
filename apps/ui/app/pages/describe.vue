<script setup lang="ts">
import type { DescribeBrief, DescribePreview, Track } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// «Плейлист по описанию»: человек пишет словами («40 минут фонка для зала, без медляков»), сервер
// разбирает и собирает — здесь предпросмотр: название (можно поправить), треки, «Пересобрать»,
// «Ещё в том же духе», убрать трек, «Слушать» сразу или «Сохранить» обычным плейлистом.
// Описание живёт в адресе (?text; не ?q — его читает строка поиска): из поиска сюда приходят уже с ним, «назад» возвращает к сборке
const route = useRoute()
const router = useRouter()
const bridge = useBridge()
const player = usePlayerStore()
const library = useLibraryStore()
const toast = useToast()
const ui = useUiStore()
const lang = useLang()
const { t } = useScopedI18n('pages.describe')
const { tg } = useGlobalT()
useHead({ title: () => t('title') })

const EXAMPLES = ['example1', 'example2', 'example3'] as const

const text = ref(typeof route.query.text === 'string' ? route.query.text : '')
const result = shallowRef<DescribePreview | null>(null)
const tracks = ref<Track[]>([])
const title = ref('')
const state = ref<'idle' | 'loading' | 'ready' | 'error'>('idle')
const errorText = ref('')
const weekly = ref(false)
const saving = ref(false)
const moreLoading = ref(false)
let seq = 0

const fieldEl = ref<HTMLTextAreaElement | null>(null)
const canBuild = computed(() => text.value.trim().length >= 2 && state.value !== 'loading')

/** Собрать: заново или «Пересобрать» (без треков прошлой сборки) */
async function build(again = false) {
  const q = text.value.trim()
  if (q.length < 2) return
  const my = ++seq
  const exclude = again ? tracks.value.map(x => x.id) : []
  state.value = 'loading'
  fieldEl.value?.blur()
  if (route.query.text !== q) void router.replace({ query: { ...route.query, text: q } })
  try {
    const r = await bridge.describe.preview(q, exclude)
    if (my !== seq) return
    result.value = r
    tracks.value = r.items.map(i => i.track)
    title.value = r.title
    state.value = 'ready'
  }
  catch (e) {
    if (my !== seq) return
    errorText.value = (e as { status?: number }).status === 429 ? t('tooOften') : t('failed')
    state.value = 'error'
  }
}

function useExample(key: typeof EXAMPLES[number]) {
  text.value = t(key)
  void build()
}

function onKey(e: KeyboardEvent) {
  // Enter — собрать, Shift+Enter — перенос строки
  if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
    e.preventDefault()
    if (canBuild.value) void build()
  }
}

onMounted(() => {
  if (text.value.trim().length >= 2) void build()
  else if (!ui.isTouchUi) fieldEl.value?.focus()
})

// Название правится на месте: нажали — поле, Enter или уход — готово
const renaming = ref(false)
const nameEl = ref<HTMLInputElement | null>(null)
function rename() {
  renaming.value = true
  nextTick(() => {
    nameEl.value?.focus()
    nameEl.value?.select()
  })
}

const durationMs = computed(() => tracks.value.reduce((s, x) => s + x.durationMs, 0))
const covers = computed(() => {
  const seen = new Set<string>()
  const out = []
  for (const x of tracks.value) {
    const img = x.album?.image
    if (!img || seen.has(x.album!.id)) continue
    seen.add(x.album!.id)
    out.push(img)
    if (out.length === 4) break
  }
  return out
})
const ctx = computed(() => ({ type: 'search' as const, id: `describe:${result.value?.brief.text ?? ''}`, title: title.value || t('title') }))

function listen() {
  if (!tracks.value.length) return
  void player.playTracks(tracks.value, 0, ctx.value)
}

function remove(index: number) {
  const removed = tracks.value[index]
  if (!removed) return
  tracks.value = tracks.value.filter((_, i) => i !== index)
  toast.show({
    text: t('removed', { title: removed.title }),
    icon: 'ph:minus-circle',
    undo: () => {
      const next = [...tracks.value]
      next.splice(index, 0, removed)
      tracks.value = next
    },
  })
}
function removeMany(indices: number[]) {
  const drop = new Set(indices)
  tracks.value = tracks.value.filter((_, i) => !drop.has(i))
}
function extraMenu(_track: Track, index: number): MenuEntry[] {
  return [{ label: t('remove'), icon: 'ph:minus-circle', danger: true, onSelect: () => remove(index) }]
}

async function more() {
  const r = result.value
  if (!r || moreLoading.value) return
  moreLoading.value = true
  try {
    const shown = tracks.value.map(x => x.id)
    const res = await bridge.describe.more(r.brief as DescribeBrief, [...shown, ...r.items.map(i => i.track.id)], shown.at(-1) ?? null, 10)
    const have = new Set(shown)
    const added = res.items.map(i => i.track).filter(x => !have.has(x.id))
    if (!added.length) toast.show({ text: t('noMore'), icon: 'ph:info' })
    tracks.value = [...tracks.value, ...added]
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    moreLoading.value = false
  }
}

async function save() {
  const r = result.value
  if (!r || !tracks.value.length || saving.value) return
  saving.value = true
  const name = title.value.trim() || r.title
  try {
    const p = await bridge.describe.save({
      brief: r.brief,
      title: name,
      description: r.description || null,
      trackIds: tracks.value.map(x => x.id),
      weekly: weekly.value,
    })
    void library.loadPlaylists(true).catch(() => {})
    toast.show({ text: t('saved', { title: name }), icon: 'ph:playlist' })
    await navigateTo(`/playlist/${p.id}`)
  }
  catch {
    toast.error(t('saveFailed'))
  }
  finally {
    saving.value = false
  }
}

function reset() {
  ++seq
  state.value = 'idle'
  result.value = null
  tracks.value = []
  text.value = ''
  void router.replace({ query: { ...route.query, text: undefined } })
  nextTick(() => fieldEl.value?.focus())
}
</script>

<template>
  <div class="dsc">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="t('title')" fallback="/library" />
    <header class="dsc__head">
      <h1 class="dsc__title">{{ t('title') }}</h1>
    </header>

    <form class="ask" :class="{ 'ask--busy': state === 'loading' }" @submit.prevent="build()">
      <textarea
        ref="fieldEl"
        v-model="text"
        class="ask__field"
        :placeholder="t('placeholder')"
        :aria-label="t('title')"
        maxlength="300"
        rows="2"
        enterkeyhint="go"
        @keydown="onKey"
      />
      <div class="ask__bar">
        <button v-if="text && state !== 'loading'" type="button" class="ask__clear" :aria-label="t('clear')" @click="reset">
          <Icon name="ph:x-circle-fill" />
        </button>
        <UiButton type="submit" variant="primary" :size="ui.isPhone ? 'lg' : 'md'" :loading="state === 'loading'" :disabled="!canBuild">
          {{ t('build') }}
        </UiButton>
      </div>
    </form>

    <!-- Примеры: только пока ничего не собрано -->
    <div v-if="state === 'idle'" class="examples">
      <span class="examples__label">{{ t('try') }}</span>
      <button v-for="k in EXAMPLES" :key="k" type="button" class="examples__item" @click="useExample(k)">
        {{ t(k) }}
      </button>
    </div>

    <!-- Сборка: обложка и строки-заглушки -->
    <div v-if="state === 'loading'" class="wait" aria-busy="true" :aria-label="t('building')">
      <div class="res__head">
        <UiSkeleton class="wait__art" />
        <div class="wait__text">
          <UiSkeleton class="wait__l1" />
          <UiSkeleton class="wait__l2" />
          <p class="wait__note">{{ t('building') }}</p>
        </div>
      </div>
      <div v-for="n in 6" :key="n" class="wait__row">
        <UiSkeleton class="wait__cover" />
        <div class="wait__lines"><UiSkeleton class="wait__r1" /><UiSkeleton class="wait__r2" /></div>
      </div>
    </div>

    <UiEmptyState v-else-if="state === 'error'" icon="ph:cloud-warning" :title="errorText" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="build()">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>

    <template v-else-if="state === 'ready' && result">
      <UiEmptyState v-if="!tracks.length" icon="ph:music-notes" :title="t('empty')" :text="t('emptyText')" />
      <section v-else class="res" :aria-label="title">
        <div class="res__head">
          <div class="res__art">
            <MusicPlaylistCover id="describe" :title="title" :covers="covers" :show-title="false" />
          </div>
          <div class="res__text">
            <input v-if="renaming" ref="nameEl" v-model="title" class="res__name res__name--edit" maxlength="200" :aria-label="t('name')" enterkeyhint="done" @blur="renaming = false" @keydown.enter.prevent="renaming = false">
            <button v-else type="button" class="res__name" :aria-label="t('rename')" @click="rename">{{ title }}</button>
            <p v-if="result.description" class="res__desc">{{ result.description }}</p>
            <p class="res__meta">
              <span>{{ formatCount(tracks.length, 'track', lang) }}</span>
              <span>{{ formatLongDuration(durationMs, lang) }}</span>
            </p>
            <div class="res__actions">
              <UiButton variant="primary" size="lg" icon="ph:play-fill" @click="listen">{{ t('listen') }}</UiButton>
              <UiButton size="lg" icon="ph:check" :loading="saving" @click="save">{{ t('save') }}</UiButton>
              <UiIconButton icon="ph:arrows-clockwise" :label="t('rebuild')" size="lg" class="res__again" @click="build(true)" />
            </div>
          </div>
        </div>

        <div class="opt">
          <div class="opt__text">
            <span class="opt__label">{{ t('weekly') }}</span>
            <span class="opt__hint">{{ t('weeklyHint') }}</span>
          </div>
          <UiSwitch v-model="weekly" :label="t('weekly')" hide-label />
        </div>

        <MusicTrackList
          :tracks="tracks"
          variant="wide"
          :context="ctx"
          :label="title"
          :extra-menu="extraMenu"
          :remove-many="removeMany"
        />

        <UiButton class="res__more" size="lg" icon="ph:plus" :loading="moreLoading" @click="more">{{ t('more') }}</UiButton>
      </section>
    </template>
  </div>
</template>

<style scoped>
.dsc { display: flex; flex-direction: column; gap: 20px; max-width: 980px; padding-top: 16px; }
.dsc__head { display: flex; align-items: center; min-height: 44px; }
.dsc__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }

/* Поле описания: ровная заливка, кнопка — внутри справа снизу */
.ask { position: relative; display: flex; flex-direction: column; gap: 8px; padding: 14px 14px 10px 18px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); transition: box-shadow var(--dur-1); }
.ask:focus-within { box-shadow: inset 0 0 0 1.5px var(--focus); }
.ask__field { width: 100%; min-height: 52px; padding: 0; border: 0; background: transparent; color: var(--text-1); font: inherit; font-size: 18px; line-height: 1.45; resize: none; field-sizing: content; max-height: 160px; }
.ask__field::placeholder { color: var(--text-3); }
.ask__field:focus { outline: none; }
.ask__bar { display: flex; align-items: center; justify-content: flex-end; gap: 8px; }
.ask__clear { display: grid; place-items: center; width: 36px; height: 36px; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-3); font-size: 20px; transition: color var(--dur-1); }
.ask__clear:hover { color: var(--text-2); }
.ask__clear:focus-visible { outline: 2px solid var(--focus); outline-offset: 0; }

.examples { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.examples__label { margin-right: 4px; font-size: var(--fs-small); color: var(--text-3); }
.examples__item { position: relative; height: 34px; padding: 0 14px; border: 0; border-radius: var(--r-pill); background: var(--fill-2); color: var(--text-1); font: inherit; font-size: var(--fs-small); font-weight: 550; white-space: nowrap; transition: background-color var(--dur-1), transform var(--dur-1); }
.examples__item::before { content: ''; position: absolute; inset: -5px -2px; }
.examples__item:hover { background: var(--fill-3); }
.examples__item:active { transform: scale(0.97); }
.examples__item:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }

.res { display: flex; flex-direction: column; gap: 20px; }
.res__head { display: flex; align-items: flex-end; gap: 28px; padding-top: 8px; }
.res__art { width: 184px; flex: none; }
.res__art :deep(.pc) { width: 100%; border-radius: var(--r-4); box-shadow: var(--shadow-cover); }
.res__text { display: flex; flex: 1; flex-direction: column; gap: 6px; min-width: 0; }
.res__name { display: block; width: 100%; margin: 0 0 0 -6px; padding: 2px 6px; border: 0; border-radius: var(--r-2); background: transparent; color: var(--text-1); font: inherit; font-size: var(--fs-h2); font-weight: 750; line-height: 1.15; letter-spacing: var(--tr-title); text-align: left; text-wrap: balance; cursor: text; transition: background-color var(--dur-1); }
.res__name:hover { background: var(--fill-1); }
.res__name:focus-visible { outline: 2px solid var(--focus); outline-offset: 0; }
.res__name--edit:focus { outline: none; background: var(--fill-1); box-shadow: inset 0 0 0 1.5px var(--focus); }
.res__desc { margin: 0; font-size: var(--fs-body); color: var(--text-2); text-wrap: pretty; }
.res__meta { display: flex; gap: 6px; margin: 0; font-size: var(--fs-small); color: var(--text-2); }
.res__meta span + span::before { content: '·'; margin-right: 6px; }
.res__actions { display: flex; align-items: center; gap: 12px; margin-top: 10px; }
.res__more { align-self: flex-start; }

/* Подпись слева — переключатель справа, разделители-волоски */
.opt { display: flex; border-radius: 0; align-items: center; gap: 16px; min-height: 56px; padding: 6px 0; border-top: 1px solid var(--hair); border-bottom: 1px solid var(--hair); }
.opt__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.opt__label { font-size: var(--fs-body); font-weight: 550; }
.opt__hint { font-size: var(--fs-small); color: var(--text-2); }

.wait { display: flex; flex-direction: column; gap: 10px; }
.wait__art { width: 184px; aspect-ratio: 1; flex: none; border-radius: var(--r-4); }
.wait__text { display: flex; flex: 1; flex-direction: column; gap: 10px; }
.wait__l1 { width: 55%; height: 28px; border-radius: 6px; }
.wait__l2 { width: 30%; height: 14px; border-radius: 4px; }
.wait__note { margin: 6px 0 0; font-size: var(--fs-small); color: var(--text-2); }
.wait__row { display: flex; align-items: center; gap: 14px; min-height: 56px; }
.wait__row:first-of-type { margin-top: 18px; }
.wait__cover { width: 44px; height: 44px; border-radius: var(--r-2); }
.wait__lines { display: flex; flex: 1; flex-direction: column; gap: 6px; }
.wait__r1 { width: 36%; height: 14px; border-radius: 4px; }
.wait__r2 { width: 22%; height: 12px; border-radius: 4px; }

@media (max-width: 699px) {
  .dsc { gap: 16px; padding: 8px 16px 0; }
  .dsc__head { min-height: 0; }
  .dsc__title { font-size: 28px; }
  .ask { padding: 12px 12px 10px 16px; }
  .ask__field { font-size: 17px; }
  .examples { flex-direction: column; align-items: flex-start; gap: 10px; }
  .examples__label { margin: 4px 0 0; }
  .examples__item { max-width: 100%; height: auto; min-height: 40px; padding: 9px 16px; white-space: normal; text-align: left; line-height: 1.3; }
  .res__head { flex-direction: column; align-items: stretch; gap: 16px; }
  .res__art, .wait__art { width: 56%; align-self: center; }
  .res__text { align-items: stretch; }
  .res__name { margin: 0; text-align: center; font-size: 24px; }
  .res__desc, .res__meta { justify-content: center; text-align: center; }
  .res__actions { gap: 10px; }
  .res__actions > :deep(.btn) { flex: 1; }
  .res__more { align-self: stretch; }
  .res > :deep(.tl) { margin: 0 -8px; }
  .wait .res__head { flex-direction: column; }
  .wait__text { align-items: center; }
}
</style>
