<script setup lang="ts">
import type { PlaybackDevice } from '~/bridge/types'

// Настройки устройства прямо в списке «Где играть» (⋯ у строки): где играть музыку, выбранную на нём,
// и «Медиаплеер» — пока звук на нём, выбор на других устройствах идёт сюда. Хранятся на сервере
// (stores/connect.ts → PATCH /api/devices/{id}), поэтому меняются с любого устройства.
// `sheet` — лист на телефоне: крупнее, отступ как у строк листа
const props = withDefaults(defineProps<{ device: PlaybackDevice, variant?: 'compact' | 'sheet' }>(), { variant: 'compact' })
const { t } = useI18n()
const connect = useConnectStore()

type Choice = NonNullable<PlaybackDevice['choice']>
const options = computed(() => [
  { value: 'auto' as const, label: t('devices.choiceAuto'), hint: t('devices.choiceAutoHint') },
  { value: 'here' as const, label: props.device.current ? t('devices.choiceSelf') : t('devices.choiceOn'), hint: t('devices.choiceSelfLong') },
  { value: 'there' as const, label: t('devices.choiceActive'), hint: t('devices.choiceActiveLong') },
])

const choice = computed<Choice>({
  get: () => props.device.choice ?? 'auto',
  set: v => void connect.updateDevice(props.device.id, { choice: v }),
})
const media = computed({
  get: () => !!props.device.mediaPlayer,
  set: v => void connect.updateDevice(props.device.id, { mediaPlayer: v }),
})
</script>

<template>
  <div class="dset" :class="`dset--${variant}`">
    <span class="dset__label">{{ device.current ? t('devices.choiceHere') : t('devices.choiceThere') }}</span>
    <UiSegmentedControl v-model="choice" :options="options" :label="device.current ? t('devices.choiceHere') : t('devices.choiceThere')" :size="variant === 'sheet' ? 'md' : 'sm'" block />
    <div class="dset__row">
      <span class="dset__text">
        <span class="dset__title">{{ t('devices.mediaPlayer') }}</span>
        <span class="dset__hint">{{ t('devices.mediaPlayerHint') }}</span>
      </span>
      <UiSwitch v-model="media" :label="t('devices.mediaPlayer')" hide-label />
    </div>
  </div>
</template>

<style scoped>
.dset { display: flex; flex-direction: column; gap: 8px; grid-column: 1 / -1; padding: 4px 10px 10px 58px; }
.dset__label { font-size: var(--fs-caption); color: var(--text-3); }
.dset__row { display: flex; align-items: center; gap: 12px; margin-top: 4px; }
.dset__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.dset__title { font-size: var(--fs-small); font-weight: 600; }
.dset__hint { font-size: var(--fs-caption); color: var(--text-3); }

/* Лист на телефоне: от края, как строки листа; переключатель — в зоне касания */
.dset--sheet { gap: 10px; padding: 2px 20px 14px 58px; }
.dset--sheet .dset__label { font-size: 13px; }
.dset--sheet .dset__row { min-height: 44px; }
.dset--sheet .dset__title { font-size: 15px; font-weight: 500; }
.dset--sheet .dset__hint { font-size: 13px; }
</style>
