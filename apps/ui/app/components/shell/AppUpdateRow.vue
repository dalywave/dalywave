<script setup lang="ts">
import type { AppUpdateState } from '~/bridge/types'

// Строки «О приложении» про само приложение (macOS, Windows): версия и обновление. Найденное
// обновление качается в фоне само; готово — спокойное «Обновление готово» и «Перезапустить».
// Сами мы не перезапускаемся: музыка не должна оборваться. На сайте и телефонах строк нет
const bridge = useBridge()
const toast = useToast()
const { t } = useScopedI18n('components.shell.AppUpdateRow')
const { tg } = useGlobalT()

const state = ref<AppUpdateState | null>(null)
const busy = ref(false)
let off: (() => void) | null = null
onMounted(async () => {
  off = bridge.updates.onState(s => (state.value = s))
  try {
    state.value = (await bridge.updates.state()) ?? state.value
  }
  catch {
    // Оболочка не ответила — строк просто нет
  }
})
onBeforeUnmount(() => off?.())

const percent = computed(() => (state.value?.progress != null ? Math.floor(state.value.progress * 100) : null))

const line = computed(() => {
  const s = state.value
  if (!s) return ''
  const version = s.update?.version ?? ''
  switch (s.status) {
    case 'checking': return t('checking')
    case 'downloading': return t('downloading', { version })
    case 'ready': return t('ready', { version })
    case 'installing': return t('installing')
    case 'failed': return t('failed')
    default: return t('upToDate')
  }
})

async function check() {
  if (busy.value) return
  busy.value = true
  // Сбой проверки виден в самой строке (`failed`)
  await bridge.updates.check().catch(() => {})
  busy.value = false
}

async function restart() {
  if (busy.value) return
  busy.value = true
  try {
    await bridge.updates.install()
  }
  catch {
    toast.error(tg('app.updateFailed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <template v-if="state">
    <SettingsRow :title="t('version')">
      <span class="value tnum">{{ state.currentVersion }}</span>
    </SettingsRow>
    <SettingsRow
      class="upd"
      :class="`upd--${state.status}`"
      role="status"
      aria-live="polite"
      :description="state.status === 'ready' && state.installOnQuit ? t('onQuit') : undefined"
    >
      <template #title>
        <span class="upd__line">{{ line }}</span><span v-if="percent !== null" class="upd__pct">·&nbsp;{{ percent }}&nbsp;%</span>
      </template>
      <UiButton v-if="state.status === 'ready' || state.status === 'installing'" variant="primary" size="sm" :loading="busy || state.status === 'installing'" @click="restart">{{ t('restart') }}</UiButton>
      <UiButton v-else-if="state.status === 'idle' || state.status === 'failed'" size="sm" :disabled="busy" @click="check">{{ t('check') }}</UiButton>
    </SettingsRow>
  </template>
</template>

<style scoped>
.upd :deep(.sr__title) { flex-wrap: nowrap; min-width: 0; }
.upd__line { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.upd__pct { flex: none; font-variant-numeric: tabular-nums; color: var(--accent); }
.upd--ready .upd__line { color: var(--accent); }
.upd--failed .upd__line { color: var(--warning); }

.value { font-size: var(--fs-small); color: var(--text-2); }
.tnum { font-variant-numeric: tabular-nums; }
</style>
