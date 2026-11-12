<script setup lang="ts">
// Перемешивание и повтор, как в Spotify: включено — акцент и точка снизу, повтор одного трека —
// значок с «1», умное перемешивание — значок с искрой. Перемешивание по кругу: выключено →
// перемешивать → умное (у альбома и плейлиста) → выключено. Повтор: выключен → вся очередь → один
// трек; в волне — только трек. Перемешивать волну незачем: кнопка на месте, но приглушена
const props = withDefaults(defineProps<{
  kind: 'shuffle' | 'repeat'
  size?: 'sm' | 'md' | 'lg'
  /** Подсказка с клавишей (S, R) — только на компьютере */
  kbd?: boolean
}>(), { size: 'sm', kbd: false })

const player = usePlayerStore()
const toast = useToast()
const touch = useMenuSheet()
const { t } = useI18n()

const idle = computed(() => props.kind === 'shuffle' && player.waveActive)
const on = computed(() => (props.kind === 'shuffle' ? !idle.value && player.shuffleOn : player.repeatMode !== 'off'))
const smart = computed(() => props.kind === 'shuffle' && !idle.value && player.shuffleMode === 'smart')
const one = computed(() => props.kind === 'repeat' && player.repeatMode === 'one')
// Подпись — что сделает нажатие
const label = computed(() => {
  if (props.kind === 'shuffle') {
    if (idle.value) return t('player.shuffleInWave')
    if (player.shuffleMode === 'off') return t('player.shuffleOn')
    return player.shuffleMode === 'on' && player.smartAvailable ? t('player.shuffleSmart') : t('player.shuffleOff')
  }
  if (player.waveActive) return player.repeatMode === 'one' ? t('player.repeatOff') : t('player.repeatOne')
  return player.repeatMode === 'off' ? t('player.repeatAll') : player.repeatMode === 'all' ? t('player.repeatOne') : t('player.repeatOff')
})

function press() {
  if (idle.value) {
    // Подсказки у пальца нет — тот же ответ коротким уведомлением
    if (touch.value) toast.show({ text: t('player.shuffleInWave'), icon: 'ph:shuffle' })
    return
  }
  if (props.kind === 'shuffle') player.cycleShuffle()
  else player.cycleRepeat()
}
</script>

<template>
  <span class="mode" :class="[`mode--${size}`, { 'mode--idle': idle }]">
    <UiIconButton
      :icon="kind === 'shuffle' ? 'ph:shuffle' : 'ph:repeat'"
      :label="label"
      :size="size"
      :active="on"
      :pressed="idle ? undefined : on"
      :aria-disabled="idle || undefined"
      :kbd="kbd && !idle ? [kind === 'shuffle' ? 'S' : 'R'] : undefined"
      :disabled="!player.current"
      @click="press"
    />
    <span v-if="one" class="mode__badge" aria-hidden="true">1</span>
    <span v-else-if="smart" class="mode__badge mode__badge--smart" aria-hidden="true"><Icon name="ph:sparkle-fill" /></span>
  </span>
</template>

<style scoped>
.mode { position: relative; display: inline-grid; flex: none; }
/* «1» и искра — на правом верхнем углу значка */
.mode__badge {
  position: absolute;
  top: 3px;
  right: 2px;
  display: grid;
  place-items: center;
  min-width: 12px;
  height: 12px;
  padding: 0 2px;
  border-radius: 6px;
  background: var(--accent);
  color: var(--ink-1);
  font-size: 9px;
  font-weight: 700;
  line-height: 1;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
}
.mode__badge--smart { padding: 0; font-size: 8px; }
.mode--lg .mode__badge { top: 5px; right: 4px; }
.mode--idle :deep(.ib) { opacity: 0.38; }
.mode--idle :deep(.ib:hover) { background: transparent; }
</style>
