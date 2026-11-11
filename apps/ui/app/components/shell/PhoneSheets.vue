<script setup lang="ts">
// Телефон: очередь, текст, устройства и «о треке» — шторками снизу
const ui = useUiStore()
const player = usePlayerStore()
const { t } = useI18n()

const open = computed({
  get: () => ui.sheetTab != null,
  set: (v: boolean) => {
    if (!v) ui.sheetTab = null
  },
})
const title = computed(() => (ui.sheetTab ? t(`player.${ui.sheetTab}`) : ''))
watch(() => ui.sheetTab, (tab) => {
  if (tab === 'devices') void player.loadDevices()
})
</script>

<template>
  <UiSheet v-model:open="open" :title="title" side="bottom">
    <div class="pad">
      <MusicQueuePanel v-if="ui.sheetTab === 'queue'" />
      <UiEmptyState v-else-if="ui.sheetTab === 'lyrics'" icon="ph:microphone-stage" :title="t('lyrics.title')" :text="t('lyrics.text')" />
      <div v-else-if="ui.sheetTab === 'devices'">
        <MusicDeviceList :devices="player.devices" :playing="player.playing" @select="player.selectDevice" />
        <p class="note">{{ t('devices.note') }}</p>
      </div>
      <MusicAboutTrack v-else-if="ui.sheetTab === 'about'" :track="player.current?.track ?? null" />
    </div>
  </UiSheet>
</template>

<style scoped>
.pad { padding: 0 8px 16px; }
.note { margin: 12px 10px 0; font-size: var(--fs-caption); color: var(--text-3); }
</style>
