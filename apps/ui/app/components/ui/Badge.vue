<script setup lang="ts">
// Значки: качество потока, explicit, «новое», «скоро», метаданные (темп, тональность), ошибка
withDefaults(defineProps<{
  variant?: 'quality' | 'explicit' | 'new' | 'soon' | 'meta' | 'danger' | 'success'
  icon?: string
  label?: string
}>(), { variant: 'meta' })
</script>

<template>
  <span v-if="variant === 'explicit'" class="badge badge--explicit" role="img" :aria-label="label ?? 'Ненормативная лексика'" :title="label ?? 'Ненормативная лексика'">E</span>
  <span v-else class="badge" :class="`badge--${variant}`">
    <Icon v-if="icon" :name="icon" class="badge__icon" />
    <slot />
  </span>
</template>

<style scoped>
.badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 20px;
  padding: 0 7px;
  border-radius: var(--r-1);
  font-size: var(--fs-micro);
  font-weight: 650;
  letter-spacing: 0.01em;
  white-space: nowrap;
  flex: none;
  vertical-align: middle;
}
.badge__icon { font-size: 13px; }
.badge--quality { color: var(--text-2); box-shadow: inset 0 0 0 1px var(--line-strong); }
.badge--quality .badge__icon { color: var(--accent); }
.badge--explicit {
  width: 16px;
  height: 16px;
  padding: 0;
  justify-content: center;
  border-radius: 4px;
  background: rgba(232, 236, 244, 0.5);
  color: var(--on-light);
  font-size: 10px;
  font-weight: 750;
}
.badge--new { color: var(--accent); background: var(--accent-soft); }
.badge--soon { color: var(--text-1); background: rgba(10, 12, 16, 0.6); box-shadow: inset 0 0 0 1px var(--line-strong); -webkit-backdrop-filter: blur(8px); backdrop-filter: blur(8px); }
.badge--meta { color: var(--text-2); background: var(--fill-2); }
.badge--danger { color: var(--danger); background: color-mix(in oklab, var(--danger) 14%, transparent); }
.badge--success { color: var(--success); background: color-mix(in oklab, var(--success) 14%, transparent); }
</style>
