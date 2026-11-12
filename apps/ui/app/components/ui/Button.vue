<script setup lang="ts">
// Кнопка-пилюля. Основная — белая (акцент бережём для того, что играет), второстепенная — стекло,
// текстовая — без фона. forceState — только для галереи компонентов.
const props = withDefaults(defineProps<{
  variant?: 'primary' | 'secondary' | 'ghost' | 'accent' | 'danger'
  size?: 'sm' | 'md' | 'lg'
  icon?: string
  iconRight?: string
  to?: string
  href?: string
  type?: 'button' | 'submit'
  loading?: boolean
  disabled?: boolean
  block?: boolean
  forceState?: 'hover' | 'pressed' | 'focus'
}>(), { variant: 'secondary', size: 'md', type: 'button' })

const NuxtLink = resolveComponent('NuxtLink')
const tag = computed(() => (props.to ? NuxtLink : props.href ? 'a' : 'button'))
const attrs = computed(() => {
  if (props.to) return { to: props.to }
  if (props.href) return { href: props.href }
  return { type: props.type, disabled: props.disabled || props.loading }
})
</script>

<template>
  <component
    :is="tag"
    v-bind="attrs"
    class="btn"
    :class="[`btn--${variant}`, `btn--${size}`, { 'btn--block': block, 'is-loading': loading, [`is-${forceState}`]: forceState }]"
    :aria-busy="loading || undefined"
  >
    <span v-if="loading" class="btn__spinner" aria-hidden="true" />
    <Icon v-else-if="icon" :name="icon" class="btn__icon" />
    <span v-if="$slots.default" class="btn__label"><slot /></span>
    <Icon v-if="iconRight" :name="iconRight" class="btn__icon" />
  </component>
</template>

<style scoped>
.btn {
  --h: 40px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: var(--h);
  padding: 0 18px;
  border: 0;
  border-radius: var(--r-pill);
  font-size: var(--fs-body);
  font-weight: 600;
  letter-spacing: -0.005em;
  white-space: nowrap;
  user-select: none;
  transition: background-color var(--dur-1), color var(--dur-1), box-shadow var(--dur-2), transform var(--dur-1) var(--ease-out), opacity var(--dur-1);
}
.btn__icon { font-size: 18px; }
.btn--sm { --h: 32px; padding: 0 14px; font-size: var(--fs-small); }
.btn--sm .btn__icon { font-size: 16px; }
.btn--lg { --h: 48px; padding: 0 24px; font-size: 15px; }
.btn--block { width: 100%; }
.btn:active, .btn.is-pressed { transform: scale(0.97); }
.btn:disabled { cursor: default; }
.btn.is-focus { outline: 2px solid var(--focus); outline-offset: 2px; }

.btn--primary { background: var(--text-1); color: var(--on-light); }
.btn--primary:hover, .btn--primary.is-hover { background: #fff; }
.btn--primary:active, .btn--primary.is-pressed { background: #dfe3ea; }
.btn--primary:disabled { background: var(--fill-2); color: var(--text-4); box-shadow: none; }

.btn--secondary { background: var(--fill-2); color: var(--text-1); box-shadow: inset 0 0 0 1px var(--line); }
.btn--secondary:hover, .btn--secondary.is-hover { background: var(--fill-3); box-shadow: inset 0 0 0 1px var(--line-strong); }
.btn--secondary:active, .btn--secondary.is-pressed { background: rgba(255, 255, 255, 0.15); }
.btn--secondary:disabled { background: var(--fill-1); color: var(--text-4); box-shadow: inset 0 0 0 1px var(--hair); }

.btn--ghost { background: transparent; color: var(--text-2); padding: 0 12px; }
.btn--ghost:hover, .btn--ghost.is-hover { color: var(--text-1); background: var(--fill-1); }
.btn--ghost:active, .btn--ghost.is-pressed { background: var(--fill-2); }
.btn--ghost:disabled { color: var(--text-4); background: transparent; }

.btn--accent { background: var(--accent-gradient); color: #fff; text-shadow: 0 1px 1px rgba(0, 0, 0, 0.25); }
.btn--accent:hover, .btn--accent.is-hover { filter: brightness(1.08); }
.btn--accent:disabled { opacity: 0.4; filter: saturate(0.6); }

.btn--danger { background: color-mix(in oklab, var(--danger) 16%, transparent); color: var(--danger); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--danger) 30%, transparent); }
.btn--danger:hover, .btn--danger.is-hover { background: color-mix(in oklab, var(--danger) 24%, transparent); }

.btn__spinner {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid currentColor;
  border-right-color: transparent;
  animation: spin 0.8s linear infinite;
}
</style>
