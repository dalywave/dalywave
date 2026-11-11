<script setup lang="ts">
import type { TransitionMode } from '~/stores/player'

// Кнопка выбора режима переходов: круглый значок (нижний ряд плеера на телефоне) или чип с подписью.
// Атрибуты триггера (aria-expanded, data-state, обработчики) приходят от листа или всплывающей панели
defineProps<{ mode: TransitionMode, icon?: boolean, compact?: boolean }>()
const { t } = useI18n()

const ICONS: Record<TransitionMode, string> = {
  gapless: 'ph:link-simple-horizontal',
  smooth: 'ph:wave-sine',
  smart: 'ph:sparkle',
  dj: 'ph:vinyl-record',
}
</script>

<template>
  <button v-if="icon" type="button" class="mode-ib" :aria-label="`${t('transition.title')}: ${t(`transition.${mode}`)}`">
    <Icon :name="ICONS[mode]" class="mode-ib__icon" />
    <span v-if="mode !== 'gapless'" class="mode-ib__dot" aria-hidden="true" />
  </button>
  <button v-else type="button" class="mode-chip" :class="{ 'mode-chip--compact': compact }" :aria-label="`${t('transition.title')}: ${t(`transition.${mode}`)}`">
    <Icon :name="ICONS[mode]" class="mode-chip__icon" />
    <span v-if="!compact" class="mode-chip__label">{{ t(`transition.${mode}`) }}</span>
  </button>
</template>
