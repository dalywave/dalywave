<script setup lang="ts">
import type { Track } from '~/api/types'
import type { OfflineReserve } from '~/bridge'
import { formatBytes, formatCount } from '~/utils/format'

// «Запас для волны» (только приложение): что умный запас держит на устройстве, чтобы волна играла
// без сети. В «Скачанном» его нет — там только то, что скачано вручную. Треки в порядке загрузки:
// то, что ещё качается, показывает обычное кольцо хода в строке
const bridge = useBridge()
const offline = useOfflineStore()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

/** Сколько id сервер принимает за раз */
const LOOKUP = 500

const reserve = ref<OfflineReserve | null>(null)
const tracks = ref<Track[]>([])
const state = ref<'loading' | 'ready' | 'error'>('loading')

const enabled = computed(() => offline.status?.settings.reserveEnabled ?? true)

async function load() {
  const off = bridge.offline
  if (!off) return
  if (!reserve.value) state.value = 'loading'
  try {
    // Состояние файлов нужно до фильтра: на прямом входе оно могло ещё не прийти
    if (!offline.tracks.size) await offline.refresh()
    const r = await off.reserve()
    // Только то, что на устройстве или в пути: не поместившееся в предел здесь не показываем
    const ids = r.trackIds.filter((id) => {
      const s = offline.trackState(id)?.state
      return s === 'ready' || s === 'downloading' || s === 'queued'
    })
    const found: Track[] = []
    for (let i = 0; i < ids.length; i += LOOKUP) found.push(...await bridge.catalog.lookup(ids.slice(i, i + LOOKUP)))
    const byId = new Map(found.map(tr => [tr.id, tr]))
    reserve.value = r
    tracks.value = ids.map(id => byId.get(id)).filter((tr): tr is Track => !!tr)
    state.value = 'ready'
  }
  catch {
    if (!reserve.value) state.value = 'error'
  }
}

onMounted(() => {
  if (!bridge.offline) {
    void navigateTo('/library', { replace: true })
    return
  }
  offline.start()
  void load()
})
// Запас пересобрался или включён — перечитать состав
watch(() => [offline.status?.reserve.tracks, offline.status?.reserve.builtAt, enabled.value], (a, b) => {
  if (a.some((v, i) => v !== b[i])) void load()
})

const readyCount = computed(() => offline.status?.reserve.ready ?? reserve.value?.readyIds.length ?? 0)
const summary = computed(() => {
  const bytes = offline.status?.usage.reserveBytes ?? 0
  return `${formatCount(readyCount.value, 'track', lang.value)} · ${formatBytes(bytes, lang.value)}`
})
const ctx = computed(() => ({ type: 'library' as const, id: 'reserve', title: t('title') }))

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
</script>

<template>
  <div class="rsv">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="t('title')" :condensed="condensed" fallback="/library" />

    <header class="head">
      <h1 ref="titleEl" class="head__title">{{ t('title') }}</h1>
      <template v-if="enabled && state === 'ready' && tracks.length">
        <p class="head__meta tnum">{{ summary }}</p>
        <p class="head__sub">{{ t('sub') }}</p>
      </template>
    </header>

    <template v-if="!enabled">
      <UiEmptyState icon="ph:waveform" :title="t('off')" :text="t('offText')" />
      <SettingsGroup class="switch">
        <SettingsRow :title="t('title')">
          <UiSwitch :model-value="false" :label="t('title')" hide-label @update:model-value="v => offline.setSettings({ reserveEnabled: v })" />
        </SettingsRow>
      </SettingsGroup>
    </template>
    <MusicPageState v-else-if="state === 'error'" state="error" @retry="load" />
    <div v-else-if="state === 'loading'" class="wait" aria-busy="true" />
    <MusicTrackList v-else-if="tracks.length" :tracks="tracks" variant="wide" :context="ctx" :label="t('title')" />
    <UiEmptyState v-else icon="ph:waveform" :title="t('empty')" :text="t('emptyText')" />
  </div>
</template>

<style scoped>
.rsv { display: flex; flex-direction: column; gap: 24px; padding-top: 16px; }
.head { display: flex; flex-direction: column; gap: 6px; }
.head__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.head__meta { margin: 0; font-size: var(--fs-body); font-weight: 550; color: var(--text-1); }
.head__sub { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.tnum { font-variant-numeric: tabular-nums; }
.wait { min-height: 240px; }
.switch { width: min(480px, 100%); align-self: center; }

@media (max-width: 699px) {
  .rsv { gap: 20px; padding: 0 16px; }
  /* Верхняя строка — во всю ширину и вплотную к заголовку */
  .rsv > .pn { margin: calc(var(--safe-top) * -1) -16px -12px; }
  .head__title { font-size: 30px; }
  .rsv > :deep(.tl) { margin: 0 -8px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Запас для волны",
    "sub": "Обновляется сам, пока есть сеть",
    "off": "Запас выключен",
    "offText": "Любимое вперемешку с новым — чтобы волна играла и без сети",
    "empty": "Пока пусто",
    "emptyText": "Треки появятся, когда будет сеть"
  },
  "en": {
    "title": "Wave reserve",
    "sub": "Refreshes itself while you're online",
    "off": "Reserve is off",
    "offText": "Favourites mixed with new music — so the wave plays offline",
    "empty": "Nothing here yet",
    "emptyText": "Tracks will appear once you're online"
  }
}
</i18n>
