<script setup lang="ts">
// Таймер сна у плеера: луна; пока таймер стоит — залита акцентом и рядом остаток («12:04»).
// По нажатию — меню: на телефоне листом снизу, у мыши — выпадающее
withDefaults(defineProps<{
  /** sm — нижняя панель большого экрана, lg — плеер на телефоне */
  size?: 'sm' | 'lg'
}>(), { size: 'lg' })

const sound = useSoundStore()
const open = ref(false)
const { items, remaining, hint } = useSleepMenu()
const { t } = useI18n()

const label = computed(() => (hint.value ? `${t('sound.sleep')}: ${hint.value}` : t('sound.sleep')))
</script>

<template>
  <UiMenu v-model:open="open" :items="items" side="top" align="center">
    <button
      type="button"
      class="moon"
      :class="[`moon--${size}`, { 'is-on': sound.sleepActive }]"
      :aria-label="label"
      :title="size === 'sm' ? label : undefined"
    >
      <Icon :name="sound.sleepActive ? 'ph:moon-fill' : 'ph:moon'" class="moon__icon" />
      <span v-if="sound.sleepActive && remaining" class="moon__time tnum">{{ remaining }}</span>
    </button>
  </UiMenu>
</template>

<style scoped>
.moon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  flex: none;
  min-width: 44px;
  height: 44px;
  padding: 0 10px;
  border: 0;
  border-radius: var(--r-pill);
  background: transparent;
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-1), transform var(--dur-1) var(--ease-out);
}
.moon__icon { font-size: 24px; flex: none; }
.moon__time { font-size: var(--fs-small); font-weight: 600; }
.moon.is-on { color: var(--accent); }
.moon:hover, .moon[aria-expanded='true'], .moon[data-state='open'] { color: var(--text-1); background: var(--fill-2); }
.moon.is-on:hover { color: var(--accent); }
.moon:active { transform: scale(0.94); background: var(--fill-3); }
.moon:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.moon--sm { min-width: 32px; height: 32px; padding: 0 8px; }
.moon--sm .moon__icon { font-size: 18px; }
.moon--sm .moon__time { font-size: var(--fs-caption); }
</style>
