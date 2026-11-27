<script setup lang="ts">
// «Подключить телефон» в обзоре админки: QR-код со ссылкой на этот сервер для приложения
const { t } = useI18n({ useScope: 'local' })
</script>

<template>
  <AdminSection :title="t('title')">
    <AuthAppQr />
  </AdminSection>
</template>

<i18n lang="json">
{
  "ru": { "title": "Подключить телефон" },
  "uk": { "title": "Під’єднати телефон" },
  "en": { "title": "Connect a phone" }
}
</i18n>
