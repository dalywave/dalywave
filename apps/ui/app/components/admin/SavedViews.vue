<script setup lang="ts">
// Виды таблицы: готовые (заданы страницей) и свои — нынешние фильтры, сохранённые под именем.
// Общий для таблиц админки. Свои виды хранятся в этом браузере (`storageKey`), выбранный вид
// подсвечен, если фильтры с ним совпадают. Страница отдаёт нынешнее состояние фильтров (`state`)
// и применяет выбранное (`apply`)
export type ViewState = Record<string, string | boolean | number | null | undefined>
export interface ViewPreset { key: string, label: string, state: ViewState }

const props = defineProps<{
  storageKey: string
  presets: ViewPreset[]
  state: ViewState
}>()
const emit = defineEmits<{ apply: [ViewState] }>()
const { t } = useI18n({ useScope: 'local' })
const pui = usePlaylistUi()

interface Saved { name: string, state: ViewState }
const saved = ref<Saved[]>([])
const KEY = `dw-admin-views:${props.storageKey}`
onMounted(() => {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? '[]')
    if (Array.isArray(raw)) saved.value = raw.filter(v => v && typeof v.name === 'string' && v.state)
  }
  catch {
    // Хранилище недоступно — только готовые виды
  }
})
function persist() {
  try {
    localStorage.setItem(KEY, JSON.stringify(saved.value))
  }
  catch {
    // Приватное окно: вид проживёт до перезагрузки
  }
}
// Пустые значения не различаются: '' и null — «не задано»
const clean = (s: ViewState) => Object.fromEntries(Object.entries(s).filter(([, v]) => v !== '' && v !== null && v !== undefined && v !== false))
const same = (a: ViewState, b: ViewState) => JSON.stringify(Object.entries(clean(a)).sort()) === JSON.stringify(Object.entries(clean(b)).sort())
const isPreset = computed(() => props.presets.some(p => same(p.state, props.state)))
const isSaved = computed(() => saved.value.some(v => same(v.state, props.state)))

async function save() {
  const name = await pui.prompt({ title: t('saveTitle'), placeholder: t('name'), action: t('save'), maxLength: 40 })
  if (!name) return
  saved.value = [...saved.value.filter(v => v.name !== name), { name, state: { ...props.state } }]
  persist()
}
function remove(v: Saved) {
  saved.value = saved.value.filter(x => x !== v)
  persist()
}
</script>

<template>
  <nav class="ap-chips sv" :aria-label="t('views')">
    <button
      v-for="p in presets"
      :key="p.key"
      type="button"
      class="ap-chip"
      :class="{ 'is-active': same(p.state, state) }"
      :aria-pressed="same(p.state, state)"
      @click="emit('apply', p.state)"
    >
      {{ p.label }}
    </button>
    <span v-for="v in saved" :key="v.name" class="ap-chip sv__own" :class="{ 'is-active': same(v.state, state) }">
      <button type="button" class="sv__name" :aria-pressed="same(v.state, state)" @click="emit('apply', v.state)">
        <Icon name="ph:bookmark-simple" />{{ v.name }}
      </button>
      <button type="button" class="sv__x" :aria-label="t('remove', { name: v.name })" @click="remove(v)"><Icon name="ph:x" /></button>
    </span>
    <button v-if="!isPreset && !isSaved" type="button" class="ap-chip sv__add" @click="save">
      <Icon name="ph:plus" />{{ t('save') }}
    </button>
  </nav>
</template>

<style scoped>
.sv__own { gap: 2px; padding: 0 4px 0 12px; }
.sv__name, .sv__x { display: inline-flex; align-items: center; gap: 6px; height: 100%; padding: 0; border: 0; background: none; color: inherit; font: inherit; cursor: pointer; }
.sv__name:focus-visible, .sv__x:focus-visible { outline: none; border-radius: 4px; box-shadow: 0 0 0 2px var(--focus); }
.sv__x { display: grid; place-items: center; width: 24px; border-radius: 50%; opacity: 0.6; }
.sv__x:hover { opacity: 1; }
.sv__add { color: var(--text-3); box-shadow: inset 0 0 0 1px var(--line); }
</style>

<i18n lang="json">
{
  "ru": { "views": "Виды", "save": "Сохранить вид", "saveTitle": "Сохранить вид", "name": "Название", "remove": "Убрать вид «{name}»" },
  "en": { "views": "Views", "save": "Save view", "saveTitle": "Save view", "name": "Name", "remove": "Remove view “{name}”" }
}
</i18n>
