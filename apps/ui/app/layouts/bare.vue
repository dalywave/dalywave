<template>
  <div class="bare">
    <!-- Без оболочки: вход, ожидание доступа, галерея компонентов -->
    <slot />
  </div>
</template>

<style scoped>
.bare { position: relative; z-index: 1; min-height: 100vh; min-height: 100dvh; }
</style>
