<script setup lang="ts">
import type { ConnectKind } from '~/api/types'

// «Играет на: MacBook» — плеер показывает другое устройство (это — пульт). Нажатие открывает
// «Где играть». Устройство пропало из сети — тихое «Было на: MacBook»: продолжить здесь с того же
// места можно обычной кнопкой ▶, отдельной плашки нет
// `line` — строка под названием (мини-плеер, панель плеера), `pill` — строка по центру в большом плеере
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
const label = computed(() => (connect.orphaned ? t('connect.wasOn', { name: connect.activeName }) : t('connect.playingOn', { name: connect.activeName })))
</script>

<template>
  <div v-if="show" class="cc" :class="[`cc--${variant}`, { 'is-lost': connect.orphaned }]" data-no-dismiss>
    <button type="button" class="cc__main" :title="connect.orphaned ? t('connect.lostHint', { name: connect.activeName }) : undefined" @click.stop="ui.openPanel('devices')">
      <Icon v-if="!connect.orphaned" :name="ICON[connect.activeKind]" class="cc__icon" />
      <span class="cc__text ellipsis">{{ label }}</span>
    </button>
  </div>
</template>

<style scoped>
.cc { display: flex; align-items: center; gap: 8px; min-width: 0; color: var(--accent); }
.cc__main { display: inline-flex; align-items: center; gap: 6px; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font-weight: 600; text-align: left; }
.cc__icon { flex: none; }
.cc__text { min-width: 0; }
.cc__main:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: var(--r-pill); }
.cc.is-lost { color: var(--text-3); }
.cc.is-lost .cc__main { font-weight: 500; }

/* Строка: мини-плеер и панель плеера */
.cc--line { font-size: var(--fs-caption); line-height: 1.3; }
.cc--line .cc__icon { font-size: 13px; }
.cc--line .cc__main:hover .cc__text { text-decoration: underline; text-underline-offset: 3px; }

/* Большой плеер: строка по центру акцентом, без плашки; высота — под палец */
.cc--pill { justify-content: center; }
.cc--pill .cc__main { min-height: 44px; padding: 0 8px; font-size: var(--fs-small); }
.cc--pill .cc__main:hover .cc__text { text-decoration: underline; text-underline-offset: 3px; }
.cc--pill .cc__icon { font-size: 16px; }
</style>
