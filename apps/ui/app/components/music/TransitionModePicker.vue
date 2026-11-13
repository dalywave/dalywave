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
  /** Круглая кнопка-значок, как соседние в нижнем ряду плеера на телефоне; режим — значком */
  icon?: boolean
  /** Размер значка: sm — нижняя панель большого экрана (с подсказкой), lg — плеер на телефоне */
  size?: 'sm' | 'lg'
  /** Значок с подписью режима (нижняя панель компьютера) */
  labeled?: boolean
  /** Галерея: панель раскрыта без всплытия */
  inline?: boolean
  side?: 'top' | 'bottom'
}>(), { nextAt: null, web: true, side: 'top', size: 'lg' })

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
    </header>
    <MusicTransitionModeOptions v-model:mode="mode" v-model:expressiveness="expressiveness" :web="web" :next-at="nextAt" />
  </div>
  <UiSheet v-else-if="sheet" v-model:open="open" :title="t('transition.title')" :show-title="false" inset>
    <template #trigger>
      <MusicTransitionModeTrigger :mode="mode" :icon="icon" :size="size" :compact="compact" :labeled="labeled" />
    </template>
    <template #head>
      <header class="mode-sheet__head">
        <span class="mode-sheet__title">{{ t('transition.title') }}</span>
      </header>
    </template>
    <div class="mode-sheet">
      <MusicTransitionModeOptions v-model:mode="mode" v-model:expressiveness="expressiveness" :web="web" :next-at="nextAt" />
    </div>
  </UiSheet>
  <UiPopover v-else v-model:open="open" :side="side" align="end" :title="t('transition.title')" :width="340">
    <template #trigger>
      <MusicTransitionModeTrigger :mode="mode" :icon="icon" :size="size" :compact="compact" :labeled="labeled" tooltip />
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
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex: none;
  width: 44px;
  height: 44px;
  border: 0;
  padding: 0;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-1), transform var(--dur-1) var(--ease-out);
}
.mode-ib__glyph { position: relative; display: grid; place-items: center; }
.mode-ib__icon { font-size: 24px; }
/* Метка режима в углу значка: что сейчас выбрано, не меняя сам значок */
.mode-ib__mode {
  position: absolute;
  right: -8px;
  bottom: -6px;
  display: grid;
  place-items: center;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--accent);
  /* Зазор цвета фона отделяет метку от значка */
  box-shadow: 0 0 0 2px var(--bg);
  color: var(--bg);
  font-size: 9px;
}
/* Как .ib--sm у соседних кнопок нижней панели: 32 px, значок 18 px */
.mode-ib--sm { width: 32px; height: 32px; }
.mode-ib--sm .mode-ib__icon { font-size: 18px; }
.mode-ib--sm .mode-ib__mode { right: -6px; bottom: -5px; width: 11px; height: 11px; font-size: 7px; box-shadow: 0 0 0 1.5px var(--bg); }
/* С подписью режима: метка не нужна */
.mode-ib--labeled { width: auto; padding: 0 11px 0 8px; }
.mode-ib--labeled .mode-ib__mode { display: none; }
.mode-ib__label { font-size: var(--fs-caption); font-weight: 650; white-space: nowrap; }
@media (max-width: 1279px) {
  .mode-ib--labeled { width: 32px; padding: 0; }
  .mode-ib--labeled .mode-ib__label { display: none; }
  .mode-ib--labeled .mode-ib__mode { display: grid; }
}
/* data-state у кнопки делят подсказка и панель — открытость берём из aria-expanded */
.mode-ib:hover, .mode-ib[aria-expanded='true'] { color: var(--text-1); background: var(--fill-2); }
.mode-ib:active { transform: scale(0.92); background: var(--fill-3); }
.mode-ib:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.mode-chip:hover, .mode-chip[data-state='open'] { color: var(--text-1); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line-strong); }
.mode-sheet__head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; padding: 12px 20px 8px; flex: none; }
.mode-sheet__title { font-size: 17px; font-weight: 700; color: var(--text-1); }
.mode-sheet { padding: 0 8px 10px; }
.mode-sheet .opt { min-height: 60px; padding: 12px; }
.mode-sheet .pop__foot { padding: 10px 12px 4px; }
</style>
