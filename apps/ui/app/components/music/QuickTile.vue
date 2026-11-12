<script setup lang="ts">
// Плитка быстрого доступа на главной планшета (как у Spotify на iPad): обложка вплотную слева,
// название рядом. Ссылка (to) или действие (click). Играет отсюда — название акцентом и «эквалайзер»
const props = withDefaults(defineProps<{
  title: string
  to?: string
  /** Сейчас играет этот альбом, плейлист или волна */
  current?: boolean
  playing?: boolean
}>(), {})
const emit = defineEmits<{ click: [] }>()

const NuxtLink = resolveComponent('NuxtLink')
</script>

<template>
  <component
    :is="props.to ? NuxtLink : 'button'"
    :to="props.to"
    :type="props.to ? undefined : 'button'"
    class="qt"
    :class="{ 'is-current': current }"
    @click="!props.to && emit('click')"
  >
    <span class="qt__art"><slot name="art" /></span>
    <span class="qt__title">{{ title }}</span>
    <span v-if="current" class="qt__eq" :class="{ 'is-paused': !playing }" aria-hidden="true"><i /><i /><i /></span>
  </component>
</template>

<style scoped>
.qt {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  height: 56px;
  padding: 0 12px 0 0;
  overflow: hidden;
  border: 0;
  border-radius: var(--r-2);
  background: var(--fill-2);
  color: var(--text-1);
  font: inherit;
  text-align: left;
  -webkit-touch-callout: none;
  user-select: none;
  transition: background-color var(--dur-1), transform var(--dur-1) var(--ease-out);
}
.qt:active { background: var(--fill-3); transform: scale(0.98); }
.qt:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.qt__art { display: grid; place-items: center; width: 56px; height: 56px; flex: none; overflow: hidden; }
.qt__art :deep(.art), .qt__art :deep(.pc) { width: 56px; height: 56px; border-radius: 0; }
.qt__title {
  display: -webkit-box;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  font-size: var(--fs-small);
  font-weight: 650;
  line-height: 1.25;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
}
.qt.is-current .qt__title { color: var(--accent); }
.qt__eq { display: inline-flex; align-items: flex-end; gap: 2px; width: 14px; height: 14px; flex: none; color: var(--accent); }
.qt__eq i { flex: 1; height: 100%; border-radius: 1px; background: currentColor; transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.qt__eq i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.qt__eq i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
.qt__eq.is-paused i { animation-play-state: paused; }
</style>
