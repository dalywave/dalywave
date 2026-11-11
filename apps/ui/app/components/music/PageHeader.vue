<script setup lang="ts">
// Шапка страницы альбома, артиста, плейлиста: обложка со свечением её цвета, вид, название, подробности.
// meta — короткая строка «9 треков · 45 мин» (разделители — .ph-dot, не переносятся отдельно);
// status — состояние на сервере своей строкой, без точки; actions — один ряд: «Слушать», второе
// действие и круглые кнопки. На телефоне ряд не разваливается на две строки
withDefaults(defineProps<{
  kicker: string
  title: string
  /** Цвет свечения за шапкой */
  glow?: string | null
  round?: boolean
}>(), { glow: null })
</script>

<template>
  <header class="ph" :class="{ 'ph--round': round }" :style="glow ? { '--glow': glow } : undefined">
    <div class="ph__art"><slot name="art" /></div>
    <div class="ph__text">
      <div class="ph__kicker">{{ kicker }}</div>
      <h1 class="ph__title">{{ title }}</h1>
      <div v-if="$slots.sub" class="ph__sub"><slot name="sub" /></div>
      <div v-if="$slots.meta || $slots.status" class="ph__info">
        <div v-if="$slots.meta" class="ph__meta"><slot name="meta" /></div>
        <div v-if="$slots.status" class="ph__status"><slot name="status" /></div>
      </div>
      <div v-if="$slots.actions" class="ph__actions"><slot name="actions" /></div>
    </div>
  </header>
</template>

<style scoped>
.ph { position: relative; display: flex; align-items: flex-end; gap: 32px; padding: 24px 0 8px; isolation: isolate; }
.ph::before {
  content: '';
  position: absolute;
  inset: calc((var(--topbar-h) + 8px) * -1) -32px -40px;
  z-index: -1;
  background: radial-gradient(60% 90% at 18% 0%, color-mix(in oklab, var(--glow, var(--amb-1)) 34%, transparent), transparent 70%);
  pointer-events: none;
}
.ph__art { width: 232px; flex: none; }
.ph__art :deep(.art), .ph__art :deep(.pc) { width: 100%; border-radius: var(--r-4); box-shadow: var(--shadow-cover); }
.ph--round .ph__art :deep(.art) { border-radius: 50%; }
.ph__text { display: flex; flex-direction: column; gap: 8px; min-width: 0; padding-bottom: 4px; }
.ph__kicker { font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-2); }
.ph__title { margin: 0; font-size: clamp(32px, 4.4vw, var(--fs-display)); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.02; text-wrap: balance; overflow-wrap: anywhere; }
.ph__sub { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: var(--fs-h4); font-weight: 550; color: var(--text-1); }
.ph__sub :deep(a:hover) { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.ph__info { display: flex; flex-direction: column; gap: 4px; }
.ph__meta { display: flex; flex-wrap: wrap; align-items: center; gap: 2px 0; font-size: var(--fs-small); color: var(--text-2); }
/* Разделитель «·» держится за следующий пункт: строка не кончается точкой */
.ph__meta > :deep(*) { white-space: nowrap; }
.ph__meta > :deep(* + *)::before { content: '·'; margin: 0 8px; color: var(--text-4); }
.ph__status { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; font-size: var(--fs-small); color: var(--text-3); text-wrap: pretty; }
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
  .ph { flex-direction: column; align-items: center; gap: 18px; padding: 12px 16px 0; text-align: center; }
  .ph::before { inset: calc(var(--safe-top) * -1 - 8px) 0 -40px; background: radial-gradient(90% 70% at 50% 0%, color-mix(in oklab, var(--glow, var(--amb-1)) 36%, transparent), transparent 75%); }
  .ph__art { width: min(240px, 64vw); }
  .ph__text { align-items: center; width: 100%; }
  .ph__title { font-size: 28px; }
  .ph__sub, .ph__meta, .ph__status { justify-content: center; }
  .ph__info { align-items: center; }
  /* Один ряд: две кнопки делят ширину, круглые — справа */
  .ph__actions { flex-wrap: nowrap; justify-content: center; gap: 8px; width: 100%; margin-top: 10px; }
  .ph__actions > :deep(.btn) { flex: 1 1 auto; min-width: 0; max-width: 200px; height: 44px; padding: 0 14px; gap: 6px; font-size: var(--fs-body); }
  .ph__actions > :deep(.btn .btn__label) { overflow: hidden; text-overflow: ellipsis; }
  /* Четыре действия в ряд: второй кнопке хватает слова, значок уступает место */
  .ph__actions:has(> :nth-child(4)) > :deep(.btn:not(.btn--primary) .btn__icon) { display: none; }
}
@media (max-width: 379px) {
  .ph__actions > :deep(.btn:not(.btn--primary) .btn__icon) { display: none; }
}
</style>
