<script setup lang="ts">
import type { TransitionMode } from '~/stores/player'

// Чип режима переходов в панели плеера; по нажатию — панель выбора.
// На сайте «Диджей» работает как «Умный»: сведение в долю умеет только звуковое ядро приложений
withDefaults(defineProps<{
  /** Подпись следующего перехода: «2:26» */
  nextAt?: string | null
  /** Сайт: «Диджей» помечен «в приложении» */
  web?: boolean
  /** Только значок (узкая панель) */
  compact?: boolean
  /** Галерея: панель раскрыта без всплытия */
  inline?: boolean
  side?: 'top' | 'bottom'
}>(), { nextAt: null, web: true, side: 'top' })

const mode = defineModel<TransitionMode>('mode', { required: true })
const expressiveness = defineModel<number>('expressiveness', { default: 0.6 })
const open = ref(false)
const { t } = useI18n()

const ICONS: Record<TransitionMode, string> = {
  gapless: 'ph:link-simple-horizontal',
  smooth: 'ph:wave-sine',
  smart: 'ph:sparkle',
  dj: 'ph:vinyl-record',
}
</script>

<template>
  <div v-if="inline" class="pop pop--static" style="width: 340px">
    <header class="pop__head">
      <span class="pop__title">{{ t('transition.title') }}</span>
      <span class="pop__hint">{{ t('transition.hint') }}</span>
    </header>
    <MusicTransitionModeOptions v-model:mode="mode" v-model:expressiveness="expressiveness" :web="web" :next-at="nextAt" />
  </div>
  <UiPopover v-else v-model:open="open" :side="side" align="end" :title="t('transition.title')" :hint="t('transition.hint')" :width="340">
    <template #trigger>
      <button type="button" class="mode-chip" :class="{ 'mode-chip--compact': compact }" :aria-label="`${t('transition.title')}: ${t(`transition.${mode}`)}`">
        <Icon :name="ICONS[mode]" class="mode-chip__icon" />
        <span v-if="!compact" class="mode-chip__label">{{ t(`transition.${mode}`) }}</span>
      </button>
    </template>
    <MusicTransitionModeOptions v-model:mode="mode" v-model:expressiveness="expressiveness" :web="web" :next-at="nextAt" />
  </UiPopover>
</template>

<style>
.mode-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 10px 0 8px;
  flex: none;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  box-shadow: inset 0 0 0 1px var(--line);
  font-size: var(--fs-caption);
  font-weight: 650;
  color: var(--text-2);
  white-space: nowrap;
  transition: color var(--dur-1), background-color var(--dur-1), box-shadow var(--dur-1);
}
.mode-chip--compact { width: 32px; height: 32px; padding: 0; justify-content: center; }
.mode-chip__icon { font-size: 16px; color: var(--accent); }
.mode-chip:hover, .mode-chip[data-state='open'] { color: var(--text-1); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line-strong); }
</style>
