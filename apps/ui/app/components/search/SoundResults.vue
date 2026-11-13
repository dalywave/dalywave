<script setup lang="ts">
import type { SoundSearchResults, Track } from '~/api/types'

// «Похоже по звучанию»: запрос словами («спокойный инструментал для работы») → ближайшее по звучанию из
// медиатеки и из индекса каталога. `compact` — блок под обычными результатами (запрос похож на описание),
// иначе — весь экран фильтра «По звучанию». «Включить волну» — бесконечная волна по этому описанию
const props = withDefaults(defineProps<{
  text: string
  compact?: boolean
  /** Ссылка «Все» в компактном блоке */
  allLink?: string
}>(), { compact: false })

const bridge = useBridge()
const player = usePlayerStore()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const data = shallowRef<SoundSearchResults | null>(null)
const state = ref<'loading' | 'ready' | 'error'>('loading')
let seq = 0
async function run() {
  const my = ++seq
  state.value = 'loading'
  try {
    const r = await bridge.catalog.soundSearch(props.text, props.compact ? 8 : 24)
    if (my !== seq) return
    data.value = r
    state.value = 'ready'
  }
  catch {
    if (my === seq) state.value = 'error'
  }
}
watch(() => props.text, () => void run(), { immediate: true })

/** Компактно — медиатека и каталог вперемешку, без повторов */
const mixed = computed<Track[]>(() => {
  const r = data.value
  if (!r) return []
  const out: Track[] = []
  const seen = new Set<string>()
  for (let i = 0; i < Math.max(r.library.length, r.catalog.length); i++) {
    for (const tr of [r.catalog[i], r.library[i]]) {
      if (tr && !seen.has(tr.id)) {
        seen.add(tr.id)
        out.push(tr)
      }
    }
  }
  return out.slice(0, 6)
})
const total = computed(() => (data.value ? data.value.library.length + data.value.catalog.length : 0))
const ctx = computed(() => ({ type: 'search' as const, id: props.text, title: t('ctx', { q: props.text }) }))

function startWave() {
  void player.startWave(undefined, { seed: { type: 'text', text: props.text }, title: t('ctx', { q: props.text }) })
}
</script>

<template>
  <!-- Компактно: блок появляется, только если есть что показать -->
  <section v-if="compact && state === 'ready' && total" class="snd snd--compact" :aria-label="t('title')">
    <header class="snd__head">
      <div class="snd__titles">
        <h2 class="snd__title"><Icon name="ph:waveform" class="snd__icon" />{{ t('title') }}</h2>
      </div>
      <div class="snd__actions">
        <UiButton size="sm" variant="secondary" icon="ph:play-fill" @click="startWave">{{ ui.isPhone ? t('waveShort') : t('wave') }}</UiButton>
        <UiButton v-if="allLink && total > mixed.length && !ui.isPhone" :to="allLink" variant="ghost" size="sm">{{ tg('common.all') }}</UiButton>
      </div>
    </header>
    <MusicTrackList :tracks="mixed" variant="compact" plain :context="ctx" :label="t('title')" class="snd__grid" />
  </section>

  <div v-else-if="!compact" class="snd">
    <div class="snd__hero">
      <div class="snd__hero-text">
        <span class="snd__kicker"><Icon name="ph:waveform" />{{ t('kicker') }}</span>
        <h2 class="snd__query">«{{ text }}»</h2>
        <p v-if="data && data.unknownWords.length" class="snd__note">{{ t('unknown', { words: data.unknownWords.join(', ') }) }}</p>
      </div>
      <UiButton variant="primary" icon="ph:play-fill" :disabled="state === 'error'" @click="startWave">{{ t('wave') }}</UiButton>
    </div>

    <div v-if="state === 'loading'" class="snd__skel" aria-busy="true">
      <UiSkeleton v-for="n in 6" :key="n" variant="line" height="44px" />
    </div>
    <UiEmptyState v-else-if="state === 'error'" icon="ph:waveform" :title="t('down')" :text="t('downText')" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="run">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>
    <UiEmptyState v-else-if="!total" compact icon="ph:waveform" :title="t('none')" :text="t('noneText')" />
    <template v-else-if="data">
      <section v-if="data.catalog.length" class="snd__block" :aria-label="t('catalog')">
        <h3 class="snd__sub">{{ t('catalog') }}</h3>
        <MusicTrackList :tracks="data.catalog" variant="wide" plain hook :context="ctx" :label="t('catalog')" />
      </section>
      <section v-if="data.library.length" class="snd__block" :aria-label="t('library')">
        <h3 class="snd__sub">{{ t('library') }}</h3>
        <MusicTrackList :tracks="data.library" variant="wide" plain :context="ctx" :label="t('library')" />
      </section>
    </template>
  </div>
</template>

<style scoped>
.snd { display: flex; flex-direction: column; gap: 28px; min-width: 0; }
.snd__head { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 36px; margin-bottom: 4px; }
.snd__titles { min-width: 0; }
.snd__title { display: flex; align-items: center; gap: 8px; margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.2; }
.snd__icon { font-size: 20px; color: var(--accent); }
.snd__actions { display: flex; flex: none; align-items: center; gap: 4px; }
.snd--compact { gap: 0; }
.snd__grid :deep(.tl__rows), .snd__grid { min-width: 0; }

.snd__hero {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 24px;
}
.snd__hero-text { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.snd__kicker { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-2); }
.snd__query { margin: 0; overflow-wrap: anywhere; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; text-wrap: balance; }
.snd__note { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.snd__block { display: flex; flex-direction: column; gap: 4px; }
.snd__sub { margin: 0; font-size: var(--fs-h4); font-weight: 700; }
.snd__skel { display: flex; flex-direction: column; gap: 12px; }

@media (max-width: 699px) {
  .snd--compact { padding: 0 8px; }
  .snd__head { padding: 0 8px; }
  .snd__title { font-size: 20px; }
  .snd__actions :deep(.btn) { min-height: 40px; }
  .snd { padding: 0 16px; }
  .snd--compact { padding: 0 8px; }
  .snd__hero { flex-direction: column; align-items: stretch; gap: 16px; padding-top: 8px; }
  .snd__query { font-size: 24px; }
  .snd__block { margin: 0 -10px; }
  .snd__sub { padding: 0 10px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Похоже по звучанию",
    "kicker": "По звучанию",
    "wave": "Включить волну по запросу",
    "waveShort": "Волна",
    "ctx": "По запросу «{q}»",
    "catalog": "Новое для вас",
    "library": "Из медиатеки",
    "unknown": "Не поняли слова: {words}",
    "down": "Поиск по звучанию недоступен",
    "downText": "Попробуйте чуть позже — обычный поиск работает",
    "none": "Ничего похожего",
    "noneText": "Опишите иначе: настроение, инструменты, темп"
  },
  "en": {
    "title": "Sounds like this",
    "kicker": "By sound",
    "wave": "Start a wave from this",
    "waveShort": "Wave",
    "ctx": "From “{q}”",
    "catalog": "New for you",
    "library": "From your library",
    "unknown": "Didn't understand: {words}",
    "down": "Sound search is unavailable",
    "downText": "Try again a bit later — regular search works",
    "none": "Nothing similar",
    "noneText": "Try describing the mood, instruments or tempo"
  }
}
</i18n>
