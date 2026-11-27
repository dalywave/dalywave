<script setup lang="ts">
import type { ClipReportItem, ServerSettings } from '~/api/types'

// Клипы: настройки (качество, кусок или целиком в такт с треком, сроки, потолок, автопоиск) и
// клипы, на которые пожаловались слушатели, — выбрать другой можно прямо отсюда
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const clipSheet = useClipSheet()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })
const { failed, attempt } = useAttempts()

const settings = ref<ServerSettings | null>(null)
const reports = ref<ClipReportItem[] | null>(null)
const load = () => attempt('settings', () => bridge.admin.settings(), v => (settings.value = v))
const loadReports = () => attempt('reports', () => bridge.admin.clipReports(), v => (reports.value = v.items))
onMounted(() => {
  void load()
  void loadReports()
})
</script>

<template>
  <div class="ap cl">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminSection v-if="reports?.length" :title="t('reports.title')" :hint="t('reports.hint')">
      <div class="ap-table">
        <button v-for="c in reports" :key="c.track.id" type="button" class="ap-tr ap-tr--btn cl__row" @click="clipSheet.show(c.track)">
          <MusicCoverArt class="cl__cover" :image="c.track.album?.image" size="small" radius="var(--r-1)" />
          <span class="ap-cell"><b>{{ c.track.title }}</b><small>{{ artistNames(c.track.artists) }}</small></span>
          <span class="ap-num ap-warn tnum">{{ t('reports.n', { n: c.reports }) }}</span>
        </button>
      </div>
    </AdminSection>

    <AdminState :loading="!settings && !failed.settings" :error="failed.settings && !settings" :rows="6" @retry="load">
      <AdminClipSettings v-if="settings" :clips="settings.clips" @saved="c => settings && (settings.clips = c)" />
    </AdminState>
    <AdminEnvList section="clips" />
  </div>
</template>

<style scoped>
.cl { max-width: 860px; }
.cl__row { --cols: 40px minmax(0, 1fr) auto; }
.cl__cover { width: 40px; height: 40px; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Клипы",
    "reports": { "title": "Жалобы на клипы", "hint": "Слушатели скрыли эти клипы у себя — выберите другой или оставьте без клипа", "n": "скрыли: {n}" }
  },
  "uk": {
    "title": "Кліпи",
    "reports": { "title": "Скарги на кліпи", "hint": "Слухачі приховали ці кліпи в себе — виберіть інший або залиште без кліпу", "n": "приховали: {n}" }
  },
  "en": {
    "title": "Clips",
    "reports": { "title": "Clip reports", "hint": "Listeners hid these clips — pick another one or leave the track without a clip", "n": "hidden: {n}" }
  }
}
</i18n>
