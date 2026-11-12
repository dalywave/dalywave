<script setup lang="ts">
// Волна или радио запускается: видно с любой страницы (мини-плеер, панель плеера), а не только на главной.
// «1 из 3» — сколько треков начала уже на сервере; ✕ — отменить, что играло — играет дальше
const player = usePlayerStore()
const { t } = useI18n({ useScope: 'local' })

const progress = computed(() => {
  const p = player.wavePrep
  return p && p.need > 1 ? t('of', { ready: Math.min(p.ready, p.need), need: p.need }) : ''
})
</script>

<template>
  <span class="wpn" role="status">
    <span class="wpn__spin" aria-hidden="true" />
    <span class="wpn__text">{{ t('preparing', { name: player.waveStarting }) }}<template v-if="progress"> · {{ progress }}</template></span>
    <button type="button" class="wpn__x" :aria-label="t('cancel')" @click.stop="player.cancelWavePrep()"><Icon name="ph:x" /></button>
  </span>
</template>

<style scoped>
.wpn { display: inline-flex; align-items: center; gap: 6px; min-width: 0; color: var(--accent); font-size: var(--fs-caption); font-weight: 550; }
.wpn__text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.wpn__spin { width: 11px; height: 11px; flex: none; border-radius: 50%; border: 1.5px solid color-mix(in oklab, currentColor 30%, transparent); border-top-color: currentColor; animation: spin 0.9s linear infinite; }
.wpn__x { display: grid; place-items: center; width: 20px; height: 20px; flex: none; padding: 0; border: 0; border-radius: 50%; background: transparent; color: var(--text-3); font-size: 12px; cursor: pointer; }
.wpn__x:hover { background: var(--fill-2); color: var(--text-1); }
@media (pointer: coarse) {
  .wpn__x { width: 32px; height: 32px; margin: -6px -8px -6px -4px; font-size: 14px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "preparing": "Готовим «{name}»…",
    "of": "{ready} из {need}",
    "cancel": "Отменить"
  },
  "en": {
    "preparing": "Preparing “{name}”…",
    "of": "{ready} of {need}",
    "cancel": "Cancel"
  }
}
</i18n>
