<script setup lang="ts">
// Круглая кнопка с иконкой. label обязателен: это и подпись для экранного диктора, и подсказка.
// Атрибуты (class, обработчики) уходят на саму кнопку, а не на обёртку подсказки
defineOptions({ inheritAttrs: false })

const props = withDefaults(defineProps<{
  icon: string
  label: string
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  /** Включено (очередь открыта и т. п.): акцент и точка снизу */
  active?: boolean
  /** Нажато в смысле переключателя — для aria-pressed */
  pressed?: boolean
  tone?: 'default' | 'liked' | 'solid' | 'glass'
  tooltip?: string | false
  kbd?: string[]
  side?: 'top' | 'bottom' | 'left' | 'right'
  disabled?: boolean
  to?: string
  forceState?: 'hover' | 'pressed' | 'focus'
}>(), { size: 'md', tone: 'default', side: 'top' })

const NuxtLink = resolveComponent('NuxtLink')
const tip = computed(() => (props.tooltip === false ? undefined : props.tooltip ?? props.label))
</script>

<template>
  <UiTooltip :text="tip" :kbd="kbd" :side="side" :disabled="disabled || tooltip === false">
    <component
      :is="to ? NuxtLink : 'button'"
      v-bind="$attrs"
      :to="to"
      :type="to ? undefined : 'button'"
      class="ib"
      :class="[`ib--${size}`, `ib--${tone}`, { 'is-on': active, [`is-${forceState}`]: forceState }]"
      :aria-label="label"
      :aria-pressed="pressed ?? undefined"
      :disabled="disabled || undefined"
    >
      <Icon :name="icon" class="ib__icon" />
    </component>
  </UiTooltip>
</template>

<style scoped>
.ib {
  --size: 36px;
  position: relative;
  display: inline-grid;
  place-items: center;
  flex: none;
  width: var(--size);
  height: var(--size);
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-1), transform var(--dur-1) var(--ease-out);
}
.ib__icon { font-size: 20px; }
.ib--xs { --size: 24px; }
.ib--xs .ib__icon { font-size: 16px; }
.ib--sm { --size: 32px; }
.ib--sm .ib__icon { font-size: 18px; }
.ib--lg { --size: 44px; }
.ib--lg .ib__icon { font-size: 24px; }
.ib--xl { --size: 52px; }
.ib--xl .ib__icon { font-size: 28px; }
.ib:hover, .ib.is-hover { color: var(--text-1); background: var(--fill-2); }
.ib:active, .ib.is-pressed { transform: scale(0.92); background: var(--fill-3); }
.ib.is-focus { outline: 2px solid var(--focus); outline-offset: 2px; }
.ib:disabled { color: var(--text-4); background: transparent; pointer-events: none; }
.ib.is-on { color: var(--accent); }
.ib.is-on::after {
  content: '';
  position: absolute;
  bottom: 1px;
  left: 50%;
  width: 4px;
  height: 4px;
  margin-left: -2px;
  border-radius: 50%;
  background: currentColor;
}
.ib--liked { color: var(--accent); }
.ib--solid { color: var(--text-1); }
.ib--glass {
  color: var(--text-1);
  background: var(--glass-btn);
  -webkit-backdrop-filter: blur(16px);
  backdrop-filter: blur(16px);
  box-shadow: inset 0 0 0 1px var(--line);
}
.ib--glass:hover { background: var(--glass-btn-hover); }
/* Планшет под пальцем: значок прежний, а область нажатия — не меньше 44 pt. Залипший после
   нажатия :hover не подсвечивает кнопку */
@media (pointer: coarse) and (min-width: 700px) {
  .ib::before { content: ''; position: absolute; inset: min(0px, calc((var(--size) - 44px) / 2)); border-radius: 50%; }
  .ib:hover:not(:active, .is-hover) { background: transparent; color: var(--text-2); }
  .ib--liked:hover:not(:active), .ib.is-on:hover:not(:active) { color: var(--accent); }
  .ib--solid:hover:not(:active) { color: var(--text-1); }
  .ib--glass:hover:not(:active) { color: var(--text-1); background: var(--glass-btn); }
}
</style>
