<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Панель «Сейчас играет» внизу большого экрана: трек и почему он здесь, управление с формой волны,
// режим переходов, текст, очередь, устройства, громкость
const player = usePlayerStore()
const connect = useConnectStore()
const library = useLibraryStore()
const ui = useUiStore()
const { t } = useI18n()

const entry = computed(() => player.current)
const track = computed(() => entry.value?.track ?? null)
const liked = computed(() => (track.value ? library.isLiked(track.value) : false))
const nextAt = computed(() => (player.nextEntry && !player.pending && player.effectiveMode !== 'gapless' && player.effectiveRepeat !== 'one' && player.durationMs ? formatDuration(player.transitionAtMs) : null))
const peaks = computed(() => (player.peaks.length ? player.peaks : FLAT))
const FLAT = Array.from({ length: 120 }, () => 0.18)

function openPlayer() {
  if (track.value) ui.playerOpen = !ui.playerOpen
}

// «Текст»: при открытом плеере — колонкой справа от обложки на самой сцене, иначе — вкладкой панели
const lyricsActive = computed(() => (ui.playerOpen ? ui.playerLyrics : ui.panelOpen && ui.panelTab === 'lyrics'))
function toggleLyrics() {
  if (!ui.playerOpen) return ui.togglePanel('lyrics')
  ui.playerLyrics = !ui.playerLyrics
  // Тот же текст во вкладке панели рядом не нужен
  if (ui.playerLyrics && ui.panelOpen && ui.panelTab === 'lyrics') ui.panelOpen = false
}

// «⋯»: таймер сна и мини-плеер (приложение для компьютера)
const sound = useSoundStore()
const bridge = useBridge()
const { items: sleepItems } = useSleepMenu()
const moreOpen = ref(false)
const moreItems = computed<MenuEntry[]>(() => {
  const list: MenuEntry[] = [{ label: t('sound.sleep'), icon: sound.sleepActive ? 'ph:moon-fill' : 'ph:moon', children: sleepItems.value.filter(e => e.kind !== 'heading') }]
  const mini = bridge.miniPlayer
  if (mini) list.push({ label: t('sound.mini'), icon: 'ph:picture-in-picture', kbd: [bridge.platform === 'macos' ? '⌘' : 'Ctrl', '⇧', 'M'], onSelect: () => void mini.open() })
  return list
})
</script>

