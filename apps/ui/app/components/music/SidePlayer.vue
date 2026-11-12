<script setup lang="ts">
import { resamplePeaks } from '~/utils/peaks'

// Плеер сбоку на планшете горизонтально, как у Spotify на iPad: колонка «Сейчас играет» справа
// от содержимого, фон — цвет обложки. Сверху — «свернуть в карточку» (настройка выключается),
// «открыть плеер» на весь экран, откуда играет и ⋯; ниже обложка, текущая строка текста песни,
// название с сердцем, полоса, управление и устройства / поделиться / очередь.
// Управление, полоса и текст — те же, что в полном плеере, а не свои копии
const player = usePlayerStore()
const connect = useConnectStore()
const library = useLibraryStore()
const prefs = usePrefsStore()
const ui = useUiStore()
const { menuFor } = useTrackActions()
const { share } = useShare()
const { kicker, title: ctxTitle } = useNowPlayingContext()
const { t } = useI18n()

const entry = computed(() => player.current)
const track = computed(() => entry.value?.track ?? null)
const liked = computed(() => (track.value ? library.isLiked(track.value) : false))
const peaks = computed(() => (player.peaks.length ? resamplePeaks(player.peaks, 48) : Array.from({ length: 48 }, () => 0.18)))
const nextAt = computed(() => ((player.nextEntry || player.waveActive) && !player.pending && player.effectiveMode !== 'gapless' && player.effectiveRepeat !== 'one' && player.durationMs ? player.transitionAtMs : null))

const menuOpen = ref(false)
const menuItems = computed(() => (menuOpen.value && track.value ? menuFor(track.value, { nowPlaying: true }) : []))

// Текущая строка синхронного текста в духе «сцены» (MusicLyricsView): крупно и жирно, смена — мягкая,
// с лёгким размытием; в проигрыше — три дышащие точки, после последней строки — пусто. Модель та же,
// что у полного текста (utils/lyrics.ts). Текст без времени или его нет — строки нет совсем; между
// строками место держится, чтобы ничего не прыгало. Колонка скрыта (карточка, вертикально) — текст не
// спрашиваем и часы не крутим
const { state: lyrics } = useTrackLyrics(computed(() => (ui.sidePlayer ? track.value?.id ?? null : null)))
const lyricItems = computed(() => (lyrics.value.status === 'ready' && lyrics.value.data.kind === 'synced' ? buildLyrics(lyrics.value.data.lines, player.durationMs || track.value?.durationMs || 0) : []))
const lines = computed(() => lyricItems.value.length > 0)
const clock = usePlaybackClock(lines)
const lyricIndex = computed(() => (lines.value ? lyricAt(lyricItems.value, clock.value) : -1))
const lyricItem = computed(() => lyricItems.value[lyricIndex.value] ?? null)
const lyric = computed(() => (lyricItem.value?.type === 'line' ? lyricItem.value.main : ''))

/** Обратно в плавающую карточку внизу — и запомнить: колонка не вернётся, пока её не закрепят снова */
function unpin() {
  prefs.sidePlayer = false
}

function openDevices() {
  ui.openPanel('devices')
}
</script>

