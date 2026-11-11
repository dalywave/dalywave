<script setup lang="ts">
import type { ConnectKind } from '~/api/types'

// «Играет на: MacBook» — плеер показывает другое устройство (это — пульт). Нажатие открывает
// «Где играть». Устройство пропало из сети — «Продолжить здесь» с того же места.
// `line` — строка под названием (мини-плеер, панель плеера), `pill` — плашка в большом плеере
withDefaults(defineProps<{ variant?: 'line' | 'pill' }>(), { variant: 'pill' })

const connect = useConnectStore()
const ui = useUiStore()
const { t } = useI18n()

const ICON: Record<ConnectKind, string> = {
  laptop: 'ph:laptop',
  desktop: 'ph:desktop',
  phone: 'ph:device-mobile',
  tablet: 'ph:device-tablet',
  web: 'ph:browser',
}

const show = computed(() => connect.remote || connect.orphaned)
const label = computed(() => (connect.orphaned ? t('connect.lost', { name: connect.activeName }) : t('connect.playingOn', { name: connect.activeName })))
</script>

<template>
  <div v-if="show" class="cc" :class="[`cc--${variant}`, { 'is-lost': connect.orphaned }]" data-no-dismiss>
    <button type="button" class="cc__main" :title="connect.orphaned ? t('connect.lostHint', { name: connect.activeName }) : undefined" @click.stop="ui.openPanel('devices')">
      <Icon :name="connect.orphaned ? 'ph:wifi-slash' : ICON[connect.activeKind]" class="cc__icon" />
      <span class="cc__text ellipsis">{{ label }}</span>
    </button>
    <button v-if="connect.orphaned" type="button" class="cc__act" @click.stop="connect.continueHere(true)">
      <Icon name="ph:play-fill" class="cc__act-icon" />{{ t('connect.continueHere') }}
    </button>
  </div>
</template>

<style scoped>
.cc { display: flex; align-items: center; gap: 8px; min-width: 0; color: var(--accent); }
.cc__main { display: inline-flex; align-items: center; gap: 6px; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font-weight: 600; text-align: left; }
.cc__icon { flex: none; }
.cc__text { min-width: 0; }
.cc__main:focus-visible, .cc__act:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: var(--r-pill); }
.cc__act {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex: none;
  height: 26px;
  padding: 0 10px 0 8px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--text-1);
  color: var(--on-light);
  font-size: var(--fs-caption);
  font-weight: 650;
  transition: filter var(--dur-1);
}
.cc__act:hover { filter: brightness(0.94); }
.cc__act:active { filter: brightness(0.86); }
.cc__act-icon { font-size: 12px; }
.cc.is-lost { color: var(--text-2); }

/* Строка: мини-плеер и панель плеера */
.cc--line { font-size: var(--fs-caption); line-height: 1.3; }
.cc--line .cc__icon { font-size: 13px; }
.cc--line .cc__main:hover .cc__text { text-decoration: underline; text-underline-offset: 3px; }

/* Плашка: большой плеер */
.cc--pill { justify-content: center; }
.cc--pill .cc__main {
  height: 32px;
  padding: 0 14px 0 12px;
  border-radius: var(--r-pill);
  background: var(--accent-soft);
  font-size: var(--fs-small);
  transition: background-color var(--dur-1);
}
.cc--pill .cc__main:hover { background: color-mix(in oklab, var(--accent-soft) 70%, var(--accent) 30%); }
.cc--pill .cc__icon { font-size: 16px; }
.cc--pill .cc__act { height: 32px; padding: 0 14px 0 12px; font-size: var(--fs-small); }
.cc--pill.is-lost .cc__main { background: var(--fill-2); }
</style>
