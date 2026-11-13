<script setup lang="ts">
import type { AdminRecording, AdminUser, Job } from '~/api/types'

// Поиск по админке (⌘K / Ctrl+K): разделы, вкладки настроек, люди, записи каталога и упавшие
// задачи. Стрелки — выбор, Enter — перейти. Люди и задачи грузятся при первом открытии, записи —
// поиском по мере ввода
export interface CommandSection { to: string, label: string, icon: string }
const props = defineProps<{ sections: CommandSection[] }>()
const open = defineModel<boolean>('open', { default: false })
const bridge = useBridge()
const L = useAdminLabels()
const { t } = useI18n({ useScope: 'local' })

interface Item { key: string, group: string, label: string, hint?: string, icon: string, to: string }

const q = ref('')
const active = ref(0)
const people = ref<AdminUser[] | null>(null)
const jobs = ref<Job[] | null>(null)
const records = ref<AdminRecording[]>([])
watch(open, async (v) => {
  if (!v) return
  q.value = ''
  active.value = 0
  if (!people.value) bridge.admin.users().then(l => (people.value = l.items)).catch(() => {})
  if (!jobs.value) bridge.admin.jobs({ status: 'failed', limit: 50 }).then(p => (jobs.value = p.items)).catch(() => {})
})
let seq = 0
watchDebounced(q, async (v) => {
  const my = ++seq
  const text = v.trim()
  if (text.length < 2) {
    records.value = []
    return
  }
  try {
    const page = await bridge.admin.recordings({ q: text, limit: 5 })
    if (my === seq) records.value = page.items
  }
  catch {
    if (my === seq) records.value = []
  }
}, { debounce: 250 })

const SETTINGS = ['general', 'signin', 'music', 'storage', 'clips', 'reco', 'notifications', 'integrations']
const norm = (s: string | null | undefined) => (s ?? '').toLowerCase().replace(/ё/g, 'е')
const has = (needle: string, ...hay: (string | null | undefined)[]) => hay.some(h => norm(h).includes(needle))

const items = computed<Item[]>(() => {
  const needle = norm(q.value.trim())
  const out: Item[] = []
  for (const s of props.sections) {
    if (!needle || has(needle, s.label)) out.push({ key: `s:${s.to}`, group: t('group.sections'), label: s.label, icon: s.icon, to: s.to })
  }
  for (const k of SETTINGS) {
    const label = t(`settings.${k}`)
    if (needle && has(needle, label, t('settingsWord'))) out.push({ key: `t:${k}`, group: t('group.settings'), label, hint: t('settingsWord'), icon: 'ph:sliders-horizontal', to: k === 'general' ? '/admin/settings' : `/admin/settings?tab=${k}` })
  }
  if (needle) {
    for (const u of people.value ?? []) {
      if (has(needle, u.name, u.email, u.username)) out.push({ key: `u:${u.id}`, group: t('group.people'), label: u.name, hint: u.email ?? u.username ?? undefined, icon: 'ph:user', to: `/admin/users/${u.id}` })
    }
    for (const r of records.value) {
      const title = r.track?.title ?? r.isrc ?? r.id
      out.push({ key: `r:${r.id}`, group: t('group.tracks'), label: title, hint: r.track ? artistNames(r.track.artists) : undefined, icon: 'ph:music-note', to: `/admin/catalog?q=${encodeURIComponent(r.track?.title ?? q.value.trim())}` })
    }
    for (const j of jobs.value ?? []) {
      if (has(needle, j.id, j.kind, L.jobKind(j.kind), j.lastError)) out.push({ key: `j:${j.id}`, group: t('group.jobs'), label: L.jobKind(j.kind), hint: j.lastError ?? undefined, icon: 'ph:warning-circle', to: '/admin/downloads?status=failed' })
    }
  }
  return out.slice(0, 40)
})
watch(items, () => (active.value = 0))

function go(i: Item | undefined) {
  if (!i) return
  open.value = false
  void navigateTo(i.to)
}
const list = ref<HTMLElement | null>(null)
function move(d: number) {
  const n = items.value.length
  if (!n) return
  active.value = (active.value + d + n) % n
  void nextTick(() => list.value?.querySelector<HTMLElement>('.is-active')?.scrollIntoView({ block: 'nearest' }))
}
</script>