<template>
  <Transition name="side" appear>
    <aside v-if="ui.sidePlayer && track" class="side scheme-dark" :aria-label="t('player.region')">
      <header class="side__head">
        <div class="side__head-start">
          <UiIconButton icon="ph:sidebar-simple" class="side-icon" :label="t('player.unpinSide')" size="lg" tone="solid" :tooltip="false" @click="unpin" />
          <UiIconButton icon="ph:arrows-out-simple" :label="t('player.expand')" size="lg" tone="solid" :tooltip="false" @click="ui.playerOpen = true" />
        </div>
        <div class="side__ctx">
          <div class="side__kicker">{{ kicker }}</div>
          <div class="side__ctx-title ellipsis">{{ ctxTitle }}</div>
        </div>
        <div class="side__head-end">
          <UiMenu v-model:open="menuOpen" :items="menuItems">
            <UiIconButton icon="ph:dots-three" :label="t('common.more')" size="lg" tone="solid" :tooltip="false" />
          </UiMenu>
        </div>
      </header>

      <i class="side__space side__space--top" aria-hidden="true" />
      <div class="side__art">
        <button type="button" class="side__art-btn" :aria-label="`${t('player.expand')}: ${track.title}`" @click="ui.playerOpen = true">
          <MusicCoverArt :image="track.album?.image" size="large" :alt="track.album?.title ?? ''" eager />
        </button>
        <div v-if="player.pending" class="side__pending">
          <MusicPendingNote :pending="player.pending" long />
          <UiButton size="sm" icon="ph:skip-forward" @click="player.next('skipped')">{{ t('player.skip') }}</UiButton>
        </div>
      </div>

      <i class="side__space" aria-hidden="true" />
      <div class="side__low">
        <!-- Строка текста: нажатие — весь текст листом -->
        <button v-if="lines" type="button" class="side__lyric" :aria-label="t('player.lyrics')" @click="ui.openPanel('lyrics')">
          <Transition name="side-line" mode="out-in">
            <span v-if="lyricItem?.type === 'break'" :key="lyricIndex" class="side__dots" :aria-label="t('player.lyrics')"><i /><i /><i /></span>
            <span v-else :key="lyricIndex" class="side__lyric-text">{{ lyric }}</span>
          </Transition>
        </button>

        <div class="side__titlerow">
          <div class="side__titles">
            <div class="side__title ellipsis">{{ track.title }}</div>
            <div class="side__artist ellipsis">
              <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`">{{ a.name }}</NuxtLink></template>
            </div>
          </div>
          <span class="side__like">
            <UiIconButton v-if="!player.modesAvailable" icon="ph:thumbs-down" :label="t('player.dislike')" size="lg" tone="solid" :tooltip="false" @click="player.dislike()" />
            <UiIconButton :icon="liked ? 'ph:heart-fill' : 'ph:heart'" :label="liked ? t('player.unlike') : t('player.like')" size="lg" :tone="liked ? 'liked' : 'solid'" :pressed="liked" :tooltip="false" @click="player.toggleLike()" />
          </span>
        </div>
        <div v-if="entry?.reason" class="side__reason ellipsis"><Icon name="ph:sparkle-fill" />{{ typographRu(entry.reason) }}</div>

        <MusicScrubber
          class="side__scrub"
          :peaks="peaks"
          :position-ms="player.positionMs"
          :duration-ms="player.durationMs || track.durationMs"
          :transition-ms="nextAt"
          size="lg"
          times="below"
          :label="t('player.seek')"
          @seek="player.seek"
        />

        <MusicPlayerControls class="side__controls" :play-size="68" />

        <MusicConnectChip class="side__connect" />
        <div class="side__foot">
          <UiIconButton icon="ph:devices" :label="t('player.devices')" size="lg" tone="solid" :active="connect.remote" :tooltip="false" @click="openDevices" />
          <span class="side__foot-end">
            <UiIconButton icon="ph:export" :label="t('share.action')" size="lg" tone="solid" :tooltip="false" @click="share('track', track.id, track.title)" />
            <UiIconButton icon="ph:queue" :label="t('player.queue')" size="lg" tone="solid" :tooltip="false" @click="ui.openPanel('queue')" />
          </span>
        </div>
      </div>
    </aside>
  </Transition>
</template>

<style scoped>
/* Колонка справа: от строки состояния до полоски «домой», с отступами, как карточки Dock.
   Фон — приглушённый цвет обложки, тот же, что у карточки плеера внизу: цвет говорит, что играет */
.side {
  position: fixed;
  top: calc(var(--safe-top) + var(--side-gap));
  right: calc(var(--side-gap) + var(--safe-right));
  bottom: calc(var(--safe-bottom) + var(--side-gap));
  z-index: 30;
  display: flex;
  flex-direction: column;
  width: var(--side-w);
  padding: 6px 20px 14px;
  border-radius: 24px;
  background: color-mix(in oklab, var(--amb-1) 46%, var(--ink-1));
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.08);
  overflow: hidden auto;
  overscroll-behavior: contain;
  scrollbar-width: none;
  transition: background-color var(--dur-mood) var(--ease-in-out);
}
.side::-webkit-scrollbar { display: none; }

.side__head { display: grid; grid-template-columns: 96px minmax(0, 1fr) 96px; align-items: center; height: 60px; margin: 0 -10px; flex: none; }
.side__head-start { display: flex; gap: 4px; }
.side__head-end { display: flex; justify-content: flex-end; }
.side__ctx { min-width: 0; text-align: center; }
.side__kicker { font-size: 10.5px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: rgba(255, 255, 255, 0.62); }
.side__ctx-title { font-size: 14px; font-weight: 650; }

/* Обложка — сколько позволяет высота: на маленьком iPad она уступает место управлению */
.side__art {
  position: relative;
  width: min(100%, max(180px, calc(100dvh - var(--safe-top) - var(--safe-bottom) - 470px)));
  margin: 0 auto;
  flex: none;
}
.side__art-btn { display: block; width: 100%; padding: 0; border: 0; border-radius: var(--r-3); background: none; }
.side__art-btn :deep(.art) { width: 100%; border-radius: var(--r-3); box-shadow: 0 24px 48px -24px rgba(0, 0, 0, 0.8); }
.side__art-btn:active { opacity: 0.85; }
.side__pending {
  position: absolute;
  inset: auto 10px 10px;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 14px;
  border-radius: var(--r-3);
  background: rgba(8, 10, 14, 0.72);
  -webkit-backdrop-filter: blur(18px);
  backdrop-filter: blur(18px);
}

/* Низ колонки прижат к краю: между обложкой и названием — свободное место */
/* Свободная высота — между шапкой и обложкой (немного) и под обложкой (остальное): низ прижат к краю */
.side__space { flex: 1 1 0; min-height: 16px; }
.side__space--top { max-height: 40px; min-height: 4px; }
.side__low { display: flex; flex-direction: column; flex: none; }
.side__lyric {
  display: flex;
  align-items: flex-end;
  min-height: calc(23px * 1.16 * 2);
  margin: 0 0 14px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-1);
  font: inherit;
  font-size: 23px;
  font-weight: 800;
  letter-spacing: -0.022em;
  line-height: 1.16;
  text-align: left;
}
.side__lyric-text { display: -webkit-box; overflow: hidden; -webkit-box-orient: vertical; -webkit-line-clamp: 2; text-wrap: pretty; }
.side__lyric:active { opacity: 0.7; }
/* Проигрыш — три точки, как в полном тексте */
.side__dots { display: inline-flex; gap: 8px; padding-bottom: 8px; animation: side-breathe 1.7s ease-in-out infinite alternate; }
.side__dots i { width: 9px; height: 9px; border-radius: 50%; background: currentColor; opacity: 0.55; }
@keyframes side-breathe {
  from { transform: scale(1); }
  to { transform: scale(1.14); }
}
/* Смена строки мягкая: старая уходит вверх и расплывается, новая проявляется снизу */
.side-line-enter-active { transition: opacity 420ms var(--ease-out), transform 520ms var(--ease-out), filter 420ms var(--ease-out); }
.side-line-leave-active { transition: opacity 220ms ease-in, transform 220ms ease-in, filter 220ms ease-in; }
.side-line-enter-from { opacity: 0; transform: translateY(12px); filter: blur(4px); }
.side-line-leave-to { opacity: 0; transform: translateY(-10px); filter: blur(4px); }

.side__titlerow { display: flex; align-items: flex-start; gap: 12px; }
.side__titles { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.side__title { font-size: 22px; font-weight: 750; letter-spacing: -0.025em; line-height: 1.2; }
.side__artist { font-size: 16px; color: rgba(255, 255, 255, 0.72); }
.side__artist a:active { color: var(--text-1); }
.side__like { display: inline-flex; flex: none; margin: -6px -10px 0 0; }
.side__reason { display: flex; align-items: center; gap: 5px; margin-top: 6px; font-size: var(--fs-small); color: rgba(255, 255, 255, 0.8); }
.side__scrub { margin-top: 14px; }
.side__controls { margin: 8px -6px 0; }
.side__connect { justify-content: center; margin-top: 8px; }
.side__foot { display: flex; align-items: center; justify-content: space-between; margin: 8px -10px 0; }
.side__foot-end { display: flex; gap: 4px; }

.side-enter-active, .side-leave-active { transition: opacity var(--dur-3) var(--ease-out), transform var(--dur-3) var(--ease-out); }
.side-enter-from, .side-leave-to { opacity: 0; transform: translateX(24px); }
@media (prefers-reduced-motion: reduce) {
  .side-enter-active, .side-leave-active, .side-line-enter-active, .side-line-leave-active { transition: none; }
  .side__dots { animation: none; }
}
</style>
