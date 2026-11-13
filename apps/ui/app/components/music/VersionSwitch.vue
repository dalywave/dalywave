<script setup lang="ts">
import type { VersionKind } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Версия играющей песни: маленькая подпись «Slowed ⌄» у исполнителя, по нажатию — «Оригинал / Slowed /
// Super slowed / Sped up» (меню, на телефоне — лист снизу). Только версии, выпущенные самим
// исполнителем; других нет — подписи нет. Отдельного ряда в плеере не занимает
const props = withDefaults(defineProps<{ size?: 'sm' | 'md' }>(), { size: 'sm' })
// Класс места — на саму подпись: у меню своего элемента нет
defineOptions({ inheritAttrs: false })

const versions = useVersionsStore()
const { t } = useI18n()

/** Пока версия качается, подписана уже она — и мерцает */
const selected = computed<VersionKind>(() => versions.waiting?.version.kind ?? versions.current?.kind ?? 'original')

const menuOpen = ref(false)
const menuItems = computed<MenuEntry[]>(() => [
  { kind: 'heading', label: t('versions.label') },
  ...versions.options.map(o => ({
    label: versions.label(o.kind),
    checked: o.kind === selected.value,
    onSelect: () => void versions.switchTo(o.kind),
  })),
])
</script>

<template>
  <UiMenu v-if="versions.available" v-model:open="menuOpen" :items="menuItems" side="top" align="start">
    <button
      v-bind="$attrs"
      type="button"
      class="vs"
      :class="[`vs--${props.size}`, { 'is-waiting': !!versions.waiting }]"
      :aria-label="`${t('versions.label')}: ${versions.label(selected)}`"
      data-no-dismiss
    >
      <span>{{ versions.label(selected) }}</span>
      <Icon name="ph:caret-up-down" class="vs__caret" />
    </button>
  </UiMenu>
</template>

<style scoped>
.vs {
  position: relative;
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 3px;
  height: 20px;
  padding: 0 6px 0 8px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  color: var(--text-2);
  font: inherit;
  font-size: var(--fs-caption);
  font-weight: 600;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition: color var(--dur-1), background-color var(--dur-2);
}
.vs--md { height: 24px; padding: 0 8px 0 10px; font-size: 13px; }
/* Под пальцем подпись маленькая, а место для нажатия — не меньше 44 pt */
.vs--md::after { content: ''; position: absolute; inset: -10px -6px; }
.vs:hover, .vs[data-state='open'] { color: var(--text-1); background: var(--fill-2); }
.vs:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.vs__caret { font-size: 11px; color: var(--text-3); }
.vs.is-waiting span { animation: vs-blink 1.1s var(--ease-out) infinite alternate; }

@keyframes vs-blink { from { opacity: 1; } to { opacity: 0.35; } }
@media (prefers-reduced-motion: reduce) {
  .vs.is-waiting span { animation: none; }
}
</style>
