<script setup lang="ts">
// Поле для файлов: перетащить или выбрать. Вся зона — одна кнопка (клавиатура, экранный диктор);
// при перетаскивании над ней — подсветка. Файлы уходят наверх событием `files`
const props = withDefaults(defineProps<{
  title: string
  hint?: string
  icon?: string
  accept?: string
  multiple?: boolean
  disabled?: boolean
  /** Подпись кнопки выбора */
  button: string
}>(), { icon: 'ph:upload-simple' })

const emit = defineEmits<{ files: [files: File[]] }>()
const input = ref<HTMLInputElement | null>(null)
const over = ref(false)
let depth = 0

function pick() {
  if (!props.disabled) input.value?.click()
}

function take(list: FileList | null | undefined) {
  const files = list ? Array.from(list) : []
  if (files.length && !props.disabled) emit('files', props.multiple ? files : files.slice(0, 1))
}

function onChange(e: Event) {
  const el = e.target as HTMLInputElement
  take(el.files)
  el.value = ''
}

function onEnter(e: DragEvent) {
  if (!e.dataTransfer?.types.includes('Files')) return
  depth++
  over.value = true
}
function onLeave() {
  depth = Math.max(0, depth - 1)
  if (!depth) over.value = false
}
function onDrop(e: DragEvent) {
  depth = 0
  over.value = false
  take(e.dataTransfer?.files)
}
</script>

<template>
  <div
    class="dz"
    :class="{ 'is-over': over, 'is-disabled': disabled }"
    @dragenter.prevent="onEnter"
    @dragover.prevent
    @dragleave="onLeave"
    @drop.prevent="onDrop"
  >
    <button type="button" class="dz__hit" :disabled="disabled" :aria-label="`${title}. ${button}`" @click="pick" />
    <span class="dz__icon" aria-hidden="true"><Icon :name="over ? 'ph:arrow-down' : icon" /></span>
    <span class="dz__title">{{ title }}</span>
    <span v-if="hint" class="dz__hint">{{ hint }}</span>
    <span class="dz__button" aria-hidden="true">{{ button }}</span>
    <input ref="input" class="dz__input" type="file" :accept="accept" :multiple="multiple" tabindex="-1" aria-hidden="true" @change="onChange">
  </div>
</template>

<style scoped>
.dz {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  min-height: 220px;
  padding: 32px 24px;
  border: 1.5px dashed var(--line-strong);
  border-radius: var(--r-4);
  background: var(--fill-1);
  text-align: center;
  transition: border-color var(--dur-2), background-color var(--dur-2), transform var(--dur-2) var(--ease-out);
}
.dz:hover:not(.is-disabled) { border-color: var(--text-4); background-color: var(--fill-2); }
.dz.is-over { border-color: var(--accent); background-color: var(--accent-soft); transform: scale(1.01); }
.dz.is-disabled { opacity: 0.55; }
.dz__hit { position: absolute; inset: -1.5px; z-index: 1; border: 0; border-radius: inherit; background: transparent; cursor: pointer; }
.dz__hit:disabled { cursor: default; }
.dz__hit:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.dz__icon {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  margin-bottom: 10px;
  border-radius: 50%;
  background: var(--fill-2);
  color: var(--text-2);
  font-size: 24px;
  transition: transform var(--dur-2) var(--ease-spring), color var(--dur-2);
}
.dz.is-over .dz__icon { color: var(--accent); transform: translateY(4px); }
.dz__title { max-width: 420px; font-size: var(--fs-h4); font-weight: 650; letter-spacing: -0.01em; text-wrap: balance; }
.dz__hint { max-width: 420px; font-size: var(--fs-small); color: var(--text-3); text-wrap: pretty; }
.dz__button {
  display: inline-flex;
  align-items: center;
  height: 40px;
  margin-top: 16px;
  padding: 0 18px;
  border-radius: var(--r-pill);
  background: var(--fill-2);
  box-shadow: inset 0 0 0 1px var(--line);
  font-size: var(--fs-small);
  font-weight: 600;
  transition: background-color var(--dur-1);
}
.dz:hover:not(.is-disabled) .dz__button { background: var(--fill-3); }
.dz__input { position: absolute; width: 1px; height: 1px; opacity: 0; pointer-events: none; }
@media (max-width: 699px) {
  .dz { min-height: 0; padding: 28px 20px; }
  .dz__button { height: 44px; padding: 0 22px; font-size: var(--fs-body); }
}
@media (prefers-reduced-motion: reduce) {
  .dz, .dz__icon { transition: none; }
  .dz.is-over { transform: none; }
}
</style>
