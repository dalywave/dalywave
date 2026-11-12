<script setup lang="ts">
// Перемешивание и повтор, как в Spotify: включено — акцент и точка снизу, повтор одного трека —
// значок с «1». Нажатие повтора идёт по кругу: выключен → вся очередь → один трек
const props = withDefaults(defineProps<{
  kind: 'shuffle' | 'repeat'
  size?: 'sm' | 'md' | 'lg'
  /** Подсказка с клавишей (S, R) — только на компьютере */
  kbd?: boolean
}>(), { size: 'sm', kbd: false })

const player = usePlayerStore()
const { t } = useI18n()

const on = computed(() => (props.kind === 'shuffle' ? player.shuffleOn : player.repeatMode !== 'off'))
const icon = computed(() => {
  return props.kind === 'shuffle' ? 'ph:shuffle' : 'ph:repeat'
})
const one = computed(() => props.kind === 'repeat' && player.repeatMode === 'one')
// Подпись — что сделает нажатие
const label = computed(() => {
  if (props.kind === 'shuffle') return player.shuffleOn ? t('player.shuffleOff') : t('player.shuffleOn')
  return player.repeatMode === 'off' ? t('player.repeatAll') : player.repeatMode === 'all' ? t('player.repeatOne') : t('player.repeatOff')
})

function press() {
  if (props.kind === 'shuffle') player.toggleShuffle()
  else player.cycleRepeat()
}
</script>

<template>
  <span class="mode" :class="`mode--${size}`">
    <UiIconButton
      :icon="icon"
      :label="label"
      :size="size"
      :active="on"
      :pressed="on"
      :kbd="kbd ? [kind === 'shuffle' ? 'S' : 'R'] : undefined"
      :disabled="!player.current"
      @click="press"
    />
    <span v-if="one" class="mode__one" aria-hidden="true">1</span>
  </span>
</template>

<style scoped>
.mode { position: relative; display: inline-grid; flex: none; }
/* «1» — на правом верхнем углу значка повтора */
.mode__one {
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
.mode--lg .mode__one { top: 5px; right: 4px; }
</style>
