<script setup lang="ts">
// Белая круглая кнопка «играть/пауза» — в панели плеера, на карточках, в шапках альбомов
withDefaults(defineProps<{
  playing?: boolean
  size?: number
  label?: string
  loading?: boolean
  disabled?: boolean
  forceState?: 'hover' | 'pressed' | 'focus'
}>(), { size: 40 })

defineEmits<{ click: [e: MouseEvent] }>()
</script>

<template>
  <button
    type="button"
    class="pb"
    :class="[forceState && `is-${forceState}`, { 'is-loading': loading }]"
    :style="{ '--size': `${size}px` }"
    :aria-label="label ?? (playing ? 'Пауза' : 'Слушать')"
    :disabled="disabled || undefined"
    @click="$emit('click', $event)"
  >
    <Icon v-if="loading" name="ph:circle-notch" class="pb__icon pb__spin" />
    <Icon v-else :name="playing ? 'ph:pause-fill' : 'ph:play-fill'" class="pb__icon" :class="{ 'pb__icon--play': !playing }" />
  </button>
</template>

<style scoped>
.pb {
  --size: 40px;
  display: grid;
  place-items: center;
  flex: none;
  width: var(--size);
  height: var(--size);
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: var(--text-1);
  color: var(--on-light);
  transition: transform var(--dur-1) var(--ease-out), background-color var(--dur-1), box-shadow var(--dur-2);
}
.pb__icon { font-size: calc(var(--size) * 0.46); }
.pb__icon--play { transform: translateX(5%); }
.pb__spin { animation: spin 0.9s linear infinite; }
.pb:hover, .pb.is-hover { transform: scale(1.05); background: color-mix(in srgb, var(--text-1) 86%, var(--ink-1)); }
.pb:active, .pb.is-pressed { transform: scale(0.95); }
.pb.is-focus { outline: 2px solid var(--focus); outline-offset: 3px; }
.pb:disabled { background: var(--fill-3); color: var(--text-4); transform: none; pointer-events: none; }
</style>
