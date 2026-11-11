<script setup lang="ts">
withDefaults(defineProps<{
  selected?: boolean
  icon?: string
  removable?: boolean
  disabled?: boolean
  forceState?: 'hover'
}>(), {})

const emit = defineEmits<{ click: [], remove: [] }>()
</script>

<template>
  <span v-if="removable" class="chip chip--remove" :class="{ 'is-disabled': disabled }">
    <button type="button" class="chip__label" :disabled="disabled" @click="emit('click')">
      <Icon v-if="icon" :name="icon" class="chip__icon" />
      <slot />
    </button>
    <button type="button" class="chip__x" :aria-label="'Убрать'" :disabled="disabled" @click="emit('remove')">
      <Icon name="ph:x" />
    </button>
  </span>
  <button
    v-else
    type="button"
    class="chip"
    :class="{ 'is-selected': selected, [`is-${forceState}`]: forceState }"
    :aria-pressed="selected"
    :disabled="disabled"
    @click="emit('click')"
  >
    <Icon v-if="selected" name="ph:check" class="chip__icon" />
    <Icon v-else-if="icon" :name="icon" class="chip__icon" />
    <slot />
  </button>
</template>

<style scoped>
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  font-size: var(--fs-small);
  font-weight: 550;
  color: var(--text-2);
  box-shadow: inset 0 0 0 1px var(--line);
  white-space: nowrap;
  transition: background-color var(--dur-1), color var(--dur-1), box-shadow var(--dur-1);
}
.chip__icon { font-size: 16px; }
.chip:hover, .chip.is-hover { color: var(--text-1); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line-strong); }
.chip.is-selected { background: var(--text-1); color: var(--on-light); box-shadow: none; }
.chip:disabled, .chip.is-disabled { color: var(--text-4); box-shadow: inset 0 0 0 1px var(--hair); pointer-events: none; }
.chip--remove { gap: 2px; padding: 0 5px 0 0; color: var(--text-1); }
.chip__label { display: inline-flex; align-items: center; gap: 6px; height: 100%; padding: 0 4px 0 14px; border: 0; border-radius: var(--r-pill) 0 0 var(--r-pill); background: none; font: inherit; color: inherit; white-space: nowrap; }
.chip__x { display: grid; place-items: center; width: 22px; height: 22px; border: 0; border-radius: 50%; background: transparent; color: var(--text-3); font-size: 14px; }
.chip__x:hover { background: var(--fill-2); color: var(--text-1); }
</style>
