<script setup lang="ts">
import type { StatusTone } from '~/composables/useAdmin'

// Состояние строкой: цветная точка и подпись. Один вид во всех таблицах админки — цвет несёт точка,
// подпись остаётся спокойной; красной становится только ошибка
withDefaults(defineProps<{
  tone?: StatusTone
  /** Идёт работа: точка мерцает */
  pulse?: boolean
  /** Свой цвет точки (состояния записи из AUDIO_COLOR) */
  color?: string
}>(), { tone: 'idle' })
</script>

<template>
  <span class="ast" :class="`is-${tone}`">
    <i class="ap-dot" :class="[`is-${tone}`, { 'is-pulse': pulse }]" :style="color ? { background: color } : undefined" />
    <span class="ast__label"><slot /></span>
  </span>
</template>

<style scoped>
.ast { display: inline-flex; align-items: center; gap: 8px; min-width: 0; font-weight: 500; color: var(--text-1); }
.ast.is-idle { color: var(--text-2); }
.ast.is-bad { color: var(--danger); }
.ast__label { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
