<script setup lang="ts">
// «или» между формой пароля и кнопками провайдеров: волосок с подписью по центру
const { t } = useI18n({ useScope: 'local' })
</script>

<template>
  <p class="or"><span>{{ t('or') }}</span></p>
</template>

<style scoped>
.or { display: flex; align-items: center; gap: 12px; margin: 6px 0; color: var(--text-4); font-size: var(--fs-caption); }
.or::before, .or::after { content: ''; flex: 1; height: 1px; background: var(--line); }
</style>

<i18n lang="json">
{
  "ru": { "or": "или" },
  "en": { "or": "or" }
}
</i18n>
