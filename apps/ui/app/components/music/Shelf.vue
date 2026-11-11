<script setup lang="ts">
// Полка: заголовок, «Все», стрелки и горизонтальный ряд с привязкой к карточкам.
// На телефоне — свайп без стрелок, ряд выходит за поля экрана
withDefaults(defineProps<{
  title: string
  hint?: string
  to?: string
  /** Сколько карточек видно на большом экране */
  cols?: number
  /** Сетка вместо ряда (плитки «Продолжить») */
  rows?: number
  /** Ширина карточки на телефоне */
  phoneWidth?: string
}>(), { cols: 5, rows: 1, phoneWidth: '148px' })

const { t } = useI18n()
const row = ref<HTMLElement | null>(null)
const atStart = ref(true)
const atEnd = ref(false)

function update() {
  const el = row.value
  if (!el) return
  atStart.value = el.scrollLeft < 4
  atEnd.value = el.scrollLeft + el.clientWidth >= el.scrollWidth - 4
}

function scroll(dir: 1 | -1) {
  const el = row.value
  if (!el) return
  el.scrollBy({ left: dir * el.clientWidth * 0.9, behavior: 'smooth' })
}

let ro: ResizeObserver | null = null
onMounted(() => {
  update()
  ro = new ResizeObserver(update)
  if (row.value) ro.observe(row.value)
})
onBeforeUnmount(() => ro?.disconnect())
const scrollable = computed(() => !(atStart.value && atEnd.value))
</script>

<template>
  <section class="shelf" :aria-label="title">
    <header class="shelf__head">
      <div class="shelf__titles">
        <h2 class="shelf__title">{{ title }}</h2>
        <p v-if="hint" class="shelf__hint">{{ hint }}</p>
      </div>
      <div class="shelf__actions">
        <slot name="actions" />
        <UiButton v-if="to" :to="to" variant="ghost" size="sm">{{ t('common.all') }}</UiButton>
        <div v-if="scrollable" class="shelf__nav">
          <UiIconButton icon="ph:caret-left" :label="t('common.scrollLeft')" size="sm" :disabled="atStart" :tooltip="false" @click="scroll(-1)" />
          <UiIconButton icon="ph:caret-right" :label="t('common.scrollRight')" size="sm" :disabled="atEnd" :tooltip="false" @click="scroll(1)" />
        </div>
      </div>
    </header>
    <div
      ref="row"
      class="shelf__row no-scrollbar"
      :class="{ 'shelf__row--grid': rows > 1 }"
      :style="{ '--cols': cols, '--rows': rows, '--phone-w': phoneWidth }"
      @scroll.passive="update"
    >
      <slot />
    </div>
  </section>
</template>

<style scoped>
.shelf { display: flex; flex-direction: column; gap: 16px; min-width: 0; }
.shelf__head { display: flex; align-items: flex-end; gap: 16px; min-width: 0; }
.shelf__titles { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.shelf__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.2; }
.shelf__hint { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.shelf__actions { display: flex; align-items: center; gap: 4px; }
.shelf__nav { display: flex; gap: 4px; margin-left: 4px; }
.shelf__nav :deep(.ib) { box-shadow: inset 0 0 0 1px var(--line); }
.shelf__row {
  --gap: 24px;
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: calc((100% - (var(--cols) - 1) * var(--gap)) / var(--cols));
  gap: var(--gap);
  overflow-x: auto;
  scroll-snap-type: x mandatory;
  overscroll-behavior-x: contain;
}
.shelf__row > :deep(*) { scroll-snap-align: start; }
.shelf__row--grid { grid-template-rows: repeat(var(--rows), auto); gap: 16px var(--gap); }

@media (max-width: 1279px) {
  .shelf__row { --gap: 20px; grid-auto-columns: calc((100% - (max(var(--cols) - 1, 1) - 1) * var(--gap)) / max(var(--cols) - 1, 1)); }
}
@media (max-width: 1180px) {
  .shelf__row--grid { grid-auto-columns: calc((100% - var(--gap)) / 2); }
}
@media (max-width: 699px) {
  .shelf { gap: 12px; }
  .shelf__head { padding: 0 16px; }
  .shelf__nav { display: none; }
  .shelf__actions :deep(.btn) { min-height: 44px; }
  .shelf__row { --gap: 12px; grid-auto-columns: var(--phone-w); padding: 0 16px; scroll-padding-inline: 16px; }
  .shelf__row--grid { grid-auto-columns: 82%; }
}
</style>
