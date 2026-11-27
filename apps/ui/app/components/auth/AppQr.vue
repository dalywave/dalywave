<script setup lang="ts">
// QR-код «Подключить телефон»: ссылка `dalywave://server?url=<этот сервер>`. Камера iPhone узнаёт её и
// открывает DalyWave с экраном «Адрес сервера», где адрес уже подставлен. Экран входа сайта и админка
const props = defineProps<{ compact?: boolean }>()
const { t } = useI18n({ useScope: 'local' })

const origin = typeof location !== 'undefined' ? location.origin : ''
const link = computed(() => `dalywave://server?url=${encodeURIComponent(origin)}`)
const host = computed(() => origin.replace(/^https?:\/\//, ''))
</script>

<template>
  <div class="aq" :class="{ 'aq--compact': props.compact }">
    <MusicJamQr :value="link" :label="t('label', { host })" class="aq__code" />
    <div class="aq__text">
      <p class="aq__title">{{ t('title') }}</p>
      <p class="aq__lead">{{ t('lead') }}</p>
      <p class="aq__host tnum">{{ host }}</p>
    </div>
  </div>
</template>

<style scoped>
.aq { display: flex; align-items: center; gap: 20px; text-align: left; }
.aq__code { flex: none; width: 140px; border-radius: 16px; }
.aq__text { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.aq__title { margin: 0; font-size: 15px; font-weight: 650; color: var(--text-1); }
.aq__lead { margin: 0; font-size: var(--fs-small); line-height: 1.45; color: var(--text-2); }
.aq__host { margin: 4px 0 0; font-size: var(--fs-caption); color: var(--text-3); overflow-wrap: anywhere; }
.aq--compact { flex-direction: column; text-align: center; }
.aq--compact .aq__text { align-items: center; }
.aq--compact .aq__code { width: 180px; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "DalyWave на телефоне",
    "lead": "Наведите камеру iPhone — приложение откроется с адресом этого сервера",
    "label": "QR-код: подключить приложение к {host}"
  },
  "uk": {
    "title": "DalyWave на телефоні",
    "lead": "Наведіть камеру iPhone — застосунок відкриється з адресою цього сервера",
    "label": "QR-код: під’єднати застосунок до {host}"
  },
  "en": {
    "title": "DalyWave on your phone",
    "lead": "Point the iPhone camera here — the app opens with this server's address",
    "label": "QR code: connect the app to {host}"
  }
}
</i18n>
