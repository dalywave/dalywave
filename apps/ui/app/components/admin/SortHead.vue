<script setup lang="ts">
// Заголовок колонки с сортировкой — общий для таблиц админки (`useTableView`): щелчок сортирует
// по колонке, повторный — в обратном порядке; стрелка показывает выбранный порядок
const props = defineProps<{
  label: string
  /** Ключ сортировки из `useTableView({ sorts })` */
  k: string
  view: { sortBy: (k: string, numericFirstDesc?: boolean) => void, ariaSort: (k: string) => string, sortIcon: (k: string) => string | undefined }
  /** Числа — вправо; по умолчанию первый щелчок — сначала большие */
  num?: boolean
  /** Текстовая колонка: первый щелчок — по алфавиту */
  text?: boolean
}>()
const icon = computed(() => props.view.sortIcon(props.k))
const aria = computed(() => props.view.ariaSort(props.k) as 'ascending' | 'descending' | 'none')
</script>

<template>
  <button
    type="button"
    role="columnheader"
    class="sh"
    :class="{ 'ap-num': num }"
    :aria-sort="aria"
    @click="view.sortBy(k, !text)"
  >
    {{ label }}<Icon v-if="icon" :name="icon" />
  </button>
</template>

<style scoped>
.sh { display: inline-flex; align-items: center; gap: 4px; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.sh.ap-num { justify-content: flex-end; }
.sh:hover { color: var(--text-1); }
.sh:focus-visible { outline: none; border-radius: 4px; box-shadow: 0 0 0 2px var(--focus); }
</style>
