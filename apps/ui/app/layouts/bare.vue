<script setup lang="ts">
// Без оболочки: вход, ожидание доступа, галерея компонентов. В приложении для компьютера — строка
// заголовка: за неё окно тянется (на macOS — вся ширина, «светофор» слева), на Windows — свои кнопки
const { win, titlebar, titlebarStyle } = useTitlebar()
</script>

<template>
  <div class="bare" :style="titlebar ? titlebarStyle : undefined">
    <ShellTitleBar v-if="titlebar === 'windows'" :focused="win.focused" :maximized="win.maximized" />
    <div v-else-if="titlebar === 'mac-inset'" class="bare__drag" data-tauri-drag-region="deep" />
    <slot />
  </div>
</template>

<style scoped>
.bare { position: relative; z-index: 1; min-height: 100vh; min-height: 100dvh; }
.bare__drag { position: fixed; top: 0; left: 0; right: 0; z-index: 80; height: var(--titlebar-size, 48px); }
</style>
