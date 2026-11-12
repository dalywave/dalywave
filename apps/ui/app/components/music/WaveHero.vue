<script setup lang="ts">
import type { WaveMood, WaveSettings } from '~/bridge/types'
import type { MenuEntry } from '~/types/ui'
import { waveSummary } from '~/utils/wave-options'

// «Моя волна» на главной: живая волна, что играет и почему, настройки без «стены чипов»
withDefaults(defineProps<{ greeting?: string }>(), {})

const player = usePlayerStore()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n()
const { t: tl } = useI18n({ useScope: 'local' })

const settings = computed({
  get: () => player.wave,
  set: (v: WaveSettings) => player.setWave(v),
})
const mood = computed({
  get: () => player.wave.mood,
  set: (m: WaveMood) => player.setWave({ mood: m }),
})
const summary = computed(() => waveSummary(player.wave, lang.value))
const now = computed(() => (player.waveActive ? player.current : null))
const isPlaying = computed(() => player.waveActive && player.playing)
// Волну готовят: начало под настройки скачивается на сервере — без технических подробностей, только ход
const preparing = computed(() => !!player.wavePrep || (player.waveLoading && !player.waveActive))
const need = computed(() => Math.max(1, player.wavePrep?.need || 3))
const ready = computed(() => Math.min(need.value, player.wavePrep?.ready ?? 0))

function onPlay() {
  if (preparing.value) return
  if (player.waveActive && player.current) return player.toggle()
  return player.startWave()
}

const moreItems = computed<MenuEntry[]>(() => [
  { label: tl('taste'), icon: 'ph:fingerprint', onSelect: () => void navigateTo('/taste') },
  { label: tl('resetSettings'), icon: 'ph:arrow-counter-clockwise', disabled: player.wave.mood === 'any' && player.wave.activity === 'any' && player.wave.character === 'mix' && player.wave.language === 'any', onSelect: () => player.setWave({ mood: 'any', activity: 'any', character: 'mix', language: 'any' }) },
])
</script>

<template>
  <section class="hero" :aria-label="t('wave.title')">
    <div class="hero__stage">
      <div class="hero__wave"><MusicWaveCanvas :layout="ui.isPhone ? 'hero-phone' : 'hero'" /></div>
      <div class="hero__more">
        <UiMenu :items="moreItems">
          <UiIconButton icon="ph:dots-three-bold" :label="tl('more')" tone="glass" :size="ui.isPhone ? 'md' : 'sm'" />
        </UiMenu>
      </div>
      <div class="hero__intro">
        <div v-if="greeting" class="hero__eyebrow">{{ greeting }}</div>
        <h1 class="hero__title">{{ t('wave.title') }}</h1>
        <p class="hero__summary">{{ summary }}</p>
      </div>
      <div class="hero__play" :class="{ 'hero__play--prep': preparing }">
        <MusicWavePlayButton :playing="isPlaying" :loading="preparing || (player.waveLoading && !now)" :size="ui.isPhone ? 88 : 72" :label="preparing ? tl('preparing') : isPlaying ? t('common.pause') : t('wave.listen')" @click="onPlay" />
        <div v-if="preparing" class="hero__prep" role="status" aria-live="polite">
          <span class="hero__prep-title">{{ tl('preparing') }}</span>
          <span class="hero__prep-sub">{{ tl('hint') }}</span>
          <div class="hero__prep-row">
            <span class="hero__dots" role="img" :aria-label="tl('progress', { ready, need })">
              <span v-for="i in need" :key="i" class="hero__dot" :class="{ 'is-on': i <= ready, 'is-next': i === ready + 1 }" />
            </span>
            <UiButton :size="ui.isPhone ? 'md' : 'sm'" variant="ghost" icon="ph:x" @click="player.cancelWavePrep()">{{ tl('cancel') }}</UiButton>
          </div>
        </div>
        <div v-else class="hero__now" aria-live="polite">
          <template v-if="now">
            <span class="hero__now-label">{{ isPlaying ? t('wave.nowPlaying') : t('wave.paused') }}</span>
            <span class="hero__now-title">{{ now.track.title }}</span>
            <span class="hero__now-sub">
              <span class="ellipsis">{{ now.track.artists.map(a => a.name).join(', ') }}</span>
              <span v-if="now.reason" class="hero__reason"><Icon name="ph:sparkle-fill" />{{ typographRu(now.reason) }}</span>
            </span>
          </template>
        </div>
      </div>
      <div class="hero__dock"><MusicWaveSettingsBar v-model="settings" variant="dock" /></div>
    </div>
    <div class="hero__below">
      <MusicMoodOrbs v-if="ui.isPhone" v-model="mood" variant="full" />
      <MusicWaveSettingsBar v-if="ui.isPhone" v-model="settings" variant="tokens" />
      <MusicSavedCombos />
    </div>
  </section>
</template>

