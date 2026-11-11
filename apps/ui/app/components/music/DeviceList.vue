<script setup lang="ts">
import type { PlaybackDevice } from '~/bridge/types'

// Список «Где играть»: текущее — акцентом и с эквалайзером, не в сети — приглушено.
// `compact` — всплывающая панель и правая панель на большом экране; `sheet` — лист на телефоне:
// строки на всю ширину с тонкими разделителями, разделы «Это устройство», другие устройства
// DalyWave и вывод звука (AirPlay и Bluetooth), если платформа его умеет
const props = withDefaults(defineProps<{
  devices: PlaybackDevice[]
  playing?: boolean
  variant?: 'compact' | 'sheet'
}>(), { variant: 'compact' })
const emit = defineEmits<{ select: [device: PlaybackDevice] }>()
const { t } = useI18n()
const lang = useLang()
const out = useAudioOutputs()

const ICON: Record<PlaybackDevice['kind'], string> = {
  laptop: 'ph:laptop',
  desktop: 'ph:desktop',
  phone: 'ph:device-mobile',
  tablet: 'ph:device-tablet',
  web: 'ph:browser',
}

const here = computed(() => props.devices.filter(d => d.current))
const others = computed(() => props.devices.filter(d => !d.current))

function meta(d: PlaybackDevice) {
  if (d.current) return t('devices.here')
  if (!d.online) return d.lastSeenAt ? `${t('devices.offline')} · ${t('devices.lastSeen', { when: formatRelative(d.lastSeenAt, lang.value) })}` : t('devices.offline')
  return d.kind === 'web' ? t('devices.tab') : t('devices.onlineOne')
}

const routeHint = computed(() => out.current.value?.name ?? t('devices.routesHint'))
const systemHint = computed(() => (out.state.value.followsSystem ? out.current.value?.name ?? '' : ''))
</script>

