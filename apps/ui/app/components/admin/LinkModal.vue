<script setup lang="ts">
import { copyText } from '~/utils/web-api'
// Одноразовая ссылка (приглашение, новый пароль): показывается один раз — скопировать и передать
// человеку самому. Почты сервер не рассылает
defineProps<{
  title: string
  text: string
  url: string
}>()
const open = defineModel<boolean>('open', { default: false })
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })

async function copy(url: string) {
  try {
    await copyText(url)
    toast.show({ text: t('copied'), icon: 'ph:link' })
  }
  catch {
    toast.error(t('copyFailed'))
  }
}
</script>

<template>
  <UiModal v-model:open="open" :title="title" :width="480">
    <div class="lm">
      <p class="lm__text">{{ text }}</p>
      <input class="lm__url" :value="url" readonly :aria-label="title" @focus="($event.target as HTMLInputElement).select()">
      <UiButton variant="primary" size="lg" block icon="ph:copy" @click="copy(url)">{{ t('copy') }}</UiButton>
    </div>
  </UiModal>
</template>

<style scoped>
.lm { display: flex; flex-direction: column; gap: 12px; padding: 0 20px 16px; }
.lm__text { margin: 0; color: var(--text-2); font-size: var(--fs-small); line-height: 1.45; }
.lm__url {
  width: 100%;
  height: 44px;
  padding: 0 16px;
  border: 0;
  border-radius: var(--r-2);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 13px;
}
.lm__url:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
</style>

<i18n lang="json">
{
  "ru": { "copy": "Скопировать", "copied": "Ссылка скопирована", "copyFailed": "Не получилось скопировать — выделите ссылку вручную" },
  "uk": { "copy": "Скопіювати", "copied": "Посилання скопійовано", "copyFailed": "Не вдалося скопіювати — виділіть посилання вручну" },
  "en": { "copy": "Copy", "copied": "Link copied", "copyFailed": "Couldn't copy — select the link manually" }
}
</i18n>
