<script setup lang="ts">
// «Подключить телефон» в обзоре админки: QR-код со ссылкой на этот сервер для приложения
const { t } = useScopedI18n('components.admin.ConnectApp')
</script>

<template>
  <AdminSection :title="t('title')">
    <AuthAppQr />
  </AdminSection>
</template>
