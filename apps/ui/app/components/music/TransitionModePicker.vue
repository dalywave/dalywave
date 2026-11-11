<script setup lang="ts">
import type { TransitionMode } from '~/stores/player'

// Чип режима переходов в панели плеера; по нажатию — панель выбора: у мыши — всплывающая,
// на телефоне и под пальцем — лист снизу, как меню «⋯» (повторное нажатие мимо листа закрывает).
// На сайте «Диджей» работает как «Умный»: сведение в долю умеет только звуковое ядро приложений
withDefaults(defineProps<{
  /** Подпись следующего перехода: «2:26» */
  nextAt?: string | null
  /** Сайт: «Диджей» помечен «в приложении» */
  web?: boolean
  /** Только значок (узкая панель) */
  compact?: boolean
  /** Круглая кнопка-значок, как соседние в нижнем ряду плеера на телефоне; режим — значком и точкой */
  icon?: boolean
  /** Галерея: панель раскрыта без всплытия */
  inline?: boolean
  side?: 'top' | 'bottom'
}>(), { nextAt: null, web: true, side: 'top' })

const mode = defineModel<TransitionMode>('mode', { required: true })
const expressiveness = defineModel<number>('expressiveness', { default: 0.6 })
const open = ref(false)
const sheet = useMenuSheet()
const { t } = useI18n()
</script>

<template>
  <div v-if="inline" class="pop pop--static" style="width: 340px">
    <header class="pop__head">
      <span class="pop__title">{{ t('transition.title') }}</span>
      <span class="pop__hint">{{ t('transition.hint') }}</span>
    </header>
    <MusicTransitionModeOptions v-model:mode="mode" v-model:expressiveness="expressiveness" :web="web" :next-at="nextAt" />
  </div>
  <UiSheet v-else-if="sheet" v-model:open="open" :title="t('transition.title')" :description="t('transition.hint')" :show-title="false" inset>
    <template #trigger>
      <MusicTransitionModeTrigger :mode="mode" :icon="icon" :compact="compact" />
    </template>
    <template #head>
      <header class="mode-sheet__head">
        <span class="mode-sheet__title">{{ t('transition.title') }}</span>
        <span class="mode-sheet__hint">{{ t('transition.hint') }}</span>
      </header>
    </template>
    <div class="mode-sheet">
      <MusicTransitionModeOptions v-model:mode="mode" v-model:expressiveness="expressiveness" :web="web" :next-at="nextAt" />
    </div>
  </UiSheet>
  <UiPopover v-else v-model:open="open" :side="side" align="end" :title="t('transition.title')" :hint="t('transition.hint')" :width="340">
    <template #trigger>
      <MusicTransitionModeTrigger :mode="mode" :icon="icon" :compact="compact" />
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
.mode-ib {
  position: relative;
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 44px;
  height: 44px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-1), transform var(--dur-1) var(--ease-out);
}
.mode-ib__icon { font-size: 24px; }
.mode-ib__dot { position: absolute; top: 9px; right: 8px; width: 6px; height: 6px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 2px var(--ink-0); }
.mode-ib:hover, .mode-ib[data-state='open'] { color: var(--text-1); background: var(--fill-2); }
.mode-ib:active { transform: scale(0.92); background: var(--fill-3); }
.mode-ib:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.mode-chip:hover, .mode-chip[data-state='open'] { color: var(--text-1); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line-strong); }
.mode-sheet__head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; padding: 12px 20px 8px; flex: none; }
.mode-sheet__title { font-size: 17px; font-weight: 700; color: var(--text-1); }
.mode-sheet__hint { font-size: var(--fs-caption); color: var(--text-3); }
.mode-sheet { padding: 0 8px 10px; }
.mode-sheet .opt { min-height: 60px; padding: 12px; }
.mode-sheet .pop__foot { padding: 10px 12px 4px; }
</style>
