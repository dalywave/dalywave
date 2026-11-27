<script setup lang="ts">
import type { RewindEvent, RewindEventPatch, RewindMode, RewindStyle, RewindStyleSetting } from '~/api/types'

// «Итоги года» — событие сервера: в каком стиле показывать истории и когда они открыты. Пока итоги
// закрыты, слушатели их не видят вовсе; предпросмотр любого стиля — администратору всегда
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const ev = ref<RewindEvent | null>(null)
const failed = ref(false)
async function load() {
  failed.value = false
  try {
    ev.value = await bridge.admin.rewindEvent()
  }
  catch {
    failed.value = true
  }
}
onMounted(load)

async function patch(p: RewindEventPatch) {
  try {
    ev.value = await bridge.admin.updateRewindEvent(p)
    toast.show({ text: t('saved'), icon: 'ph:check' })
  }
  catch {
    toast.error(t('failed'))
  }
}

const STYLES: RewindStyle[] = ['wave', 'spread', 'record']
const styleOptions = computed(() => [...STYLES, 'random' as const].map(v => ({ value: v as RewindStyleSetting, label: t(`style.${v}`) })))
const modeOptions = computed(() => (['auto', 'open', 'closed'] as RewindMode[]).map(v => ({ value: v, label: t(`mode.${v}`) })))
const yearOptions = computed(() => (ev.value?.years ?? []).map(y => ({ value: y, label: String(y) })))

const date = (iso: string) => new Intl.DateTimeFormat(intlLocale(lang.value, 'en-GB'), { day: 'numeric', month: 'long' }).format(new Date(iso))
const status = computed(() => {
  const e = ev.value
  if (!e) return ''
  if (e.openYear != null && e.closesAt) return t('status.openUntil', { year: e.openYear, date: date(e.closesAt) })
  if (e.openYear != null) return t('status.open', { year: e.openYear })
  if (e.opensAt) return t('status.closedUntil', { date: date(e.opensAt) })
  return t('status.closed')
})
/** Год предпросмотра: открытый, выбранный для ручного запуска или последний */
const previewYear = computed(() => ev.value?.openYear ?? ev.value?.year ?? ev.value?.years[0] ?? new Date().getFullYear())
</script>

<template>
  <AdminState :loading="!ev && !failed" :error="failed && !ev" :rows="4" @retry="load">
    <SettingsGroup v-if="ev" :title="t('title')">
      <SettingsRow :title="t('when')" :description="status">
        <AdminSelect :model-value="ev.mode" :options="modeOptions" :label="t('when')" align="end" @update:model-value="v => patch({ mode: v })" />
      </SettingsRow>
      <SettingsRow v-if="ev.mode === 'open'" :title="t('year')">
        <AdminSelect :model-value="ev.year ?? previewYear" :options="yearOptions" :label="t('year')" align="end" @update:model-value="v => patch({ year: v })" />
      </SettingsRow>
      <SettingsRow :title="t('styleLabel')">
        <AdminSelect :model-value="ev.style" :options="styleOptions" :label="t('styleLabel')" align="end" @update:model-value="v => patch({ style: v })" />
      </SettingsRow>
      <SettingsRow :title="t('preview')" stack>
        <div class="rwe__previews">
          <UiButton v-for="s in STYLES" :key="s" variant="secondary" size="sm" icon="ph:play" :to="`/rewind?preview=1&style=${s}&year=${previewYear}`">{{ t(`style.${s}`) }}</UiButton>
        </div>
      </SettingsRow>
    </SettingsGroup>
  </AdminState>
</template>

<style scoped>
.rwe__previews { display: flex; flex-wrap: wrap; gap: 8px; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Итоги года",
    "when": "Когда открыты",
    "year": "За какой год",
    "styleLabel": "Стиль историй",
    "preview": "Предпросмотр",
    "style": { "wave": "Волна года", "spread": "Разворот", "record": "Пластинка", "random": "Случайный для каждого слушателя" },
    "mode": { "auto": "Автоматически", "open": "Открыто сейчас", "closed": "Закрыто" },
    "status": {
      "openUntil": "Открыты итоги {year} · закроются {date}",
      "open": "Открыты итоги {year}",
      "closedUntil": "Закрыты · откроются {date}",
      "closed": "Закрыты — слушатели их не видят"
    },
    "saved": "Сохранено",
    "failed": "Не получилось сохранить"
  },
  "uk": {
    "title": "Підсумки року",
    "when": "Коли відкриті",
    "year": "За який рік",
    "styleLabel": "Стиль історій",
    "preview": "Попередній перегляд",
    "style": { "wave": "Хвиля року", "spread": "Розворот", "record": "Платівка", "random": "Випадковий для кожного слухача" },
    "mode": { "auto": "Автоматично", "open": "Відкрито зараз", "closed": "Закрито" },
    "status": {
      "openUntil": "Відкрито підсумки {year} · закриються {date}",
      "open": "Відкрито підсумки {year}",
      "closedUntil": "Закрито · відкриються {date}",
      "closed": "Закрито — слухачі їх не бачать"
    },
    "saved": "Збережено",
    "failed": "Не вдалося зберегти"
  },
  "en": {
    "title": "Year in review",
    "when": "When it’s open",
    "year": "Year",
    "styleLabel": "Story style",
    "preview": "Preview",
    "style": { "wave": "Wave of the year", "spread": "Spread", "record": "Record", "random": "Random for each listener" },
    "mode": { "auto": "Automatically", "open": "Open now", "closed": "Closed" },
    "status": {
      "openUntil": "Your {year} is open · closes {date}",
      "open": "Your {year} is open",
      "closedUntil": "Closed · opens {date}",
      "closed": "Closed — listeners don’t see it"
    },
    "saved": "Saved",
    "failed": "Couldn’t save"
  }
}
</i18n>
