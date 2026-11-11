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
  gap: 6px;
  min-height: 232px;
  padding: 32px 24px;
  border-radius: var(--r-4);
  background:
    radial-gradient(70% 90% at 50% 0%, color-mix(in oklab, var(--accent) 9%, transparent), transparent 70%),
    var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  text-align: center;
  isolation: isolate;
  transition: box-shadow var(--dur-2), background-color var(--dur-2), transform var(--dur-2) var(--ease-out);
}
/* Пунктир поверх волоска: рисуется отдельно, чтобы скругление было ровным */
.dz::after {
  content: '';
  position: absolute;
  inset: 8px;
  z-index: -1;
  border: 1.5px dashed var(--line-strong);
  border-radius: calc(var(--r-4) - 8px);
  pointer-events: none;
  transition: border-color var(--dur-2);
}
.dz:hover:not(.is-disabled) { background-color: var(--fill-2); }
.dz:hover:not(.is-disabled)::after { border-color: var(--text-4); }
.dz.is-over { transform: scale(1.01); box-shadow: inset 0 0 0 1px var(--accent-line), 0 0 0 4px var(--accent-soft); }
.dz.is-over::after { border-color: var(--accent); }
.dz.is-disabled { opacity: 0.55; }
.dz__hit { position: absolute; inset: 0; z-index: 1; border: 0; border-radius: inherit; background: transparent; cursor: pointer; }
.dz__hit:disabled { cursor: default; }
.dz__hit:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.dz__icon {
  display: grid;
  place-items: center;
  width: 56px;
  height: 56px;
  margin-bottom: 10px;
  border-radius: 18px;
  background: var(--brand-gradient);
  box-shadow: 0 12px 32px -14px color-mix(in oklab, var(--brand-2) 80%, transparent), var(--inner-hi);
  color: #fff;
  font-size: 26px;
  transition: transform var(--dur-2) var(--ease-spring);
}
.dz.is-over .dz__icon { transform: translateY(4px); }
.dz__title { max-width: 420px; font-size: var(--fs-h4); font-weight: 650; letter-spacing: -0.01em; text-wrap: balance; }
.dz__hint { max-width: 420px; font-size: var(--fs-small); color: var(--text-3); text-wrap: pretty; }
.dz__button {
  display: inline-flex;
  align-items: center;
  height: 36px;
  margin-top: 14px;
  padding: 0 16px;
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
  .dz { min-height: 200px; padding: 28px 20px; }
}
@media (prefers-reduced-motion: reduce) {
  .dz, .dz__icon { transition: none; }
  .dz.is-over { transform: none; }
}
</style>
