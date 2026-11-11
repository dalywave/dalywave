<script setup lang="ts">
const props = withDefaults(defineProps<{
  placeholder?: string
  /** Подсказка клавиш ⌘K */
  shortcut?: boolean
  autofocus?: boolean
  size?: 'md' | 'lg'
  forceState?: 'hover' | 'focus'
}>(), { placeholder: 'Треки, артисты, альбомы', size: 'md' })

const emit = defineEmits<{ submit: [value: string], focus: [] }>()
const model = defineModel<string>({ default: '' })
const input = ref<HTMLInputElement | null>(null)

defineExpose({ focus: () => input.value?.focus() })

onMounted(() => {
  if (props.autofocus) input.value?.focus()
})

function clear() {
  model.value = ''
  input.value?.focus()
}
</script>

<template>
  <form
    role="search"
    class="sf"
    :class="[`sf--${size}`, { 'is-filled': !!model, [`is-${forceState}`]: forceState }]"
    @submit.prevent="emit('submit', model)"
  >
    <Icon name="ph:magnifying-glass" class="sf__icon" />
    <input
      ref="input"
      v-model="model"
      type="search"
      class="sf__input"
      :placeholder="placeholder"
      :aria-label="placeholder"
      autocomplete="off"
      spellcheck="false"
      enterkeyhint="search"
      @focus="emit('focus')"
      @keydown.esc="model ? clear() : input?.blur()"
    >
    <UiKbd v-if="shortcut && !model" :keys="['⌘', 'K']" class="sf__kbd" />
    <button v-if="model" type="button" class="sf__clear" aria-label="Очистить" @click="clear">
      <Icon name="ph:x-circle-fill" />
    </button>
  </form>
</template>

<style scoped>
.sf {
  position: relative;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 40px;
  padding: 0 8px 0 14px;
  border-radius: var(--r-3);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-3);
  transition: box-shadow var(--dur-2), background-color var(--dur-2);
  min-width: 0;
}
.sf--lg { height: 48px; border-radius: var(--r-4); padding-left: 16px; }
.sf--lg .sf__input { font-size: 16px; }
.sf__icon { font-size: 18px; flex: none; }
.sf__input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: 0;
  background: none;
  font: inherit;
  font-size: var(--fs-body);
  color: var(--text-1);
  outline: none;
}
.sf__input::placeholder { color: var(--text-3); }
.sf__input::-webkit-search-cancel-button { display: none; }
.sf:hover, .sf.is-hover { background: var(--fill-2); }
.sf:focus-within, .sf.is-focus { background: var(--ink-3); box-shadow: inset 0 0 0 1px var(--accent-line), 0 0 0 4px var(--accent-soft); color: var(--text-2); }
.sf__kbd { flex: none; }
.sf__clear { display: grid; place-items: center; width: 28px; height: 28px; border: 0; border-radius: 50%; background: transparent; color: var(--text-3); font-size: 18px; flex: none; }
.sf__clear:hover { color: var(--text-1); }
</style>