<template>
  <UiModal v-model:open="open" :title="t('title')" :show-title="false" :width="560" tall>
    <div class="cmd">
      <div class="cmd__field">
        <Icon name="ph:magnifying-glass" />
        <input
          v-model="q"
          class="cmd__input"
          type="search"
          autofocus
          :placeholder="t('placeholder')"
          :aria-label="t('title')"
          role="combobox"
          aria-expanded="true"
          aria-controls="cmd-list"
          :aria-activedescendant="items[active] ? `cmd-${active}` : undefined"
          @keydown.down.prevent="move(1)"
          @keydown.up.prevent="move(-1)"
          @keydown.enter.prevent="go(items[active])"
        >
      </div>
      <ul id="cmd-list" ref="list" class="cmd__list thin-scrollbar" role="listbox">
        <template v-for="(i, n) in items" :key="i.key">
          <li v-if="!n || items[n - 1]!.group !== i.group" class="cmd__group" role="presentation">{{ i.group }}</li>
          <li
            :id="`cmd-${n}`"
            class="cmd__item"
            :class="{ 'is-active': n === active }"
            role="option"
            :aria-selected="n === active"
            @mousemove="active = n"
            @click="go(i)"
          >
            <Icon :name="i.icon" />
            <span class="cmd__label">{{ i.label }}</span>
            <span v-if="i.hint" class="cmd__hint">{{ i.hint }}</span>
          </li>
        </template>
        <li v-if="!items.length" class="cmd__empty" role="presentation">{{ t('empty') }}</li>
      </ul>
    </div>
  </UiModal>
</template>

<style scoped>
.cmd { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.cmd__field { display: flex; align-items: center; gap: 10px; margin: 4px 16px 8px; padding: 0 14px; height: 48px; border-radius: var(--r-3); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-3); }
.cmd__field:focus-within { box-shadow: inset 0 0 0 2px var(--focus); }
.cmd__field :deep(.iconify) { flex: none; font-size: 18px; }
.cmd__input { flex: 1; min-width: 0; height: 100%; border: 0; background: none; color: var(--text-1); font: inherit; font-size: var(--fs-body); }
.cmd__input:focus { outline: none; }
.cmd__input::placeholder { color: var(--text-4); }
.cmd__list { flex: 1; min-height: 0; margin: 0; padding: 0 8px 12px; overflow-y: auto; list-style: none; }
.cmd__group { padding: 12px 10px 4px; font-size: 11px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.cmd__item { display: flex; align-items: center; gap: 12px; min-height: 44px; padding: 0 10px; border-radius: var(--r-2); cursor: pointer; }
.cmd__item :deep(.iconify) { flex: none; font-size: 18px; color: var(--text-3); }
.cmd__item.is-active { background: var(--fill-2); }
.cmd__item.is-active :deep(.iconify) { color: var(--text-1); }
.cmd__label { flex: none; max-width: 60%; overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.cmd__hint { flex: 1; min-width: 0; overflow: hidden; font-size: var(--fs-small); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
.cmd__empty { padding: 24px 10px; font-size: var(--fs-small); color: var(--text-3); text-align: center; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Поиск по админке",
    "placeholder": "Раздел, человек, трек, задача или настройка",
    "empty": "Ничего не нашлось",
    "settingsWord": "Настройки",
    "group": { "sections": "Разделы", "settings": "Настройки", "people": "Люди", "tracks": "Записи", "jobs": "Упавшие задачи" },
    "settings": { "general": "Общее, часовой пояс", "signin": "Вход", "music": "Музыка: подписки, скорость, расписание", "storage": "Хранилище", "clips": "Клипы: качество, место", "reco": "Рекомендации", "notifications": "Уведомления", "integrations": "Интеграции" }
  },
  "en": {
    "title": "Search the admin",
    "placeholder": "Section, person, track, job or setting",
    "empty": "Nothing found",
    "settingsWord": "Settings",
    "group": { "sections": "Sections", "settings": "Settings", "people": "People", "tracks": "Recordings", "jobs": "Failed jobs" },
    "settings": { "general": "General, time zone", "signin": "Sign-in", "music": "Music: follows, speed, schedule", "storage": "Storage", "clips": "Clips: quality, space", "reco": "Recommendations", "notifications": "Notifications", "integrations": "Integrations" }
  }
}
</i18n>
