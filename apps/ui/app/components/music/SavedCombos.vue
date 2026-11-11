<script setup lang="ts">
import { MOOD_ORB, WAVE_PRESETS } from '~/utils/wave-options'

// Сохранённые сочетания запускают волну в одно нажатие
defineProps<{ active?: string | null }>()
const emit = defineEmits<{ select: [id: string], save: [] }>()
const { t } = useI18n()
</script>

<template>
  <div v-edge-fade class="combos no-scrollbar" role="group" :aria-label="t('wave.preset.saved')">
    <button
      v-for="p in WAVE_PRESETS"
      :key="p.id"
      type="button"
      class="combo"
      :class="{ 'is-active': active === p.id }"
      :aria-pressed="active === p.id"
      @click="emit('select', p.id)"
    >
      <span class="combo__swatch" :style="{ '--p1': MOOD_ORB[p.set.mood][0], '--p2': MOOD_ORB[p.set.mood][1], '--p3': MOOD_ORB[p.set.mood][2] }">
        <Icon :name="p.icon" />
      </span>
      <span class="combo__text">
        <span class="combo__title">{{ t(p.titleKey) }}<span v-if="active === p.id" class="combo__now"> · {{ t('wave.preset.current') }}</span></span>
        <span class="combo__meta">{{ t(p.metaKey) }}</span>
      </span>
      <Icon name="ph:play-fill" class="combo__go" />
    </button>
    <button type="button" class="combo combo--add" @click="emit('save')">
      <span class="combo__swatch combo__swatch--add"><Icon name="ph:plus" /></span>
      <span class="combo__text">
        <span class="combo__title">{{ t('wave.preset.save') }}</span>
        <span class="combo__meta">{{ t('wave.preset.saveMeta') }}</span>
      </span>
    </button>
  </div>
</template>

<style scoped>
.combos { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 12px; }
.combo {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  padding: 10px 14px 10px 10px;
  border: 0;
  border-radius: var(--r-4);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  text-align: left;
  transition: background-color var(--dur-1), box-shadow var(--dur-1);
}
.combo:hover { background: var(--fill-2); box-shadow: inset 0 0 0 1px var(--line); }
.combo:active { background: var(--fill-3); }
.combo.is-active { box-shadow: inset 0 0 0 1px var(--accent-line); }
.combo__swatch {
  position: relative;
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex: none;
  border-radius: 10px;
  overflow: hidden;
  color: rgba(255, 255, 255, 0.92);
  background: radial-gradient(120% 90% at 20% 110%, var(--p2), transparent 60%), radial-gradient(100% 80% at 90% 0%, var(--p3), transparent 55%), var(--p1);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
  font-size: 16px;
}
.combo__swatch--add { background: transparent; box-shadow: inset 0 0 0 1.5px var(--line-strong); color: var(--text-2); }
.combo--add { background: transparent; color: var(--text-2); }
.combo__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.combo__title { font-size: var(--fs-body); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.combo__now { color: var(--accent); font-weight: 500; }
.combo__meta { font-size: var(--fs-caption); color: var(--text-2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.combo__go { font-size: 16px; color: var(--text-3); opacity: 0; transform: translateX(-4px); transition: opacity var(--dur-1), transform var(--dur-2) var(--ease-out); }
.combo:hover .combo__go { opacity: 1; transform: none; }

@media (max-width: 1180px) {
  .combos { display: flex; overflow-x: auto; margin-right: -32px; padding-right: 32px; }
  .combo { flex: none; padding-right: 18px; }
}
@media (max-width: 699px) {
  .combos { gap: 8px; margin: 0 -16px; padding: 0 16px; }
  .combo__go { display: none; }
}
</style>
