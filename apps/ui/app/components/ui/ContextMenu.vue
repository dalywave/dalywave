<script setup lang="ts">
import { ContextMenuContent, ContextMenuPortal, ContextMenuRoot, ContextMenuTrigger } from 'reka-ui'
import type { MenuEntry } from '~/types/ui'

// Правый клик (или долгое нажатие) по области. Пункты могут зависеть от того, по чему кликнули:
// items вычисляются в момент открытия. На телефоне и под пальцем — лист снизу
defineProps<{ items: MenuEntry[] }>()
const emit = defineEmits<{ open: [target: EventTarget | null] }>()

const sheet = useMenuSheet()
const bridge = useBridge()
const sheetOpen = ref(false)
let pressed: EventTarget | null = null

function onContext(e: MouseEvent) {
  emit('open', e.target)
}

// Долгое нажатие на iOS не присылает contextmenu: цель берём из последнего касания
function onPress(e: PointerEvent) {
  pressed = e.target
}

async function onOpenRequest(v: boolean) {
  if (!v) return
  // Долгое нажатие открыло меню — отклик пальцу (iPhone)
  bridge.haptic('medium')
  emit('open', pressed)
  await nextTick()
  sheetOpen.value = true
}
</script>

<template>
  <ContextMenuRoot v-if="sheet" :open="false" :modal="false" @update:open="onOpenRequest">
    <ContextMenuTrigger as-child @pointerdown.capture="onPress" @contextmenu.capture="onPress">
      <slot />
    </ContextMenuTrigger>
  </ContextMenuRoot>
  <ContextMenuRoot v-else :modal="false">
    <ContextMenuTrigger as-child @contextmenu.capture="onContext">
      <slot />
    </ContextMenuTrigger>
    <ContextMenuPortal>
      <ContextMenuContent v-if="items.length" class="menu" :collision-padding="12">
        <UiMenuItems :items="items" context />
      </ContextMenuContent>
    </ContextMenuPortal>
  </ContextMenuRoot>
  <UiActionSheet v-if="sheet && items.length" v-model:open="sheetOpen" :items="items" detached />
</template>
