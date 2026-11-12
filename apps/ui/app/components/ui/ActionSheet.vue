<script setup lang="ts">
import type { MenuAction, MenuEntry, MenuHeading } from '~/types/ui'

// Меню на телефоне — лист снизу (UiSheet), как в «Музыке» и Spotify: шапка с обложкой,
// крупные строки группами, опасное красным, отдельная «Отмена». Жесты — общие для всех листов:
// смахнуть вниз или нажать мимо — закрыть. Пункты те же, что у выпадающего меню на большом экране
const props = defineProps<{
  items: MenuEntry[]
  /** Без кнопки-триггера: лист открывают снаружи (долгое нажатие на строку) */
  detached?: boolean
}>()

const open = defineModel<boolean>('open', { default: false })
const { t } = useI18n()
// Пункты часто считаются только пока меню открыто: пока лист уезжает вниз, держим последние
const items = shallowRef<MenuEntry[]>(props.items)
watch(() => props.items, (v) => {
  if (v.length) items.value = v
})

type Row = { kind: 'row', entry: MenuAction } | { kind: 'caption', label: string }
const stack = ref<MenuAction[]>([])
watch(open, (v) => {
  if (!v) stack.value = []
})

const level = computed(() => stack.value.at(-1)?.children ?? items.value)
const header = computed<MenuHeading | null>(() => {
  if (stack.value.length) return null
  const first = items.value.find(e => e.kind !== 'separator')
  return first?.kind === 'heading' ? first : null
})

// Пункты → группы: разделитель начинает новую группу, подзаголовок — подпись группы
const groups = computed(() => {
  const out: Row[][] = [[]]
  for (const e of level.value) {
    if (e === header.value) continue
    if (e.kind === 'separator') {
      if (out.at(-1)!.length) out.push([])
    }
    else if (e.kind === 'heading') {
      if (out.at(-1)!.length) out.push([])
      out.at(-1)!.push({ kind: 'caption', label: e.label })
    }
    else out.at(-1)!.push({ kind: 'row', entry: e })
  }
  return out.filter(g => g.some(r => r.kind === 'row'))
})

function pick(e: MenuAction) {
  if (e.disabled) return
  if (e.children?.length) {
    stack.value.push(e)
    return
  }
  open.value = false
  e.onSelect?.()
}

</script>

<template>
  <UiSheet v-model:open="open" :title="header?.label ?? stack.at(-1)?.label ?? t('common.more')" :description="header?.hint" :show-title="false" inset>
    <template v-if="!detached" #trigger>
      <slot />
    </template>
    <template #head>
      <header v-if="header" class="as__head">
        <MusicCoverArt v-if="header.image !== undefined" class="as__cover" :image="header.image" size="small" :round="header.round" />
        <div class="as__head-text">
          <div class="as__title ellipsis">{{ header.label }}</div>
          <div v-if="header.hint" class="as__sub ellipsis">{{ header.hint }}</div>
        </div>
      </header>
      <header v-else-if="stack.length" class="as__head as__head--back">
        <button type="button" class="as__back" @click="stack.pop()">
          <Icon name="ph:caret-left" />{{ t('nav.back') }}
        </button>
        <div class="as__title ellipsis">{{ stack.at(-1)!.label }}</div>
      </header>
      <div v-else class="as__gap" />
    </template>
    <div v-for="(g, gi) in groups" :key="gi" class="as__group">
      <template v-for="(r, ri) in g" :key="ri">
        <div v-if="r.kind === 'caption'" class="as__caption">{{ r.label }}</div>
        <button
          v-else
          type="button"
          class="as__row"
          :class="{ 'is-danger': r.entry.danger, 'is-checked': r.entry.checked, 'no-icon': !r.entry.icon }"
          :disabled="r.entry.disabled || undefined"
          @click="pick(r.entry)"
        >
          <Icon v-if="r.entry.icon" :name="r.entry.icon" class="as__icon" />
          <span class="as__label">
            <span class="ellipsis">{{ r.entry.label }}</span>
            <span v-if="r.entry.hint" class="as__hint ellipsis">{{ r.entry.hint }}</span>
          </span>
          <Icon v-if="r.entry.children?.length" name="ph:caret-right" class="as__end" />
          <Icon v-else-if="r.entry.checked" name="ph:check-bold" class="as__end as__check" />
        </button>
      </template>
    </div>
    <template #footer>
      <button type="button" class="as__cancel" @click="open = false">{{ t('common.cancel') }}</button>
    </template>
  </UiSheet>
</template>

<style>
.as__head { display: flex; align-items: center; gap: 12px; min-width: 0; padding: 10px 16px 14px; flex: none; border-bottom: 0.5px solid rgba(var(--hi-rgb), 0.1); }
.as__cover { flex: none; }
.as__cover.art { width: 48px; border-radius: 8px; }
.as__cover.art--round { border-radius: 50%; }
.as__head-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.as__title { font-size: 16px; font-weight: 650; color: var(--text-1); }
.as__sub { font-size: 14px; color: var(--text-2); }
.as__head--back { gap: 8px; padding-top: 6px; padding-bottom: 10px; }
.as__back { display: inline-flex; align-items: center; gap: 2px; height: 36px; margin-left: -6px; padding: 0 8px 0 0; border: 0; background: none; font-size: 16px; color: var(--accent); flex: none; }
.as__group + .as__group { border-top: 6px solid rgba(var(--shade-rgb), calc(0.22 * var(--shade-k))); }
.as__caption { padding: 12px 16px 4px; font-size: 12px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.as__row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  min-height: 54px;
  padding: 8px 16px;
  border: 0;
  background: transparent;
  font-size: 17px;
  text-align: left;
  color: var(--text-1);
  -webkit-tap-highlight-color: transparent;
}
.as__row + .as__row::before { content: ''; position: absolute; top: 0; left: 52px; right: 0; height: 0.5px; background: rgba(var(--hi-rgb), 0.1); }
/* Волосок начинается там, где текст: без значка — от левого поля */
.as__row.no-icon::before { left: 16px; }
.as__row:active { background: rgba(var(--hi-rgb), 0.08); }
.as__row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.as__row:disabled { color: var(--text-4); }
.as__row.is-danger, .as__row.is-danger .as__icon { color: var(--danger); }
.as__icon { flex: none; width: 22px; font-size: 22px; color: var(--text-2); }
.as__row.is-checked .as__icon { color: var(--accent); }
.as__label { display: flex; flex-direction: column; flex: 1; min-width: 0; }
.as__hint { font-size: 13px; color: var(--text-3); }
.as__end { flex: none; font-size: 18px; color: var(--text-3); }
.as__check { color: var(--accent); }
.as__gap { height: 6px; }
.as__cancel {
  display: block;
  width: 100%;
  height: 56px;
  border: 0;
  background: transparent;
  font-size: 17px;
  font-weight: 650;
  color: var(--text-1);
  -webkit-tap-highlight-color: transparent;
}
.as__cancel:active { background: rgba(var(--hi-rgb), 0.08); }
.as__cancel:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
</style>
