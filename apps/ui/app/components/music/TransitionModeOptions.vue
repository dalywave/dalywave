<script setup lang="ts">
import type { TransitionMode } from '~/stores/player'

// Содержимое выбора режима переходов: и во всплывающей панели, и в настройках
withDefaults(defineProps<{
  nextAt?: string | null
  web?: boolean
}>(), { nextAt: null, web: true })

const mode = defineModel<TransitionMode>('mode', { required: true })
const expressiveness = defineModel<number>('expressiveness', { default: 0.6 })
const { t } = useI18n()

const MODES: { value: TransitionMode, icon: string }[] = [
  { value: 'gapless', icon: 'ph:link-simple-horizontal' },
  { value: 'smooth', icon: 'ph:wave-sine' },
  { value: 'smart', icon: 'ph:sparkle' },
  { value: 'dj', icon: 'ph:vinyl-record' },
]
const exprLabel = computed(() => (expressiveness.value < 0.34 ? t('transition.subtle') : expressiveness.value < 0.67 ? t('transition.set') : t('transition.bold')))
</script>

<template>
  <div role="radiogroup" :aria-label="t('transition.title')">
    <template v-for="m in MODES" :key="m.value">
      <button type="button" role="radio" class="opt" :class="{ 'is-selected': mode === m.value }" :aria-checked="mode === m.value" @click="mode = m.value">
        <span class="opt__icon"><Icon :name="m.icon" /></span>
        <span class="opt__text">
          <span class="opt__title">{{ t(`transition.${m.value}`) }}<UiBadge v-if="m.value === 'dj' && web" variant="soon">{{ t('transition.inApp') }}</UiBadge></span>
          <span class="opt__desc">{{ t(`transition.${m.value}Desc`) }}</span>
        </span>
        <span class="opt__radio" aria-hidden="true" />
      </button>
      <div v-if="mode === m.value && (m.value === 'smart' || m.value === 'dj')" class="opt__extra">
        <div class="opt__extra-label"><span>{{ t('transition.expressiveness') }}</span><span>{{ exprLabel }}</span></div>
        <UiSlider v-model="expressiveness" :label="t('transition.expressiveness')" :value-text="exprLabel" />
      </div>
    </template>
  </div>
  <div v-if="nextAt" class="pop__foot">{{ t('transition.next', { time: nextAt }) }}</div>
</template>

<style>
.opt { display: flex; align-items: flex-start; gap: 12px; width: 100%; padding: 10px; border: 0; border-radius: 12px; background: transparent; text-align: left; transition: background-color var(--dur-1); }
.opt:hover, .opt:focus-visible { background: var(--fill-2); outline: none; }
.opt__icon { display: grid; place-items: center; width: 32px; height: 32px; flex: none; border-radius: 10px; background: var(--fill-2); color: var(--text-2); font-size: 18px; }
.opt.is-selected .opt__icon { background: var(--accent-soft); color: var(--accent); }
.opt__text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.opt__title { display: flex; align-items: center; gap: 8px; font-weight: 600; }
.opt__desc { margin-top: 1px; font-size: var(--fs-caption); color: var(--text-3); line-height: 1.35; }
.opt__radio { position: relative; width: 18px; height: 18px; margin-top: 7px; flex: none; border-radius: 50%; box-shadow: inset 0 0 0 1.5px var(--line-strong); transition: box-shadow var(--dur-2) var(--ease-out); }
.opt.is-selected .opt__radio { box-shadow: inset 0 0 0 5px var(--accent); }
.opt__extra { padding: 2px 10px 10px 54px; }
.opt__extra-label { display: flex; justify-content: space-between; margin-bottom: 2px; font-size: var(--fs-caption); color: var(--text-3); }
</style>
