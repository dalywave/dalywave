<script setup lang="ts" generic="T extends string | number">
import { DropdownMenuContent, DropdownMenuPortal, DropdownMenuRoot, DropdownMenuTrigger } from 'reka-ui'
import type { MenuEntry } from '~/types/ui'

// Выбор из списка для фильтров и настроек админки. Вместо системного <select>: тот же вид, что у меню,
// на телефоне — лист снизу с крупными строками. variant="title" — имя слушателя заголовком со стрелкой
// class и прочие атрибуты — на саму кнопку: у меню и листа нет своего корневого элемента
defineOptions({ inheritAttrs: false })
interface AdminSelectOption<V> { value: V, label: string, hint?: string }
const props = withDefaults(defineProps<{
  options: AdminSelectOption<T>[]
  /** Подпись для диктора и шапка листа на телефоне */
  label: string
  variant?: 'field' | 'title'
  align?: 'start' | 'end'
}>(), { variant: 'field', align: 'start' })
const model = defineModel<T>({ required: true })
const open = ref(false)
const sheet = useMenuSheet()

const current = computed(() => props.options.find(o => o.value === model.value))
const items = computed<MenuEntry[]>(() => [
  ...(sheet.value ? [{ kind: 'heading' as const, label: props.label }] : []),
  ...props.options.map(o => ({ label: o.label, hint: o.hint, checked: o.value === model.value, onSelect: () => (model.value = o.value) })),
])
</script>

<template>
  <UiActionSheet v-if="sheet" v-model:open="open" :items="items">
    <button v-bind="$attrs" type="button" class="asl" :class="`asl--${variant}`" :aria-label="`${label}: ${current?.label ?? ''}`">
      <span class="asl__value">{{ current?.label ?? '—' }}</span>
      <Icon name="ph:caret-down" class="asl__caret" />
    </button>
  </UiActionSheet>
  <DropdownMenuRoot v-else v-model:open="open" :modal="false">
    <DropdownMenuTrigger as-child>
      <button v-bind="$attrs" type="button" class="asl" :class="[`asl--${variant}`, { 'is-open': open }]" :aria-label="`${label}: ${current?.label ?? ''}`">
        <span class="asl__value">{{ current?.label ?? '—' }}</span>
        <Icon name="ph:caret-down" class="asl__caret" />
      </button>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent class="menu asl__menu" side="bottom" :align="align" :side-offset="6" :collision-padding="12">
        <UiMenuItems :items="items" />
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>

<style>
.asl { display: inline-flex; align-items: center; justify-content: space-between; gap: 10px; min-width: 0; max-width: 100%; border: 0; color: var(--text-1); font: inherit; text-align: left; cursor: pointer; -webkit-tap-highlight-color: transparent; }
.asl__value { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.asl__caret { flex: none; font-size: 14px; color: var(--text-3); transition: transform var(--dur-2) var(--ease-out); }
.asl.is-open .asl__caret { transform: rotate(180deg); }
.asl:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.asl--field { height: 36px; padding: 0 10px 0 12px; border-radius: var(--r-3); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); font-size: var(--fs-small); transition: box-shadow var(--dur-1), background-color var(--dur-1); }
.asl--field:hover, .asl--field.is-open { box-shadow: inset 0 0 0 1px var(--line-strong); }
/* Сдвиг влево через left, не отрицательным полем: поле сжимает ширину по содержимому и обрезает имя */
.asl--title { position: relative; left: -8px; gap: 8px; height: auto; padding: 2px 8px; border-radius: var(--r-3); background: none; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); transition: background-color var(--dur-1); }
.asl--title .asl__caret { font-size: 18px; color: var(--text-2); }
.asl--title:hover, .asl--title.is-open { background: var(--fill-1); }
.asl__menu { max-height: min(420px, var(--reka-dropdown-menu-content-available-height, 420px)); overflow-y: auto; }
@media (max-width: 699px) {
  .asl--field { height: 44px; font-size: var(--fs-body); }
}
</style>
