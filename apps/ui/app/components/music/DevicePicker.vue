<script setup lang="ts">
import type { PlaybackDevice } from '~/bridge/types'

// «Где играть»: значок в панели плеера раскрывает список устройств
withDefaults(defineProps<{
  devices: PlaybackDevice[]
  playing?: boolean
  side?: 'top' | 'bottom'
  size?: 'sm' | 'md' | 'lg'
}>(), { side: 'top', size: 'sm' })
const emit = defineEmits<{ select: [device: PlaybackDevice], open: [] }>()

const open = ref(false)
const { t } = useI18n()
watch(open, v => v && emit('open'))

function pick(d: PlaybackDevice) {
  emit('select', d)
  open.value = false
}
</script>

<template>
  <UiPopover v-model:open="open" :side="side" align="end" :title="t('devices.title')" :width="320">
    <template #trigger>
      <UiIconButton icon="ph:devices" :label="t('player.devices')" :size="size" :tooltip="false" :active="devices.some(d => d.current && d.kind !== 'web')" />
    </template>
    <MusicDeviceList :devices="devices" :playing="playing" @select="pick" @jam="open = false" />
  </UiPopover>
</template>
