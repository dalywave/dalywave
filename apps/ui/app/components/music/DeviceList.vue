<script setup lang="ts">
import type { PlaybackDevice } from '~/bridge/types'

// Список устройств «Где играть»: текущее — акцентом и с эквалайзером, не в сети — приглушено
defineProps<{
  devices: PlaybackDevice[]
  playing?: boolean
}>()
const emit = defineEmits<{ select: [device: PlaybackDevice] }>()
const { t } = useI18n()
const lang = useLang()

const ICON: Record<PlaybackDevice['kind'], string> = {
  laptop: 'ph:laptop',
  desktop: 'ph:desktop',
  phone: 'ph:device-mobile',
  tablet: 'ph:device-tablet',
  web: 'ph:browser',
}

function meta(d: PlaybackDevice) {
  if (d.current) return t('devices.here')
  if (!d.online) return d.lastSeenAt ? `${t('devices.offline')} · ${t('devices.lastSeen', { when: formatRelative(d.lastSeenAt, lang.value) })}` : t('devices.offline')
  return d.kind === 'web' ? 'Вкладка браузера' : 'В сети'
}
</script>

<template>
  <div class="devs" role="list">
    <button
      v-for="d in devices"
      :key="d.id"
      type="button"
      role="listitem"
      class="dev"
      :class="{ 'is-current': d.current, 'is-offline': !d.online }"
      :aria-current="d.current || undefined"
      @click="emit('select', d)"
    >
      <span class="dev__icon"><Icon :name="ICON[d.kind]" /></span>
      <span class="dev__text">
        <span class="dev__name ellipsis">{{ d.name }}</span>
        <span class="dev__meta">
          <span v-if="d.current" class="dev__eq" :class="{ 'is-paused': !playing }" aria-hidden="true"><i /><i /><i /></span>
          {{ meta(d) }}
        </span>
      </span>
      <Icon v-if="d.current" name="ph:check" class="dev__check" />
    </button>
  </div>
</template>

<style scoped>
.devs { display: flex; flex-direction: column; gap: 2px; }
.dev { display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px 10px; border: 0; border-radius: 12px; background: transparent; text-align: left; transition: background-color var(--dur-1); }
.dev:hover, .dev:focus-visible { background: var(--fill-2); outline: none; }
.dev__icon { display: grid; place-items: center; width: 36px; height: 36px; flex: none; border-radius: 10px; background: var(--fill-2); color: var(--text-2); font-size: 20px; }
.dev.is-current .dev__icon { background: var(--accent-soft); color: var(--accent); }
.dev__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.dev__name { font-weight: 600; }
.dev.is-current .dev__name { color: var(--accent); }
.dev__meta { display: flex; align-items: center; gap: 6px; font-size: var(--fs-caption); color: var(--text-3); }
.dev.is-offline { opacity: 0.5; }
.dev__check { font-size: 18px; color: var(--accent); }
.dev__eq { display: inline-flex; align-items: flex-end; gap: 1.5px; width: 11px; height: 11px; color: var(--accent); }
.dev__eq i { flex: 1; height: 100%; border-radius: 1px; background: currentColor; transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.dev__eq i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.dev__eq i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
.dev__eq.is-paused i { animation-play-state: paused; }
</style>
