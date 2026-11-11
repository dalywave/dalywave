<script setup lang="ts">
import type { WaveMood, WaveSettings } from '~/bridge/types'
import { waveSummary } from '~/utils/wave-options'

// «Моя волна» на главной: живая волна, что играет и почему, настройки без «стены чипов»
withDefaults(defineProps<{ greeting?: string }>(), {})

const player = usePlayerStore()
const ui = useUiStore()
const toast = useToast()
const lang = useLang()
const { t } = useI18n()

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

function onPlay() {
  if (player.waveActive && player.current) return player.toggle()
  return player.startWave()
}

function savePreset() {
  toast.show({ text: t('wave.preset.saveSoon'), icon: 'ph:bookmark-simple' })
}
</script>

<template>
  <section class="hero" :aria-label="t('wave.title')">
    <div class="hero__stage">
      <div class="hero__wave"><MusicWaveCanvas :layout="ui.isPhone ? 'hero-phone' : 'hero'" /></div>
      <div class="hero__intro">
        <div v-if="greeting" class="hero__eyebrow">{{ greeting }}</div>
        <h1 class="hero__title">{{ t('wave.title') }}</h1>
        <p class="hero__summary">{{ summary }}</p>
      </div>
      <div class="hero__play">
        <MusicWavePlayButton :playing="isPlaying" :loading="player.waveLoading && !now" :size="ui.isPhone ? 88 : 72" :label="isPlaying ? t('common.pause') : t('wave.listen')" @click="onPlay" />
        <div class="hero__now" aria-live="polite">
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
      <MusicSavedCombos :active="player.activePreset" @select="player.applyPreset" @save="savePreset" />
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
.hero__below { margin-top: 12px; }

@media (max-width: 699px) {
  .hero { padding: 0 16px; }
  .hero__stage { display: flex; flex-direction: column; justify-content: flex-end; height: 388px; min-height: 0; padding: 0 20px 20px; border-radius: 28px; }
  .hero__wave::after { background: linear-gradient(0deg, rgba(2, 5, 9, 0.85) 0%, rgba(2, 5, 9, 0.35) 34%, transparent 55%); }
  .hero__play { position: absolute; left: 0; right: 0; top: 30%; justify-content: center; padding: 0; transform: translateY(-50%); }
  .hero__now, .hero__eyebrow, .hero__dock { display: none; }
  .hero__intro { gap: 6px; }
  .hero__title { font-size: 34px; }
  .hero__summary { font-size: 15px; }
  .hero__below { display: flex; flex-direction: column; gap: 16px; margin-top: 16px; }
}
</style>
