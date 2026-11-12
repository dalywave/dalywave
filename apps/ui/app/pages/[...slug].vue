<script setup lang="ts">
// Неизвестный адрес: остаёмся в оболочке (боковая панель, плеер), чтобы было куда уйти
const { t } = useI18n()
useHead({ title: () => t('common.notFound') })
</script>

<template>
  <div class="nf">
    <MusicPhoneNav fallback="/" />
    <UiEmptyState icon="ph:compass" :title="t('common.notFound')" :text="t('common.notFoundText')">
      <UiButton to="/" variant="primary" icon="ph:house">{{ t('nav.home') }}</UiButton>
    </UiEmptyState>
  </div>
</template>

<style scoped>
.nf { display: grid; place-items: center; min-height: calc(100vh - var(--topbar-h) - var(--bar-h) - 96px); }
@media (max-width: 699px) {
  .nf { display: flex; flex-direction: column; justify-content: center; min-height: calc(100dvh - var(--tabbar-h) - var(--mini-h) - 64px); }
  .nf > .pn { position: absolute; top: 0; left: 0; right: 0; margin-top: 0; }
}
</style>
