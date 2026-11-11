<script setup lang="ts">
// Пункты меню — общие для выпадающего и контекстного: одинаковый вид везде
import {
  ContextMenuItem, ContextMenuLabel, ContextMenuPortal, ContextMenuSeparator, ContextMenuSub, ContextMenuSubContent, ContextMenuSubTrigger,
  DropdownMenuItem, DropdownMenuLabel, DropdownMenuPortal, DropdownMenuSeparator, DropdownMenuSub, DropdownMenuSubContent, DropdownMenuSubTrigger,
} from 'reka-ui'
import type { MenuAction, MenuEntry } from '~/types/ui'

const props = defineProps<{
  items: MenuEntry[]
  context?: boolean
}>()

const P = computed(() => props.context
  ? { Item: ContextMenuItem, Label: ContextMenuLabel, Separator: ContextMenuSeparator, Sub: ContextMenuSub, SubTrigger: ContextMenuSubTrigger, SubContent: ContextMenuSubContent, Portal: ContextMenuPortal }
  : { Item: DropdownMenuItem, Label: DropdownMenuLabel, Separator: DropdownMenuSeparator, Sub: DropdownMenuSub, SubTrigger: DropdownMenuSubTrigger, SubContent: DropdownMenuSubContent, Portal: DropdownMenuPortal })

const isAction = (e: MenuEntry): e is MenuAction => !e.kind || e.kind === 'item'
</script>

<template>
  <template v-for="(entry, i) in items" :key="i">
    <component :is="P.Separator" v-if="entry.kind === 'separator'" class="menu__sep" />
    <component :is="P.Label" v-else-if="entry.kind === 'heading'" class="menu__head">
      <span class="menu__head-title ellipsis">{{ entry.label }}</span>
      <span v-if="entry.hint" class="menu__head-hint ellipsis">{{ entry.hint }}</span>
    </component>
    <template v-else-if="isAction(entry)">
      <component :is="P.Sub" v-if="entry.children?.length">
        <component :is="P.SubTrigger" class="menu__item" :disabled="entry.disabled">
          <Icon v-if="entry.icon" :name="entry.icon" class="menu__icon" />
          <span class="menu__label">{{ entry.label }}</span>
          <Icon name="ph:caret-right" class="menu__end" />
        </component>
        <component :is="P.Portal">
          <component :is="P.SubContent" class="menu" :side-offset="6" :collision-padding="12">
            <UiMenuItems :items="entry.children" :context="context" />
          </component>
        </component>
      </component>
      <component
        :is="P.Item"
        v-else
        class="menu__item"
        :class="{ 'is-danger': entry.danger, 'is-checked': entry.checked }"
        :disabled="entry.disabled"
        @select="entry.onSelect?.()"
      >
        <Icon v-if="entry.icon" :name="entry.icon" class="menu__icon" />
        <span class="menu__label">
          {{ entry.label }}
          <span v-if="entry.hint" class="menu__desc">{{ entry.hint }}</span>
        </span>
        <UiKbd v-if="entry.kbd" :keys="entry.kbd" class="menu__end" />
        <Icon v-else-if="entry.checked" name="ph:check" class="menu__end menu__check" />
      </component>
    </template>
  </template>
</template>

<style>
.menu {
  z-index: 220;
  min-width: 240px;
  max-width: min(340px, calc(100vw - 24px));
  padding: 6px;
  border-radius: var(--r-4);
  background: var(--glass-strong);
  -webkit-backdrop-filter: blur(32px) saturate(1.6);
  backdrop-filter: blur(32px) saturate(1.6);
  box-shadow: var(--shadow-pop);
  animation: pop-in var(--dur-2) var(--ease-out);
}
.menu__item {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 36px;
  padding: 6px 10px;
  border-radius: 10px;
  font-size: var(--fs-body);
  color: var(--text-1);
  outline: none;
  cursor: default;
  user-select: none;
}
.menu__item[data-highlighted] { background: var(--fill-2); }
.menu__item[data-disabled] { color: var(--text-4); pointer-events: none; }
.menu__item.is-danger, .menu__item.is-danger .menu__icon { color: var(--danger); }
.menu__icon { font-size: 18px; color: var(--text-2); flex: none; }
.menu__item.is-checked .menu__icon { color: var(--accent); }
.menu__label { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.menu__desc { font-size: var(--fs-caption); color: var(--text-3); line-height: 1.35; }
.menu__end { margin-left: auto; color: var(--text-3); flex: none; font-size: 16px; }
.menu__check { color: var(--accent); }
.menu__sep { height: 1px; margin: 6px 4px; background: var(--hair); }
.menu__head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; padding: 8px 10px 8px; min-width: 0; }
.menu__head-title { font-size: var(--fs-small); font-weight: 650; color: var(--text-1); }
.menu__head-hint { font-size: var(--fs-caption); color: var(--text-3); }
</style>
