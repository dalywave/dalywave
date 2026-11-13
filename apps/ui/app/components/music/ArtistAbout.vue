<script setup lang="ts">
import type { ArtistAbout } from '~/api/types'

// «Об артисте» на странице артиста: начало статьи Википедии и факты (годы на сцене, откуда, жанры),
// «Читать дальше» — сама статья. Статьи нет или источник молчит — блока нет
const props = defineProps<{ artistId: string }>()
const bridge = useBridge()
const { t } = useI18n({ useScope: 'local' })

const about = shallowRef<ArtistAbout | null>(null)
async function load() {
  about.value = null
  try {
    about.value = (await bridge.catalog.about(props.artistId)).about ?? null
  }
  catch {
    about.value = null
  }
}
onMounted(load)
watch(() => props.artistId, () => void load())

const facts = computed(() => {
  const a = about.value
  if (!a) return []
  const out: { label: string, value: string }[] = []
  if (a.active) {
    const value = a.active.to ? `${a.active.from}–${a.active.to}` : t('since', { year: a.active.from })
    out.push({ label: t('active'), value })
  }
  if (a.origin) out.push({ label: t('from'), value: a.origin })
  if (a.genres.length) out.push({ label: t('genres'), value: a.genres.join(', ') })
  return out
})
</script>

<template>
  <section v-if="about" class="ab" :aria-label="t('title')">
    <h2 class="ab__title">{{ t('title') }}</h2>
    <p class="ab__text" :lang="about.lang">{{ about.summary }}</p>
    <a :href="about.url" target="_blank" rel="noopener" class="ab__more">{{ t('more') }}</a>
    <dl v-if="facts.length" class="ab__facts">
      <div v-for="f in facts" :key="f.label" class="ab__row">
        <dt>{{ f.label }}</dt>
        <dd>{{ f.value }}</dd>
      </div>
    </dl>
  </section>
</template>

<style scoped>
.ab { display: flex; flex-direction: column; gap: 10px; max-width: 680px; }
.ab__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.ab__text { margin: 0; color: var(--text-2); line-height: 1.55; display: -webkit-box; -webkit-line-clamp: 5; -webkit-box-orient: vertical; overflow: hidden; }
.ab__more { align-self: flex-start; min-height: 32px; display: inline-flex; align-items: center; font-weight: 600; color: var(--text-1); }
.ab__more:hover { text-decoration: underline; text-underline-offset: 3px; }
.ab__facts { margin: 4px 0 0; }
.ab__row { display: flex; align-items: baseline; justify-content: space-between; gap: 24px; min-height: 44px; padding: 12px 0; box-shadow: inset 0 -0.5px 0 var(--hair); }
.ab__row:first-child { box-shadow: inset 0 0.5px 0 var(--hair), inset 0 -0.5px 0 var(--hair); }
.ab__row:last-child { box-shadow: none; }
.ab__row:only-child { box-shadow: inset 0 0.5px 0 var(--hair); }
.ab__row dt { flex: none; color: var(--text-2); }
.ab__row dd { min-width: 0; margin: 0; text-align: right; }
.ab__row dd::first-letter { text-transform: uppercase; }
@media (max-width: 699px) {
  .ab { padding: 0 16px; }
  .ab__title { font-size: 20px; }
  .ab__more { min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Об артисте",
    "more": "Читать дальше",
    "active": "На сцене",
    "since": "с {year}",
    "from": "Откуда",
    "genres": "Жанры"
  },
  "en": {
    "title": "About",
    "more": "Read more",
    "active": "Active",
    "since": "since {year}",
    "from": "From",
    "genres": "Genres"
  }
}
</i18n>
