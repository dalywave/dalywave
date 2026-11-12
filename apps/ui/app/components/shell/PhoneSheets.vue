<script setup lang="ts">
// Телефон: очередь, текст, устройства и «о треке» — листами снизу. Открываются до половины
// экрана (или по содержимому), вверх — во весь экран; при переходе на другую страницу закрываются
const ui = useUiStore()
const player = usePlayerStore()
const route = useRoute()
const { t } = useI18n()

const open = computed({
  get: () => ui.sheetTab != null,
  set: (v: boolean) => {
    if (!v) ui.sheetTab = null
  },
})
// Пока лист уезжает, содержимое остаётся прежним
const tab = ref(ui.sheetTab)
watch(() => ui.sheetTab, (v) => {
  if (v) tab.value = v
  if (v === 'devices') void player.loadDevices()
})
watch(() => route.path, () => (ui.sheetTab = null))
const title = computed(() => (tab.value ? t(`player.${tab.value}`) : ''))
</script>

<template>
  <UiSheet v-model:open="open" :title="title" detents="expand">
    <MusicDeviceList v-if="tab === 'devices'" :devices="player.devices" :playing="player.playing" variant="sheet" @select="player.selectDevice" />
    <div v-else class="pad">
      <MusicQueuePanel v-if="tab === 'queue'" />
      <MusicLyricsView v-else-if="tab === 'lyrics'" variant="sheet" />
      <MusicAboutTrack v-else-if="tab === 'about'" :track="player.current?.track ?? null" />
    </div>
  </UiSheet>
</template>

<style scoped>
.pad { padding: 0 8px 16px; }
</style>
