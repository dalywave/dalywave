<script setup lang="ts">
// Строка настроек: подпись слева, элемент управления справа (слот по умолчанию).
// Строки внутри <SettingsGroup> разделяются волосками.
//   description — короткая подсказка в одну строку, только где без неё не понять
//   #lead       — что-то перед подписью (аватар); icon — значок, если он действительно нужен
//   stack       — на телефоне элемент управления уходит под подпись на всю ширину
//   indent      — подстрока: без волоска, относится к строке выше
//   as="button" — вся строка нажимается; radio + checked — вариант с кружком справа
const props = withDefaults(defineProps<{
  icon?: string
  title?: string
  description?: string
  stack?: boolean
  indent?: boolean
  as?: 'div' | 'button'
  radio?: boolean
  checked?: boolean
  danger?: boolean
  disabled?: boolean
}>(), { as: 'div' })

defineEmits<{ click: [e: MouseEvent] }>()
const slots = useSlots()
const hasLead = computed(() => !!props.icon || !!slots.lead)
const attrs = computed(() => {
  if (props.as !== 'button') return {}
  return {
    'type': 'button',
    'disabled': props.disabled || undefined,
    'role': props.radio ? 'radio' : undefined,
    'aria-checked': props.radio ? props.checked : undefined,
  }
})
</script>

<template>
  <component
    :is="as"
    v-bind="attrs"
    class="sr"
    :class="{
      'has-lead': hasLead,
      'sr--stack': stack,
      'sr--indent': indent,
      'sr--action': as === 'button',
      'sr--danger': danger,
      'is-checked': radio && checked,
      'is-disabled': disabled,
    }"
    @click="$emit('click', $event)"
  >
    <span v-if="hasLead" class="sr__lead">
      <slot name="lead"><Icon :name="icon!" /></slot>
    </span>
    <span v-if="title || description || $slots.title || $slots.description" class="sr__text">
      <span v-if="title || $slots.title" class="sr__title"><slot name="title">{{ title }}</slot></span>
      <span v-if="description || $slots.description" class="sr__desc"><slot name="description">{{ description }}</slot></span>
    </span>
    <span v-if="$slots.default" class="sr__control"><slot /></span>
    <span v-if="radio" class="sr__radio" aria-hidden="true" />
  </component>
</template>

<style scoped>
.sr {
  --sr-pad: 0px;
  --sr-lead: 24px;
  --sr-gap: 14px;
  --sr-inset: var(--sr-pad);
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--sr-gap);
  width: 100%;
  min-height: 56px;
  padding: 10px var(--sr-pad);
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
}
.sr.has-lead { --sr-inset: calc(var(--sr-pad) + var(--sr-lead) + var(--sr-gap)); }
/* Волосок между строками */
.sr + .sr::before {
  content: '';
  position: absolute;
  top: 0;
  left: var(--sr-inset);
  right: var(--sr-pad);
  height: 1px;
  background: var(--hair);
  pointer-events: none;
}
.sr--indent::before { display: none; }

.sr__lead { display: grid; place-items: center; width: var(--sr-lead); height: var(--sr-lead); flex: none; color: var(--text-2); font-size: 20px; }
.sr__text { display: flex; flex: 1; flex-direction: column; gap: 1px; min-width: 0; }
.sr__title { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; font-size: var(--fs-body); font-weight: 500; line-height: 1.35; color: var(--text-1); }
.sr__desc { font-size: var(--fs-small); line-height: 1.4; color: var(--text-3); }
.sr__control { display: flex; flex: none; align-items: center; justify-content: flex-end; gap: 8px; min-width: 0; max-width: 62%; }

/* Нажимаемая строка: подсветка выходит за колонку, текст остаётся на месте */
.sr--action { --sr-pad: 12px; width: calc(100% + 24px); margin: 0 -12px; border-radius: var(--r-3); cursor: pointer; transition: background-color var(--dur-1); -webkit-tap-highlight-color: transparent; }
.sr--action:hover { background: var(--fill-1); }
.sr--action:active { background: var(--fill-2); }
.sr--action:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.sr--action:hover::before, .sr--action:hover + .sr::before { opacity: 0; }
.sr.is-disabled { cursor: default; pointer-events: none; }
.sr.is-disabled .sr__text { opacity: 0.45; }
.sr--danger .sr__title { color: var(--danger); }

/* Кружок варианта */
.sr__radio { position: relative; width: 20px; height: 20px; flex: none; border-radius: 50%; box-shadow: inset 0 0 0 1.5px var(--line-strong); transition: box-shadow var(--dur-2) var(--ease-out); }
.sr.is-checked .sr__radio { box-shadow: inset 0 0 0 6px var(--accent); }

/* Подстрока под выбранным вариантом */
.sr--indent { min-height: 0; padding-top: 0; padding-bottom: 14px; }
.sr--indent .sr__control { flex: 1; max-width: none; justify-content: stretch; }

@media (max-width: 699px) {
  .sr { min-height: 56px; }
  .sr__control { max-width: 70%; }
  .sr--stack { flex-wrap: wrap; row-gap: 10px; padding-bottom: 14px; }
  .sr--stack .sr__control { flex: 1 0 100%; max-width: none; justify-content: stretch; }
  .sr--stack .sr__control > :deep(*) { flex: 1; min-width: 0; }
}
</style>
