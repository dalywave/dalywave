<script setup lang="ts">
import { DropdownMenuContent, DropdownMenuPortal, DropdownMenuRoot, DropdownMenuTrigger } from 'reka-ui'
import type { MenuEntry } from '~/types/ui'

// Меню по кнопке. Телефон или палец — лист снизу (UiActionSheet), мышь — выпадающее меню
withDefaults(defineProps<{
  items: MenuEntry[]
  side?: 'top' | 'bottom' | 'left' | 'right'
  align?: 'start' | 'center' | 'end'
}>(), { side: 'bottom', align: 'end' })

const open = defineModel<boolean>('open', { default: false })
const sheet = useMenuSheet()
</script>

<template>
  <UiActionSheet v-if="sheet" v-model:open="open" :items="items">
    <slot />
  </UiActionSheet>
  <DropdownMenuRoot v-else v-model:open="open" :modal="false">
    <DropdownMenuTrigger as-child>
      <slot />
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent class="menu" :side="side" :align="align" :side-offset="6" :collision-padding="12">
        <UiMenuItems :items="items" />
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