<template>
  <!-- Телефон: лист -->
  <div v-if="variant === 'sheet'" class="dsheet">
    <section v-if="here.length" class="dsec">
      <h3 class="dsec__title">{{ t('devices.thisDevice') }}</h3>
      <button v-for="d in here" :key="d.id" type="button" class="drow is-current" aria-current="true" @click="emit('select', d)">
        <Icon :name="ICON[d.kind]" class="drow__icon" />
        <span class="drow__text">
          <span class="drow__name ellipsis">{{ d.name }}</span>
          <span class="drow__meta">
            <span class="dev__eq" :class="{ 'is-paused': !playing }" aria-hidden="true"><i /><i /><i /></span>
            {{ meta(d) }}
          </span>
        </span>
        <Icon name="ph:check-bold" class="drow__check" />
      </button>
    </section>

    <section class="dsec">
      <h3 class="dsec__title">{{ t('devices.otherApps') }}</h3>
      <button v-for="d in others" :key="d.id" type="button" class="drow" :class="{ 'is-offline': !d.online }" @click="emit('select', d)">
        <Icon :name="ICON[d.kind]" class="drow__icon" />
        <span class="drow__text">
          <span class="drow__name ellipsis">{{ d.name }}</span>
          <span class="drow__meta ellipsis">{{ meta(d) }}</span>
        </span>
      </button>
      <p v-if="!others.length" class="dsec__empty">{{ t('devices.none') }}</p>
    </section>

    <section v-if="out.mode === 'picker'" class="dsec">
      <h3 class="dsec__title">{{ t('devices.routes') }}</h3>
      <button type="button" class="drow" @click="out.showPicker()">
        <Icon name="ph:airplay" class="drow__icon drow__icon--accent" />
        <span class="drow__text">
          <span class="drow__name ellipsis">{{ t('devices.otherDevices') }}</span>
          <span class="drow__meta ellipsis">{{ routeHint }}</span>
        </span>
      </button>
    </section>

    <section v-else-if="out.mode === 'list'" class="dsec">
      <h3 class="dsec__title">{{ t('devices.outputs') }}</h3>
      <button type="button" class="drow" :aria-pressed="out.state.value.followsSystem" @click="out.select(null)">
        <Icon name="ph:speaker-high" class="drow__icon" />
        <span class="drow__text">
          <span class="drow__name ellipsis">{{ t('devices.system') }}</span>
          <span v-if="systemHint" class="drow__meta ellipsis">{{ systemHint }}</span>
        </span>
        <Icon v-if="out.state.value.followsSystem" name="ph:check-bold" class="drow__check" />
      </button>
      <button
        v-for="o in out.state.value.outputs"
        :key="o.id"
        type="button"
        class="drow"
        :aria-pressed="!out.state.value.followsSystem && o.current"
        @click="out.select(o.id)"
      >
        <Icon :name="out.iconOf(o.kind)" class="drow__icon" />
        <span class="drow__text"><span class="drow__name ellipsis">{{ o.name }}</span></span>
        <Icon v-if="!out.state.value.followsSystem && o.current" name="ph:check-bold" class="drow__check" />
      </button>
    </section>
  </div>

  <!-- Большой экран: всплывающая и правая панель -->
  <div v-else class="devs" role="list">
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
    <template v-if="out.mode">
      <div class="pop__sep" role="presentation" />
      <button v-if="out.mode === 'picker'" type="button" role="listitem" class="dev" @click="out.showPicker()">
        <span class="dev__icon"><Icon name="ph:airplay" /></span>
        <span class="dev__text">
          <span class="dev__name ellipsis">{{ t('devices.otherDevices') }}</span>
          <span class="dev__meta ellipsis">{{ routeHint }}</span>
        </span>
      </button>
      <template v-else>
        <button type="button" role="listitem" class="dev" :class="{ 'is-current': out.state.value.followsSystem }" @click="out.select(null)">
          <span class="dev__icon"><Icon name="ph:speaker-high" /></span>
          <span class="dev__text">
            <span class="dev__name ellipsis">{{ t('devices.system') }}</span>
            <span v-if="systemHint" class="dev__meta ellipsis">{{ systemHint }}</span>
          </span>
          <Icon v-if="out.state.value.followsSystem" name="ph:check" class="dev__check" />
        </button>
        <button
          v-for="o in out.state.value.outputs"
          :key="o.id"
          type="button"
          role="listitem"
          class="dev"
          :class="{ 'is-current': !out.state.value.followsSystem && o.current }"
          @click="out.select(o.id)"
        >
          <span class="dev__icon"><Icon :name="out.iconOf(o.kind)" /></span>
          <span class="dev__text"><span class="dev__name ellipsis">{{ o.name }}</span></span>
          <Icon v-if="!out.state.value.followsSystem && o.current" name="ph:check" class="dev__check" />
        </button>
      </template>
    </template>
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
.dev__eq { display: inline-flex; align-items: flex-end; gap: 1.5px; width: 11px; height: 11px; flex: none; color: var(--accent); }
.dev__eq i { flex: 1; height: 100%; border-radius: 1px; background: currentColor; transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.dev__eq i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.dev__eq i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
.dev__eq.is-paused i { animation-play-state: paused; }

/* ---------- Лист на телефоне ---------- */
.dsheet { padding-bottom: 12px; }
.dsec + .dsec { margin-top: 12px; }
.dsec__title { margin: 0; padding: 12px 20px 6px; font-size: 12px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.dsec__empty { margin: 0; padding: 4px 20px 8px; font-size: var(--fs-small); line-height: 1.4; color: var(--text-3); }
.drow {
  position: relative;
  display: flex;
  align-items: center;
  gap: 14px;
  width: 100%;
  min-height: 58px;
  padding: 8px 20px;
  border: 0;
  background: transparent;
  text-align: left;
  color: var(--text-1);
  -webkit-tap-highlight-color: transparent;
}
.drow + .drow::before { content: ''; position: absolute; top: 0; left: 58px; right: 0; height: 0.5px; background: rgba(255, 255, 255, 0.1); }
.drow:active { background: rgba(255, 255, 255, 0.06); }
.drow:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }
.drow__icon { flex: none; width: 24px; font-size: 24px; color: var(--text-2); }
.drow__icon--accent, .drow.is-current .drow__icon { color: var(--accent); }
.drow__text { display: flex; flex: 1; flex-direction: column; gap: 1px; min-width: 0; }
.drow__name { font-size: 17px; font-weight: 500; }
.drow__meta { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: 13px; color: var(--text-3); }
.drow.is-offline .drow__name, .drow.is-offline .drow__icon { color: var(--text-3); }
.drow__check { flex: none; font-size: 18px; color: var(--accent); }
</style>