<template>
  <div class="bar" role="region" :aria-label="t('player.region')">
    <div class="bar__track">
      <button type="button" class="bar__cover" :aria-label="t('player.expand')" @click="openPlayer">
        <MusicCoverArt v-if="track" :image="track.album?.image" size="small" :alt="track.album?.title ?? ''" />
        <span v-else class="bar__idle-art"><BrandGlyph class="bar__glyph" /></span>
        <span v-if="track" class="bar__expand" aria-hidden="true"><Icon :name="ui.playerOpen ? 'ph:caret-down' : 'ph:caret-up'" /></span>
      </button>
      <div v-if="track" class="bar__meta">
        <div class="bar__titlerow">
          <NuxtLink :to="track.album ? `/album/${track.album.id}` : undefined" class="bar__title ellipsis">{{ track.title }}</NuxtLink>
          <!-- Версия песни (оригинал, slowed, sped up) — маленькой подписью с меню, только если есть другие -->
          <MusicVersionSwitch />
        </div>
        <div class="bar__artist ellipsis">
          <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`">{{ a.name }}</NuxtLink></template>
        </div>
        <MusicWavePrepNote v-if="player.waveStarting" />
        <MusicConnectChip v-else-if="connect.remote || connect.orphaned" variant="line" />
        <MusicPendingNote v-else-if="player.pending" :pending="player.pending" />
        <div v-else-if="entry?.reason" class="bar__reason ellipsis"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>
      </div>
      <!-- Сердце — рядом с треком, как в Spotify; «не нравится» — только в волне и радио -->
      <div v-if="track" class="bar__react">
        <UiIconButton
          :icon="liked ? 'ph:heart-fill' : 'ph:heart'"
          :label="liked ? t('player.unlike') : t('player.like')"
          :kbd="['L']"
          :tone="liked ? 'liked' : 'default'"
          :pressed="liked"
          size="sm"
          @click="player.toggleLike()"
        />
        <UiIconButton v-if="!player.modesAvailable" icon="ph:thumbs-down" :label="t('player.dislike')" :tooltip="t('player.dislikeHint')" size="sm" @click="player.dislike()" />
      </div>
      <div v-else class="bar__meta">
        <span class="bar__title ellipsis">{{ t('wave.title') }}</span>
        <MusicWavePrepNote v-if="player.waveStarting" />
        <span v-else class="bar__artist ellipsis">{{ t('player.nothingHint') }}</span>
      </div>
    </div>

    <div class="bar__center">
      <div class="bar__transport">
        <MusicPlayModeButton kind="shuffle" kbd />
        <UiIconButton icon="ph:skip-back-fill" :label="t('player.prev')" :kbd="['⇧', '←']" :disabled="!track" @click="player.prev()" />
        <MusicPlayButton :playing="player.playing" :loading="(player.waveLoading && !track) || !!player.pending" :size="36" :label="player.pending ? t('player.acquiring') : player.playing ? t('common.pause') : t('common.play')" @click="player.toggle()" />
        <UiIconButton icon="ph:skip-forward-fill" :label="t('player.next')" :kbd="['⇧', '→']" :disabled="!track" @click="player.next('skipped')" />
        <MusicPlayModeButton kind="repeat" kbd />
      </div>
      <MusicScrubber
        :peaks="peaks"
        :position-ms="player.positionMs"
        :duration-ms="player.durationMs || track?.durationMs || 0"
        :transition-ms="nextAt ? player.transitionAtMs : null"
        :transition-label="nextAt ? `${t('player.transitionAt', { time: nextAt })} · ${t(`transition.${player.effectiveMode}`)}` : undefined"
        :disabled="!track || !!player.pending"
        :label="t('player.seek')"
        @seek="player.seek"
      />
    </div>

    <div class="bar__right">
      <!-- Приватная сессия: друзья не видят, что играет -->
      <FriendsPrivateChip variant="icon" />
      <!-- Таймер сна стоит: луна с остатком -->
      <MusicSleepTimerButton v-if="sound.sleepActive" size="sm" />
      <MusicTransitionModePicker v-model:mode="connect.transition" v-model:expressiveness="player.expressiveness" :web="!connect.djAvailable" :next-at="nextAt" icon size="sm" />
      <UiIconButton icon="ph:microphone-stage" :label="t('player.lyrics')" size="sm" :active="lyricsActive" :pressed="lyricsActive" @click="toggleLyrics" />
      <UiIconButton icon="ph:queue" :label="t('player.queue')" size="sm" :active="ui.panelOpen && ui.panelTab === 'queue'" @click="ui.togglePanel('queue')" />
      <MusicDevicePicker :devices="player.devices" :playing="player.playing" @open="player.loadDevices()" @select="player.selectDevice" />
      <!-- Пульт: громкость активного устройства -->
      <MusicVolumeSlider v-if="connect.volumeControl" v-model:volume="connect.volume" v-model:muted="connect.muted" />
      <UiMenu v-model:open="moreOpen" :items="moreItems" side="top" align="end">
        <UiIconButton icon="ph:dots-three" :label="t('common.more')" size="sm" />
      </UiMenu>
    </div>
  </div>
</template>

<style scoped>
.bar {
  position: relative;
  z-index: 30;
  display: grid;
  grid-template-columns: minmax(220px, 1fr) minmax(360px, 640px) minmax(220px, 1fr);
  align-items: center;
  gap: 24px;
  /* iPad: над полоской «домой» */
  height: calc(var(--bar-h) + var(--safe-bottom));
  padding: 0 calc(16px + var(--safe-right)) var(--safe-bottom) calc(14px + var(--safe-left));
  background: var(--glass-bar);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
  backdrop-filter: blur(30px) saturate(1.4);
  border-top: 1px solid var(--hair);
}
.bar__track { display: flex; align-items: center; gap: 14px; min-width: 0; }
.bar__cover { position: relative; width: 56px; height: 56px; flex: none; padding: 0; border: 0; border-radius: 8px; background: none; box-shadow: 0 6px 18px -8px rgba(var(--shade-rgb), calc(0.9 * var(--shade-k))); transition: transform var(--dur-2) var(--ease-out); }
.bar__cover :deep(.art) { width: 100%; border-radius: 8px; }
.bar__cover:hover { transform: scale(1.04); }
.bar__expand { position: absolute; inset: 0; display: grid; place-items: center; border-radius: 8px; background: rgba(4, 5, 7, 0.5); color: #fff; font-size: 20px; opacity: 0; transition: opacity var(--dur-1); }
.bar__cover:hover .bar__expand, .bar__cover:focus-visible .bar__expand { opacity: 1; }
.bar__idle-art { display: grid; place-items: center; width: 100%; height: 100%; border-radius: 8px; background: var(--brand-gradient); color: #fff; }
.bar__glyph { width: 30px; height: 30px; }
.bar__meta { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
.bar__react { display: flex; align-items: center; flex: none; margin-left: -6px; }
.bar__title { font-weight: 600; }
.bar__titlerow { display: flex; align-items: center; gap: 8px; min-width: 0; }
.bar__titlerow .bar__title { min-width: 0; }
a.bar__title:hover, .bar__artist a:hover { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.bar__artist { font-size: var(--fs-small); color: var(--text-2); }
.bar__reason { display: flex; align-items: center; gap: 4px; font-size: var(--fs-caption); color: var(--text-3); }
.bar__reason :deep(.iconify) { flex: none; font-size: 12px; color: var(--accent); }
.bar__center { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.bar__transport { display: flex; align-items: center; justify-content: center; gap: 8px; height: 40px; }
.bar__transport :deep(.pb) { margin: 0 6px; }
.bar__right { display: flex; align-items: center; justify-content: flex-end; gap: 2px; min-width: 0; }

@media (max-width: 1279px) {
  .bar { grid-template-columns: minmax(200px, 1fr) minmax(320px, 520px) minmax(200px, 1fr); gap: 16px; }
}
@media (max-width: 1099px) {
  .bar { grid-template-columns: minmax(0, 1fr) minmax(300px, 1.6fr) auto; }
  .bar__reason { display: none; }
  .bar__right :deep(.vol) { width: 96px; }
}
</style>