<style scoped>
.hero { position: relative; }
.hero__stage {
  position: relative;
  display: grid;
  grid-template-areas: 'intro' 'play' 'dock';
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: auto 1fr auto;
  min-height: 404px;
  padding: 36px 32px 24px 40px;
  overflow: hidden;
  isolation: isolate;
  border-radius: var(--r-6);
  background: #020509;
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.06), 0 40px 80px -40px rgba(0, 0, 0, 0.9);
}
.hero__wave { position: absolute; inset: 0; z-index: -1; }
.hero__more { position: absolute; top: 20px; right: 20px; z-index: 1; }
.hero__wave::after {
  content: '';
  position: absolute;
  inset: 0;
  pointer-events: none;
  background: linear-gradient(90deg, rgba(2, 5, 9, 0.55) 0%, rgba(2, 5, 9, 0.12) 38%, transparent 55%), linear-gradient(0deg, rgba(2, 5, 9, 0.5), transparent 32%);
}
.hero__intro { grid-area: intro; display: flex; flex-direction: column; gap: 10px; }
.hero__eyebrow { font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-2); }
.hero__title { margin: 0; font-size: var(--fs-display); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1; text-wrap: balance; }
.hero__summary { max-width: 460px; margin: 0; font-size: var(--fs-h4); color: var(--text-2); text-wrap: pretty; }
.hero__play { grid-area: play; display: flex; align-items: center; gap: 20px; align-self: center; min-width: 0; padding: 20px 0 24px; }
.hero__now { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.hero__now-label { font-size: var(--fs-caption); font-weight: 550; color: var(--text-3); }
.hero__now-title { overflow: hidden; font-size: var(--fs-h4); font-weight: 650; text-overflow: ellipsis; white-space: nowrap; }
.hero__now-sub { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: var(--fs-small); color: var(--text-2); white-space: nowrap; }
.hero__reason { display: inline-flex; align-items: center; gap: 4px; color: var(--accent); }
.hero__reason :deep(.iconify) { font-size: 12px; }
.hero__dock { grid-area: dock; display: flex; min-width: 0; }
.hero__prep { display: flex; flex-direction: column; gap: 4px; min-width: 0; animation: hero-prep-in var(--dur-4) var(--ease-out) both; }
.hero__prep-title { font-size: var(--fs-h4); font-weight: 650; }
.hero__prep-sub { font-size: var(--fs-small); color: var(--text-2); text-wrap: pretty; }
.hero__prep-row { display: flex; align-items: center; gap: 12px; margin-top: 8px; margin-left: -10px; }
.hero__dots { display: inline-flex; align-items: center; gap: 8px; padding: 0 10px; }
.hero__dot {
  width: 8px;
  height: 8px;
  border-radius: var(--r-pill);
  background: rgba(255, 255, 255, 0.18);
  transition: background var(--dur-3) var(--ease-out), transform var(--dur-3) var(--ease-spring);
}
.hero__dot.is-on { background: var(--accent); transform: scale(1.15); }
.hero__dot.is-next { animation: hero-dot 1.4s var(--ease-in-out) infinite; }
@keyframes hero-dot {
  0%, 100% { background: rgba(255, 255, 255, 0.18); transform: scale(1); }
  50% { background: color-mix(in srgb, var(--accent) 55%, rgba(255, 255, 255, 0.2)); transform: scale(1.2); }
}
@keyframes hero-prep-in {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: none; }
}
@media (prefers-reduced-motion: reduce) {
  .hero__prep, .hero__dot.is-next { animation: none; }
}
.hero__below { margin-top: 12px; }

@media (max-width: 699px) {
  .hero { padding: 0 16px; }
  .hero__stage { display: flex; flex-direction: column; justify-content: flex-end; height: min(388px, 50svh); min-height: 300px; padding: 0 20px 20px; border-radius: 28px; }
  .hero__wave::after { background: linear-gradient(0deg, rgba(2, 5, 9, 0.85) 0%, rgba(2, 5, 9, 0.35) 34%, transparent 55%); }
  .hero__play { position: absolute; left: 0; right: 0; top: 30%; justify-content: center; padding: 0; transform: translateY(-50%); }
  .hero__play--prep { flex-direction: column; gap: 14px; top: 34%; }
  .hero__prep { align-items: center; text-align: center; }
  .hero__prep-sub { display: none; }
  .hero__prep-row { margin: 2px 0 0; }
  .hero__now, .hero__eyebrow, .hero__dock { display: none; }
  .hero__intro { gap: 6px; }
  .hero__title { font-size: 34px; }
  .hero__more { top: 12px; right: 12px; }
  .hero__summary { font-size: 15px; }
  .hero__below { display: flex; flex-direction: column; gap: 16px; margin-top: 16px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "preparing": "Готовим волну…",
    "hint": "Подбираем музыку под эти настройки",
    "progress": "Готово {ready} из {need}",
    "cancel": "Отменить",
    "more": "Ещё",
    "taste": "Мой вкус",
    "resetSettings": "Сбросить настройки волны"
  },
  "en": {
    "preparing": "Preparing your wave…",
    "hint": "Picking music for these settings",
    "progress": "{ready} of {need} ready",
    "cancel": "Cancel",
    "more": "More",
    "taste": "My taste",
    "resetSettings": "Reset wave settings"
  }
}
</i18n>
