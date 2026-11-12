<script setup lang="ts">
// Шапка страницы альбома, артиста, плейлиста: обложка, вид, название, подробности. Фон — ровный:
// цвет на странице даёт сама обложка.
// meta — короткая строка «9 треков · 45 мин» (разделители — точки, не переносятся отдельно).
// actions — две главные кнопки («Слушать» и вторая), tools — круглые второстепенные (+, скачать, ⋯).
// Компьютер: всё одним рядом. Телефон: tools уезжают в верхнюю строку рядом с «назад»,
// а две главные кнопки делят ширину поровну — подписи не обрезаются
withDefaults(defineProps<{
  kicker: string
  title: string
  round?: boolean
  /** Куда «назад» на телефоне, если истории нет */
  back?: string
}>(), { back: '/' })

const ui = useUiStore()

// Название ушло под верхнюю строку — она показывает его сама
const titleEl = ref<HTMLElement | null>(null)
const condensed = ref(false)
let io: IntersectionObserver | null = null
onMounted(() => {
  if (!titleEl.value) return
  io = new IntersectionObserver(([e]) => {
    if (e) condensed.value = !e.isIntersecting && e.boundingClientRect.top < 100
  }, { rootMargin: '-100px 0px 0px 0px' })
  io.observe(titleEl.value)
})
onBeforeUnmount(() => io?.disconnect())
</script>

<template>
  <MusicPhoneNav v-if="ui.isPhone" :title="title" :condensed="condensed" :fallback="back">
    <slot name="tools" />
  </MusicPhoneNav>
  <header class="ph" :class="{ 'ph--round': round }">
    <div class="ph__art"><slot name="art" /></div>
    <div class="ph__text">
      <div class="ph__kicker">{{ kicker }}</div>
      <h1 ref="titleEl" class="ph__title">{{ title }}</h1>
      <div v-if="$slots.sub" class="ph__sub"><slot name="sub" /></div>
      <div v-if="$slots.meta" class="ph__meta"><slot name="meta" /></div>
      <div v-if="$slots.actions || $slots.tools" class="ph__actions">
        <slot name="actions" />
        <template v-if="!ui.isPhone"><slot name="tools" /></template>
      </div>
    </div>
  </header>
</template>

<style scoped>
.ph { position: relative; display: flex; align-items: flex-end; gap: 32px; padding: 24px 0 8px; }
.ph__art { width: 232px; flex: none; }
.ph__art :deep(.art), .ph__art :deep(.pc) { width: 100%; border-radius: var(--r-4); box-shadow: var(--shadow-cover); }
.ph--round .ph__art :deep(.art) { border-radius: 50%; }
.ph__text { display: flex; flex-direction: column; gap: 8px; min-width: 0; padding-bottom: 4px; }
.ph__kicker { font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-2); }
/* Длинное название — не больше трёх строк, дальше многоточие */
.ph__title {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  font-size: clamp(32px, 4.4vw, var(--fs-display));
  font-weight: 750;
  letter-spacing: var(--tr-display);
  line-height: 1.04;
  text-wrap: balance;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  line-clamp: 3;
}
.ph__sub { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-width: 0; font-size: var(--fs-h4); font-weight: 550; color: var(--text-1); }
.ph__sub :deep(a:hover) { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.ph__meta { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 0; font-size: var(--fs-small); color: var(--text-2); }
/* Разделитель «·» держится за следующий пункт: строка не кончается точкой */
.ph__meta > :deep(*) { white-space: nowrap; }
.ph__meta > :deep(* + *)::before { content: '·'; margin: 0 8px; color: var(--text-4); }
.ph__actions { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin-top: 12px; }
@media (max-width: 1023px) {
  .ph { gap: 24px; }
  .ph__art { width: 184px; }
}
/* Колонку сужает и правая панель, не только окно: шапка подстраивается под ширину колонки */
@media (min-width: 700px) {
  @container main (max-width: 820px) {
    .ph { gap: 24px; }
    .ph__art { width: 168px; }
    .ph__title { font-size: clamp(32px, 7cqi, 56px); }
    .ph__actions { flex-wrap: nowrap; }
  }
}
@media (max-width: 699px) {
  .ph { flex-direction: column; align-items: center; gap: 20px; padding: 0 16px; text-align: center; }
  .ph__art { width: min(260px, 66vw); }
  .ph__text { align-items: center; gap: 6px; width: 100%; padding-bottom: 0; }
  /* Узкая колонка: до четырёх строк, перенос обычный — balance с обрезкой сдвигает последнюю строку */
  .ph__title { font-size: 26px; line-height: 1.12; letter-spacing: var(--tr-title); text-wrap: pretty; -webkit-line-clamp: 4; line-clamp: 4; }
  .ph__sub { justify-content: center; font-size: var(--fs-h4); }
  .ph__meta { justify-content: center; }
  /* Две главные кнопки поровну на всю ширину, как в Apple Music */
  .ph__actions { display: grid; grid-auto-columns: minmax(0, 1fr); grid-auto-flow: column; gap: 12px; width: 100%; margin-top: 14px; }
  .ph__actions > :deep(.btn) { min-width: 0; height: 48px; padding: 0 12px; gap: 8px; font-size: var(--fs-body); }
  .ph__actions > :deep(.btn .btn__label) { overflow: hidden; text-overflow: ellipsis; }
}
</style>
