<script setup lang="ts">
// Строка «Эквалайзер» в настройках: справа — выбранный пресет; по нажатию — лист снизу на телефоне
// и под пальцем, всплывающая панель у мыши — только выбор пресета. Полосы «Своего» — прямо под строкой
// на странице: двигать их во всплывающем окне неудобно
const sound = useSoundStore()
function picked(id: string) {
  if (id === 'custom') open.value = false
}
const sheet = useMenuSheet()
const open = ref(false)
const { t } = useI18n()

const current = computed(() => t(`sound.preset.${sound.eq.preset}`))
</script>

<template>
  <UiSheet v-if="sheet" v-model:open="open" :title="t('sound.eq')" inset>
    <template #trigger>
      <SettingsRow as="button" :title="t('sound.eq')">
        <span class="eqp__value">{{ current }}</span>
        <Icon name="ph:caret-right" class="eqp__chev" aria-hidden="true" />
      </SettingsRow>
    </template>
    <div class="eqp__sheet">
      <MusicEqPanel @picked="picked" />
    </div>
  </UiSheet>
  <UiPopover v-else v-model:open="open" side="bottom" align="end" :title="t('sound.eq')" :width="348" no-autofocus>
    <template #trigger>
      <SettingsRow as="button" :title="t('sound.eq')" :aria-expanded="open">
        <span class="eqp__value">{{ current }}</span>
        <Icon name="ph:caret-down" class="eqp__chev" aria-hidden="true" />
      </SettingsRow>
    </template>
    <MusicEqPanel @picked="picked" />
  </UiPopover>
  <div v-if="sound.eq.preset === 'custom'" class="eqp__inline">
    <MusicEqPanel section="bands" />
  </div>
</template>

<style scoped>
.eqp__value { font-size: var(--fs-body); color: var(--text-2); white-space: nowrap; }
.eqp__chev { margin-left: 6px; font-size: 16px; color: var(--text-3); }
.eqp__sheet { padding: 0 8px 10px; }
.eqp__inline { padding: 8px 16px 16px; }
</style>
