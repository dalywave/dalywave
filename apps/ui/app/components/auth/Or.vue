<script setup lang="ts">
// «или» между формой пароля и кнопками провайдеров: волосок с подписью по центру.
// label — своя подпись, например «или своим аккаунтом» под кнопкой демо
defineProps<{ label?: string }>()
const { t } = useScopedI18n('components.auth.Or')
</script>

<template>
  <p class="or"><span>{{ label ?? t('or') }}</span></p>
</template>

<style scoped>
.or { display: flex; align-items: center; gap: 12px; margin: 6px 0; color: var(--text-4); font-size: var(--fs-caption); }
.or::before, .or::after { content: ''; flex: 1; height: 1px; background: var(--line); }
</style>
