<script setup lang="ts">
import type { Recognition } from '~/api/types'

// «Распознанное» в поиске: последние узнанные песни. Нажатие — лист с «Слушать» и остальным,
// ✕ — убрать из истории. Пусто — блока нет
const props = withDefaults(defineProps<{ limit?: number }>(), { limit: 6 })

const rec = useRecognizeStore()
const toast = useToast()
const lang = useLang()
const { t } = useScopedI18n('components.recognize.History')

onMounted(() => void rec.loadHistory().catch(() => {}))
// Одну песню узнают и по нескольку раз — в списке она одна, последней датой
const keyOf = (r: Recognition) => r.track?.id ?? `${r.artist}\n${r.title}`.toLowerCase()
const list = computed(() => {
  const seen = new Set<string>()
  return rec.history.filter((r) => {
    const key = keyOf(r)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  }).slice(0, props.limit)
})

const day = (iso: string) => new Date(iso).toLocaleDateString(intlLocale(lang.value, 'en-US'), { day: 'numeric', month: 'long' })
function sub(r: Recognition) {
  const artist = r.track ? r.track.artists.map(a => a.name).join(', ') : r.artist
  return `${artist} · ${day(r.recognizedAt)}`
}
async function remove(r: Recognition) {
  try {
    // Убрать песню — значит все её распознавания, иначе под ней всплыла бы прошлая
    for (const x of rec.history.filter(x => keyOf(x) === keyOf(r))) await rec.remove(x.id)
  }
  catch {
    toast.error(t('removeFailed'))
  }
}
</script>

<template>
  <section v-if="list.length" class="rhist" :aria-label="t('title')">
    <header class="rhist__head">
      <h2 class="rhist__title">{{ t('title') }}</h2>
    </header>
    <div class="rhist__list">
      <SearchItemRow
        v-for="r in list"
        :key="r.id"
        :title="r.track?.title ?? r.title"
        :sub="sub(r)"
        :image="r.track?.album?.image ?? (r.artworkUrl ? { small: r.artworkUrl, medium: r.artworkUrl, large: r.artworkUrl } : null)"
        @open="rec.show(r)"
      >
        <template #end>
          <UiIconButton icon="ph:x" size="sm" :label="t('remove')" :tooltip="false" class="rhist__x" @click="remove(r)" />
        </template>
      </SearchItemRow>
    </div>
  </section>
</template>

<style scoped>
.rhist { display: flex; flex-direction: column; gap: 4px; }
.rhist__head { display: flex; align-items: center; min-height: 44px; }
.rhist__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.rhist__list { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 0 16px; margin: 0 -8px; }
.rhist .rhist__list :deep(.rhist__x) { color: var(--text-3); }
.rhist .rhist__list :deep(.rhist__x:hover) { color: var(--text-1); }
@media (max-width: 699px) {
  .rhist__title { font-size: 20px; }
  .rhist__list { grid-template-columns: minmax(0, 1fr); }
  .rhist .rhist__list :deep(.rhist__x) { --size: 44px; margin-right: -8px; }
}
</style>
