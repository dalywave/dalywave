<script setup lang="ts">
import type { DownloaderView } from '~/composables/useDownloader'

// Знак загрузчика: кольцо хода задания и значок состояния. Акцент — только когда качает;
// на связи — зелёная точка; нет связи и отдых — предупреждение; токен и поломка — ошибка
const props = withDefaults(defineProps<{
  view: DownloaderView
  size?: number
  /** Windows: значок компьютера вместо ноутбука */
  pc?: boolean
}>(), { size: 28 })

const C = 2 * Math.PI * 10

const computer = computed(() => (props.pc ? 'ph:desktop' : 'ph:laptop'))
const icon = computed(() => {
  switch (props.view.kind) {
    case 'working': return 'ph:arrow-down'
    case 'paused': return 'ph:pause-fill'
    case 'offline': return 'ph:wifi-slash'
    case 'resting': return 'ph:hourglass-medium'
    case 'token': return 'ph:key'
    case 'broken': return 'ph:warning-circle'
    case 'forbidden':
    case 'signed-out': return 'ph:lock-simple'
    default: return computer.value
  }
})
const tone = computed(() => {
  switch (props.view.kind) {
    case 'working': return 'accent'
    case 'idle': return 'live'
    case 'offline':
    case 'resting': return 'warn'
    case 'token':
    case 'broken': return 'bad'
    case 'disabled':
    case 'forbidden':
    case 'signed-out': return 'off'
    case 'starting':
    case 'restarting':
    case 'stopping': return 'busy'
    default: return 'calm'
  }
})
const progress = computed(() => props.view.job?.progress ?? 0)
const offset = computed(() => (C * (1 - Math.min(1, Math.max(0, progress.value)))).toFixed(2))
</script>

<template>
  <span class="dring" :class="`dring--${tone}`" :style="{ '--size': `${size}px` }" aria-hidden="true">
    <svg viewBox="0 0 24 24">
      <circle class="dring__track" cx="12" cy="12" r="10" />
      <circle v-if="tone === 'accent'" class="dring__bar" cx="12" cy="12" r="10" :stroke-dasharray="C.toFixed(2)" :stroke-dashoffset="offset" />
      <circle v-else-if="tone === 'busy'" class="dring__spin" cx="12" cy="12" r="10" :stroke-dasharray="`${(C * 0.28).toFixed(2)} ${C.toFixed(2)}`" />
    </svg>
    <Icon :name="icon" class="dring__icon" />
    <span v-if="tone === 'live'" class="dring__dot" />
  </span>
</template>

<style scoped>
.dring { position: relative; display: inline-grid; place-items: center; flex: none; width: var(--size); height: var(--size); color: var(--text-2); }
.dring svg { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; transform: rotate(-90deg); }
.dring circle { fill: none; stroke-width: 2.2; }
.dring__track { stroke: var(--fill-3); }
.dring__bar { stroke: var(--accent); stroke-linecap: round; transition: stroke-dashoffset var(--dur-3) var(--ease-out); filter: drop-shadow(0 0 3px color-mix(in oklab, var(--accent) 55%, transparent)); }
.dring__spin { stroke: var(--text-3); stroke-linecap: round; transform-origin: 12px 12px; animation: dring-spin 1.1s linear infinite; }
.dring__icon { font-size: calc(var(--size) * 0.46); }
.dring__dot { position: absolute; right: 0; bottom: 0; width: max(6px, calc(var(--size) * 0.24)); height: max(6px, calc(var(--size) * 0.24)); border-radius: 50%; background: var(--success); box-shadow: 0 0 0 2px var(--ink-1), 0 0 8px color-mix(in oklab, var(--success) 60%, transparent); animation: dring-breathe 2.8s var(--ease-in-out) infinite; }
.dring--accent { color: var(--accent); }
.dring--accent .dring__track { stroke: var(--accent-soft); }
.dring--calm { color: var(--text-2); }
.dring--live { color: var(--text-1); }
.dring--warn { color: var(--warning); }
.dring--warn .dring__track { stroke: color-mix(in oklab, var(--warning) 30%, transparent); stroke-dasharray: 2 3; }
.dring--bad { color: var(--danger); }
.dring--bad .dring__track { stroke: color-mix(in oklab, var(--danger) 38%, transparent); }
.dring--off { color: var(--text-3); }
.dring--off .dring__track { stroke: var(--line-strong); stroke-dasharray: 2 3; }
.dring--busy { color: var(--text-3); }
@keyframes dring-spin { to { transform: rotate(360deg); } }
@keyframes dring-breathe { 50% { opacity: 0.55; } }
@media (prefers-reduced-motion: reduce) {
  .dring__spin, .dring__dot { animation: none; }
}
</style>
