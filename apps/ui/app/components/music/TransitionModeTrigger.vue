<script setup lang="ts">
import type { TransitionMode } from '~/stores/player'

// Кнопка выбора режима переходов: круглый значок — как соседние кнопки плеера (маленький в нижней
// панели большого экрана, крупный в нижнем ряду плеера на телефоне), или чип с подписью.
// Атрибуты триггера (aria-expanded, data-state, обработчики) приходят от листа или всплывающей панели
// и уходят на саму кнопку, а не на обёртку подсказки
defineOptions({ inheritAttrs: false })

const props = withDefaults(defineProps<{
  mode: TransitionMode
  icon?: boolean
  compact?: boolean
  /** Размер значка: sm — нижняя панель большого экрана, lg — плеер на телефоне */
  size?: 'sm' | 'lg'
  /** Подсказка с названием режима при наведении */
  tooltip?: boolean
}>(), { size: 'lg' })
const { t } = useI18n()

const ICONS: Record<TransitionMode, string> = {
  gapless: 'ph:link-simple-horizontal',
  smooth: 'ph:wave-sine',
  smart: 'ph:sparkle',
  dj: 'ph:vinyl-record',
}
const name = computed(() => t(`transition.${props.mode}`))
</script>

<template>
  <UiTooltip v-if="icon" :disabled="!tooltip">
    <button v-bind="$attrs" type="button" class="mode-ib" :class="`mode-ib--${size}`" :aria-label="`${t('transition.title')}: ${name}`">
      <Icon :name="ICONS[mode]" class="mode-ib__icon" />
      <span v-if="mode !== 'gapless'" class="mode-ib__dot" aria-hidden="true" />
    </button>
    <template #content>
      <span class="tooltip__muted">{{ t('transition.title') }}</span>{{ name }}
    </template>
  </UiTooltip>
  <button v-else v-bind="$attrs" type="button" class="mode-chip" :class="{ 'mode-chip--compact': compact }" :aria-label="`${t('transition.title')}: ${name}`">
    <Icon :name="ICONS[mode]" class="mode-chip__icon" />
    <span v-if="!compact" class="mode-chip__label">{{ name }}</span>
  </button>
</template>
