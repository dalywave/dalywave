<script setup lang="ts">
import { ContextMenuContent, ContextMenuPortal, ContextMenuRoot, ContextMenuTrigger } from 'reka-ui'
import type { MenuEntry } from '~/types/ui'

// Правый клик (или долгое нажатие) по области. Пункты могут зависеть от того, по чему кликнули:
// items вычисляются в момент открытия
defineProps<{ items: MenuEntry[] }>()
const emit = defineEmits<{ open: [target: EventTarget | null] }>()

function onContext(e: MouseEvent) {
  emit('open', e.target)
}
</script>

<template>
  <ContextMenuRoot :modal="false">
    <ContextMenuTrigger as-child @contextmenu.capture="onContext">
      <slot />
    </ContextMenuTrigger>
    <ContextMenuPortal>
      <ContextMenuContent v-if="items.length" class="menu" :collision-padding="12">
        <UiMenuItems :items="items" context />
      </ContextMenuContent>
    </ContextMenuPortal>
  </ContextMenuRoot>
</template>
