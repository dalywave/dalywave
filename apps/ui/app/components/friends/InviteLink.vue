<script setup lang="ts">
import { copyText } from '~/utils/web-api'
// Ссылка-приглашение с QR-кодом: «Добавить в друзья» и вход в совместный плейлист. Как у «Слушать
// вместе»: QR для телефона рядом, ссылка — скопировать или отправить; «Новая ссылка» — прежняя
// перестаёт работать
const props = defineProps<{ url: string, hint: string, shareTitle: string, busy?: boolean }>()
const emit = defineEmits<{ reset: [] }>()

const bridge = useBridge()
const ui = useUiStore()
const toast = useToast()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

const shortUrl = computed(() => props.url.replace(/^https?:\/\//, ''))
const canShare = computed(() => import.meta.client && typeof navigator.share === 'function' && (ui.isPhone || bridge.platform === 'ios' || bridge.platform === 'ipados'))

async function copy() {
  try {
    if (bridge.kind === 'tauri' && bridge.isDesktopApp) {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('plugin:clipboard-manager|write_text', { text: props.url })
    }
    else await copyText(props.url)
    bridge.haptic('light')
    toast.show({ text: tg('share.copied'), icon: 'ph:link' })
  }
  catch {
    toast.error(tg('share.failed'))
  }
}

async function share() {
  try {
    await navigator.share({ title: props.shareTitle, url: props.url })
  }
  catch (e) {
    if (!(e instanceof DOMException && e.name === 'AbortError')) void copy()
  }
}
</script>

<template>
  <section class="il">
    <MusicJamQr :value="url" :label="t('qr')" class="il__qr" />
    <p class="il__hint">{{ hint }}</p>
    <div class="il__link">
      <span class="il__url ellipsis" :title="url">{{ shortUrl }}</span>
      <UiButton v-if="canShare" size="sm" icon="ph:export" @click="share">{{ t('share') }}</UiButton>
      <UiButton v-else size="sm" icon="ph:copy" @click="copy">{{ t('copy') }}</UiButton>
    </div>
    <UiButton variant="ghost" size="sm" icon="ph:arrows-clockwise" :loading="busy" class="il__reset" @click="emit('reset')">{{ t('reset') }}</UiButton>
  </section>
</template>

<style scoped>
.il { display: flex; flex-direction: column; align-items: center; gap: 12px; }
.il__qr { width: min(208px, 60vw); }
.il__hint { max-width: 300px; margin: 0; font-size: var(--fs-small); color: var(--text-2); text-align: center; text-wrap: pretty; }
.il__link { display: flex; align-items: center; gap: 10px; width: 100%; min-width: 0; padding: 6px 6px 6px 14px; border-radius: var(--r-pill); background: var(--fill-1); }
.il__url { flex: 1; min-width: 0; font-size: var(--fs-small); color: var(--text-2); }
.il__reset { color: var(--text-3); }
@media (pointer: coarse) {
  .il__link :deep(.btn), .il__reset { min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "qr": "QR-код ссылки",
    "copy": "Скопировать",
    "share": "Отправить",
    "reset": "Новая ссылка"
  },
  "en": {
    "qr": "QR code with the link",
    "copy": "Copy",
    "share": "Send",
    "reset": "New link"
  }
}
</i18n>
