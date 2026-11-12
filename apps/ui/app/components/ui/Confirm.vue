<script setup lang="ts">
// Подтверждение необратимого (удалить плейлист). Одно на приложение: вызывается через
// usePlaylistUi().confirm(...) и отвечает промисом
const pui = usePlaylistUi()
const ui = useUiStore()
const { tg } = useGlobalT()
</script>

<template>
  <UiModal v-model:open="pui.confirmOpen" :title="pui.confirmReq?.title ?? ''" :width="380">
    <div class="cf">
      <p v-if="pui.confirmReq?.text" class="cf__text">{{ pui.confirmReq.text }}</p>
      <div class="cf__actions">
        <UiButton size="lg" block @click="pui.answer(false)">{{ tg('common.cancel') }}</UiButton>
        <UiButton size="lg" block :variant="pui.confirmReq?.danger ? 'danger' : 'primary'" :autofocus="!ui.isPhone || undefined" @click="pui.answer(true)">{{ pui.confirmReq?.action }}</UiButton>
      </div>
    </div>
  </UiModal>
</template>

<style scoped>
.cf { display: flex; flex-direction: column; gap: 20px; padding: 0 20px 20px; }
.cf__text { margin: 0; font-size: var(--fs-body); color: var(--text-2); text-wrap: pretty; }
.cf__actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
@media (max-width: 699px) {
  .cf { padding-bottom: 12px; }
  .cf__actions { grid-template-columns: 1fr; }
  .cf__actions > :last-child { order: -1; }
}
</style>
