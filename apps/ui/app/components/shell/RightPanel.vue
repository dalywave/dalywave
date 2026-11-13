<script setup lang="ts">
import type { PanelTab } from '~/stores/ui'

// Правая панель большого экрана: очередь, текст, устройства, о треке
const ui = useUiStore()
const player = usePlayerStore()
const { t } = useI18n()

const tabs = computed(() => [
  { value: 'queue' as PanelTab, label: t('player.queue') },
  { value: 'lyrics' as PanelTab, label: t('player.lyrics') },
  { value: 'devices' as PanelTab, label: t('player.devices') },
  { value: 'about' as PanelTab, label: t('player.about') },
])
// «Слушать вместе» — внутри «Устройств», как Jam в списке устройств Spotify: вкладка та же
const tab = computed({
  get: () => (ui.panelTab === 'jam' ? 'devices' : ui.panelTab),
  set: (v: PanelTab) => (ui.panelTab = v),
})

watch(() => [ui.panelOpen, ui.panelTab], ([open, tab]) => {
  if (open && tab === 'devices') void player.loadDevices()
}, { immediate: true })
</script>

<template>
  <aside class="panel" :class="{ 'is-open': ui.panelOpen }" :aria-hidden="!ui.panelOpen" :inert="!ui.panelOpen || undefined" :aria-label="t('player.panel')">
    <div class="panel__inner">
      <header class="panel__head">
        <!-- Друзья — своя панель (кнопка в верхней строке), без вкладок плеера -->
        <h2 v-if="ui.panelTab === 'friends'" class="panel__title">{{ t('friends.panel') }}</h2>
        <UiTabs v-else v-model="tab" :items="tabs" :label="t('player.panel')" class="panel__tabs" />
        <UiIconButton icon="ph:x" :label="t('common.close')" size="sm" side="left" @click="ui.panelOpen = false" />
      </header>
      <div class="panel__body thin-scrollbar" :class="{ 'is-lyrics': ui.panelTab === 'lyrics' }">
        <MusicQueuePanel v-if="ui.panelTab === 'queue'" />
        <MusicLyricsView v-else-if="ui.panelTab === 'lyrics'" />
        <div v-else-if="ui.panelTab === 'devices'" class="panel__devices">
          <MusicDeviceList :devices="player.devices" :playing="player.playing" @select="player.selectDevice" />
        </div>
        <MusicJamPanel v-else-if="ui.panelTab === 'jam'" back @back="ui.panelTab = 'devices'" />
        <FriendsPanel v-else-if="ui.panelTab === 'friends'" />
        <MusicAboutTrack v-else :track="player.current?.track ?? null" />
      </div>
    </div>
  </aside>
</template>

<style scoped>
.panel {
  width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  background: color-mix(in srgb, var(--ink-1) 66%, transparent);
  -webkit-backdrop-filter: blur(30px) saturate(1.3);
  backdrop-filter: blur(30px) saturate(1.3);
  border-left: 1px solid transparent;
  transition: width var(--dur-3) var(--ease-out), border-color var(--dur-3);
}
.panel.is-open { width: var(--panel-w); border-left-color: var(--hair); }
.panel__inner { display: flex; flex-direction: column; width: var(--panel-w); height: 100%; min-height: 0; padding-top: var(--titlebar-inset-panel, 0px); }
.panel__head { display: flex; align-items: center; gap: 8px; flex: none; padding: 6px 12px 0 20px; }
.panel__tabs { flex: 1; min-width: 0; }
.panel__title { flex: 1; min-width: 0; height: 50px; margin: 0; font-size: var(--fs-body); font-weight: 700; line-height: 50px; }
.panel__tabs :deep(.tabs__list) { gap: 18px; border-bottom: 0; }
.panel__tabs :deep(.tabs__item) { height: 50px; }
.panel__body { flex: 1; min-height: 0; overflow-y: auto; padding: 8px 12px 24px; border-top: 1px solid var(--hair); }
.panel__devices { padding: 12px 0; }
/* Текст: лента сама держит строку — без прокрутки панели */
.panel__body.is-lyrics { position: relative; overflow: hidden; padding: 0 4px; }

/* Планшет: панель ложится поверх содержимого; рядом со сценой плеера — встаёт в ряд */
@media (max-width: 1099px) {
  .panel { position: absolute; top: 0; right: 0; bottom: 0; z-index: 28; box-shadow: none; }
  .panel.is-open { box-shadow: -24px 0 48px -12px rgba(var(--shade-rgb), calc(0.7 * var(--shade-k))); }
  .app.is-player .panel { position: relative; box-shadow: none; }
}
</style>
