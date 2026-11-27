<script setup lang="ts">
import type { WavePreset } from '~/api/types'
import type { WaveSettings } from '~/bridge/types'
import type { MenuEntry } from '~/types/ui'
import { MOOD_ORB, fromApiSettings, presetIcon, presetMeta, sameSettings, suggestIcon, suggestPresetName } from '~/utils/wave-options'

// Сохранённые сочетания запускают волну в одно нажатие; последняя карточка сохраняет текущие настройки.
// Правый клик или долгое нажатие по сочетанию — переименовать или удалить
const MAX = 24

const player = usePlayerStore()
const presets = useWavePresetsStore()
const toast = useToast()
const lang = useLang()
const { t } = useScopedI18n('components.music.SavedCombos')

onMounted(() => void presets.load().catch(() => {}))

const list = computed(() => presets.items.map(p => ({ p, set: fromApiSettings(p.settings) })))
const activeId = computed(() => list.value.find(x => sameSettings(x.set, player.wave))?.p.id ?? null)

const cards = new Map<string, HTMLElement>()
const saveCard = ref<HTMLElement | null>(null)
function cardRef(id: string, el: unknown) {
  if (el instanceof HTMLElement) cards.set(id, el)
  else cards.delete(id)
}

const editorOpen = ref(false)
const busy = ref(false)
const anchor = ref<HTMLElement | null>(null)
const editing = ref<{ id: string | null, name: string, icon: string }>({ id: null, name: '', icon: 'ph:sparkle' })

function current(): WaveSettings {
  const w = player.wave
  return { mood: w.mood, activity: w.activity, character: w.character, language: w.language }
}

function openSave() {
  const set = current()
  editing.value = { id: null, name: suggestPresetName(set, lang.value), icon: suggestIcon(set) }
  anchor.value = saveCard.value
  editorOpen.value = true
}

function openEdit(p: WavePreset) {
  editing.value = { id: p.id, name: p.name, icon: presetIcon(p.icon) }
  anchor.value = cards.get(p.id) ?? null
  editorOpen.value = true
}

async function submit(v: { name: string, icon: string }) {
  const icon = v.icon.replace(/^ph:/, '')
  busy.value = true
  try {
    const id = editing.value.id
    if (id) {
      await presets.update(id, { name: v.name, icon })
    }
    else {
      await presets.create({ name: v.name, icon, settings: current() })
      toast.show({ text: t('savedToast', { name: v.name }), icon: 'ph:bookmark-simple' })
    }
    editorOpen.value = false
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}

async function remove(p: WavePreset) {
  try {
    const undo = await presets.remove(p.id)
    toast.show({
      text: t('removed', { name: p.name }),
      icon: 'ph:trash',
      undo: () => void undo().catch(() => toast.error(t('failed'))),
    })
  }
  catch {
    toast.error(t('failed'))
  }
}

function menu(p: WavePreset, set: WaveSettings): MenuEntry[] {
  return [
    { kind: 'heading', label: p.name, hint: presetMeta(set, lang.value) },
    { label: t('rename'), icon: 'ph:pencil-simple', onSelect: () => openEdit(p) },
    { label: t('remove'), icon: 'ph:trash', danger: true, onSelect: () => void remove(p) },
  ]
}

const swatch = (set: WaveSettings) => {
  const [p1, p2, p3] = MOOD_ORB[set.mood]
  return { '--p1': p1, '--p2': p2, '--p3': p3 }
}
</script>

<template>
  <div v-edge-fade class="combos no-scrollbar" role="group" :aria-label="t('label')">
    <UiContextMenu v-for="{ p, set } in list" :key="p.id" :items="menu(p, set)">
      <button
        :ref="el => cardRef(p.id, el)"
        type="button"
        class="combo"
        :class="{ 'is-active': activeId === p.id }"
        :aria-pressed="activeId === p.id"
        @click="player.applyPreset(set)"
      >
        <span class="combo__swatch" :style="swatch(set)">
          <Icon :name="presetIcon(p.icon)" />
        </span>
        <span class="combo__text">
          <span class="combo__title">{{ p.name }}<span v-if="activeId === p.id" class="combo__now"> · {{ t('now') }}</span></span>
          <span class="combo__meta">{{ presetMeta(set, lang) }}</span>
        </span>
        <Icon name="ph:play-fill" class="combo__go" />
      </button>
    </UiContextMenu>
    <button v-if="presets.items.length < MAX" ref="saveCard" type="button" class="combo combo--save" @click="openSave">
      <span class="combo__swatch combo__swatch--save"><Icon name="ph:plus" /></span>
      <span class="combo__text">
        <span class="combo__title">{{ t('save') }}</span>
        <span class="combo__meta">{{ t('saveMeta') }}</span>
      </span>
    </button>
  </div>
  <MusicPresetEditor
    v-model:open="editorOpen"
    :title="editing.id ? t('editTitle') : t('saveTitle')"
    :submit-label="editing.id ? t('done') : t('save')"
    :name="editing.name"
    :icon="editing.icon"
    :anchor="anchor"
    :busy="busy"
    @submit="submit"
  />
</template>

<style scoped>
/* Карточки делят ряд поровну, но не уже своих двух строк: где места мало (открыта правая панель),
   ряд прокручивается с затуханием у края, а подписи не режутся */
.combos { display: flex; gap: 12px; overflow-x: auto; overscroll-behavior-x: contain; }
.combo {
  display: flex;
  flex: 1 1 0;
  align-items: center;
  gap: 12px;
  min-width: max-content;
  padding: 10px 14px 10px 10px;
  border: 0;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  text-align: left;
  -webkit-touch-callout: none;
  user-select: none;
  transition: background-color var(--dur-1), box-shadow var(--dur-1);
}
.combo:hover { background: var(--fill-2); box-shadow: inset 0 0 0 1px var(--line); }
.combo:active { background: var(--fill-3); }
.combo:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.combo.is-active { box-shadow: inset 0 0 0 1px var(--accent-line); }
.combo__swatch {
  position: relative;
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex: none;
  border-radius: 10px;
  overflow: hidden;
  color: rgba(255, 255, 255, 0.92);
  background: color-mix(in oklab, var(--p2) 55%, var(--p1));
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
  font-size: 16px;
}
/* «Сохранить» — без цвета: это действие, а не сочетание */
.combo--save { flex: 0 1 auto; background: transparent; }
.combo--save:hover { background: var(--fill-1); }
.combo__swatch--save { background: var(--fill-2); box-shadow: none; color: var(--text-2); }
.combo__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.combo__title { font-size: var(--fs-body); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.combo__now { color: var(--accent); font-weight: 500; }
.combo__meta { font-size: var(--fs-caption); color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.combo__go { font-size: 16px; color: var(--text-3); opacity: 0; transform: translateX(-4px); transition: opacity var(--dur-1), transform var(--dur-2) var(--ease-out); }
.combo:hover .combo__go { opacity: 1; transform: none; }

@media (max-width: 699px) {
  .combos { gap: 8px; margin: 0 -16px; padding: 0 16px; }
  .combo { min-height: 56px; }
  .combo__go { display: none; }
}
</style>
