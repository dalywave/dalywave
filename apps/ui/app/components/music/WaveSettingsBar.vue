<script setup lang="ts">
import type { WaveSettings } from '~/bridge/types'
import type { MenuEntry } from '~/types/ui'
import { ACTIVITY_OPTIONS, CHARACTER_OPTIONS, LANGUAGE_OPTIONS } from '~/utils/wave-options'
import type { WaveOption } from '~/utils/wave-options'

// Настройки волны без «стены чипов»: на большом экране — док из ячеек со списками,
// на телефоне — шары настроения плитками и три токена
withDefaults(defineProps<{ variant?: 'dock' | 'tokens' }>(), { variant: 'dock' })

const model = defineModel<WaveSettings>({ required: true })
const { t } = useI18n()

type Group = 'activity' | 'character' | 'language'
const GROUPS: { key: Group, labelKey: string, options: WaveOption<string>[] }[] = [
  { key: 'activity', labelKey: 'wave.activity.label', options: ACTIVITY_OPTIONS },
  { key: 'character', labelKey: 'wave.character.label', options: CHARACTER_OPTIONS },
  { key: 'language', labelKey: 'wave.language.label', options: LANGUAGE_OPTIONS },
]

function valueLabel(group: Group, long = false) {
  const v = model.value[group]
  if (long && group === 'language' && v === 'any') return t('wave.language.anyLong')
  if (long && group === 'activity' && v === 'any') return `${t('wave.activity.label')}: ${t('wave.activity.any').toLowerCase()}`
  const o = GROUPS.find(g => g.key === group)!.options.find(x => x.value === v)
  return o ? t(o.key) : ''
}

function items(group: Group): MenuEntry[] {
  const g = GROUPS.find(x => x.key === group)!
  return [
    { kind: 'heading', label: t(g.labelKey) },
    ...g.options.map(o => ({
      label: t(o.key),
      icon: o.icon,
      hint: o.hintKey ? t(o.hintKey) : undefined,
      checked: model.value[group] === o.value,
      onSelect: () => (model.value = { ...model.value, [group]: o.value }),
    })),
  ]
}

const mood = computed({
  get: () => model.value.mood,
  set: v => (model.value = { ...model.value, mood: v }),
})
</script>

<template>
  <div v-if="variant === 'dock'" v-edge-fade class="dock" role="group" :aria-label="t('wave.title')">
    <div class="dock__cell">
      <span class="dock__label">{{ t('wave.mood.label') }}</span>
      <MusicMoodOrbs v-model="mood" />
    </div>
    <UiMenu v-for="g in GROUPS" :key="g.key" :items="items(g.key)" align="start" side="top">
      <button type="button" class="dock__cell dock__cell--btn">
        <span class="dock__label">{{ t(g.labelKey) }}</span>
        <span class="dock__value">{{ valueLabel(g.key) }}<Icon name="ph:caret-down" class="dock__caret" /></span>
      </button>
    </UiMenu>
  </div>
  <div v-else v-edge-fade class="tokens no-scrollbar">
    <UiMenu v-for="g in GROUPS" :key="g.key" :items="items(g.key)" align="start" side="bottom">
      <button type="button" class="token" :aria-label="t(g.labelKey)">
        {{ valueLabel(g.key, true) }}<Icon name="ph:caret-down" class="token__caret" />
      </button>
    </UiMenu>
  </div>
</template>

<style scoped>
.dock {
  display: inline-flex;
  align-items: stretch;
  max-width: 100%;
  overflow-x: auto;
  scrollbar-width: none;
  border-radius: var(--r-4);
  background: rgba(10, 12, 16, 0.55);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  backdrop-filter: blur(24px) saturate(1.4);
  box-shadow: inset 0 0 0 1px var(--line), var(--inner-hi);
}
.dock__cell {
  position: relative;
  display: flex;
  flex: none;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
  padding: 9px 16px 10px;
  border: 0;
  border-radius: var(--r-4);
  background: transparent;
  text-align: left;
  transition: background-color var(--dur-1);
}
.dock__cell + .dock__cell::before, .dock > * + *::before {
  content: '';
  position: absolute;
  left: 0;
  top: 12px;
  bottom: 12px;
  width: 1px;
  background: var(--line);
}
.dock__cell--btn:hover, .dock__cell--btn[data-state='open'] { background: var(--fill-1); }
.dock__label { font-size: 10.5px; font-weight: 600; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.dock__value { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-body); font-weight: 600; color: var(--text-1); white-space: nowrap; }
.dock__caret { font-size: 14px; color: var(--text-3); transition: transform var(--dur-2) var(--ease-out); }
.dock__cell--btn[data-state='open'] .dock__caret { transform: rotate(180deg); }

/* Узкая колонка (открыта правая панель): в ряд четыре ячейки не помещаются — док встаёт сеткой 2×2
   на всю ширину карточки, разделители — между колонками и между строками */
@container main (max-width: 700px) {
  .dock { display: grid; grid-template-columns: repeat(2, minmax(max-content, 1fr)); width: 100%; }
  .dock__cell { border-radius: 0; }
  .dock > :nth-child(odd)::before { display: none; }
  .dock > :nth-child(n + 3)::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: var(--line);
  }
  .dock > :nth-child(3)::after { left: 16px; }
  .dock > :nth-child(4)::after { right: 16px; }
}

.tokens { display: flex; gap: 8px; overflow-x: auto; margin: 0 -16px; padding: 0 16px; }
.token {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 44px;
  padding: 0 12px 0 14px;
  flex: none;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  font-size: var(--fs-small);
  font-weight: 600;
  white-space: nowrap;
  transition: background-color var(--dur-1), box-shadow var(--dur-1);
}
.token:hover, .token[data-state='open'] { background: var(--fill-2); box-shadow: inset 0 0 0 1px var(--line-strong); }
.token__caret { font-size: 14px; color: var(--text-3); }
</style>
