<script setup lang="ts">
import type { CreditPerson, CreditRole, TrackCredits } from '~/api/types'

// «Кто сделал трек» на странице трека: авторы музыки и слов, продюсеры, звукорежиссёры, лейбл, год.
// Подпись слева — люди справа. Имя ведёт к артисту, только если его страница точно известна
const props = defineProps<{ trackId: string }>()
const bridge = useBridge()
const { t } = useI18n({ useScope: 'local' })

const data = shallowRef<TrackCredits | null>(null)
const loading = ref(true)
let timer = 0
let tries = 0

async function load() {
  try {
    data.value = await bridge.catalog.credits(props.trackId)
  }
  catch {
    data.value = null
  }
  finally {
    loading.value = false
  }
  // Ссылки на страницы людей ищутся в фоне: заглянуть ещё пару раз
  window.clearTimeout(timer)
  if (data.value?.resolving && tries++ < 3) timer = window.setTimeout(load, 5000)
}
onMounted(load)
watch(() => props.trackId, () => {
  tries = 0
  loading.value = true
  data.value = null
  void load()
})
onBeforeUnmount(() => window.clearTimeout(timer))

const ORDER: CreditRole[] = ['writer', 'composer', 'lyricist', 'producer', 'arranger', 'programming', 'recording', 'engineer', 'mix', 'mastering']

/** Строки: роль → люди с этой ролью, по порядку важности ролей */
const rows = computed(() => {
  const d = data.value
  if (!d) return []
  // Автор и музыки, и слов — одной строкой «Музыка и слова», а не в трёх
  const writing = d.writing.map((p): CreditPerson => {
    const both = p.roles.includes('writer') || (p.roles.includes('composer') && p.roles.includes('lyricist'))
    return both ? { ...p, roles: ['writer'] } : p
  })
  const all: CreditPerson[] = [...writing, ...d.production]
  return ORDER.map((role) => {
    const people = all.filter(p => p.roles.includes(role))
    return { role, people }
  }).filter(r => r.people.length)
})
const empty = computed(() => !loading.value && !rows.value.length && !data.value?.label && !data.value?.year)
</script>

<template>
  <section v-if="!empty" class="cr" :aria-label="t('title')">
    <h2 class="cr__title">{{ t('title') }}</h2>
    <div v-if="loading" class="cr__skel" aria-hidden="true">
      <div v-for="i in 3" :key="i" class="cr__row">
        <UiSkeleton variant="line" width="96px" />
        <UiSkeleton variant="line" width="140px" />
      </div>
    </div>
    <dl v-else class="cr__list">
      <div v-for="r in rows" :key="r.role" class="cr__row">
        <dt>{{ t(`role.${r.role}`, r.people.length) }}</dt>
        <dd>
          <template v-for="(p, i) in r.people" :key="p.id">
            <span v-if="i" class="cr__sep">, </span>
            <NuxtLink v-if="p.artistId" :to="`/artist/${p.artistId}`" class="cr__link">{{ p.name }}</NuxtLink>
            <span v-else>{{ p.name }}</span>
          </template>
        </dd>
      </div>
      <div v-if="data?.label" class="cr__row">
        <dt>{{ t('label') }}</dt>
        <dd>{{ data.label }}</dd>
      </div>
      <div v-if="data?.year" class="cr__row">
        <dt>{{ t('year') }}</dt>
        <dd class="tnum">{{ data.year }}</dd>
      </div>
    </dl>
  </section>
</template>

<style scoped>
.cr { display: flex; flex-direction: column; gap: 4px; max-width: 640px; }
.cr__title { margin: 0 0 4px; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.cr__list, .cr__skel { margin: 0; }
.cr__row { display: flex; align-items: baseline; justify-content: space-between; gap: 24px; min-height: 44px; padding: 12px 0; box-shadow: inset 0 -0.5px 0 var(--hair); }
.cr__row:last-child { box-shadow: none; }
.cr__row dt { flex: none; color: var(--text-2); }
.cr__row dd { min-width: 0; margin: 0; text-align: right; }
.cr__skel .cr__row { align-items: center; }
.cr__link { text-decoration: underline; text-decoration-color: var(--text-4); text-underline-offset: 3px; }
.cr__link:hover { text-decoration-color: currentColor; }
@media (max-width: 699px) {
  .cr { padding: 0 16px; }
  .cr__title { font-size: 20px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Кто сделал трек",
    "label": "Лейбл",
    "year": "Год",
    "role": {
      "writer": "Музыка и слова",
      "composer": "Музыка",
      "lyricist": "Слова",
      "producer": "Продюсер | Продюсеры",
      "arranger": "Аранжировка",
      "programming": "Программирование",
      "recording": "Запись",
      "engineer": "Звукорежиссёр | Звукорежиссёры",
      "mix": "Сведение",
      "mastering": "Мастеринг"
    }
  },
  "en": {
    "title": "Credits",
    "label": "Label",
    "year": "Year",
    "role": {
      "writer": "Songwriting",
      "composer": "Music",
      "lyricist": "Lyrics",
      "producer": "Producer | Producers",
      "arranger": "Arrangement",
      "programming": "Programming",
      "recording": "Recording",
      "engineer": "Engineer | Engineers",
      "mix": "Mixing",
      "mastering": "Mastering"
    }
  }
}
</i18n>
