<script setup lang="ts">
import type { EmbedSize, EmbedTheme } from '~/utils/export'

// «Поделиться → Встроить»: предпросмотр плеера для чужого сайта, размер и тема; код <iframe> — в буфер обмена.
// Плейлист встраивается только с разрешения владельца — свой можно разрешить прямо здесь.
// На телефоне — лист снизу, на компьютере — окно
const { target, open } = useEmbed()
const bridge = useBridge()
const toast = useToast()
const ui = useUiStore()
const { copy } = useShare()
const i18n = useNuxtApp().$i18n
const { tg } = useGlobalT()

const size = ref<EmbedSize>('large')
const theme = ref<EmbedTheme>('dark')
/** Плейлист: разрешено ли встраивание; null — ещё не знаем */
const allowed = ref<boolean | null>(null)
const saving = ref(false)

const isPlaylist = computed(() => target.value?.kind === 'playlist')
const ready = computed(() => !isPlaylist.value || allowed.value === true)
const src = computed(() => (target.value ? embedUrl(target.value, theme.value, i18n.locale.value) : ''))
const code = computed(() => (target.value ? embedCode(src.value, size.value, target.value.title) : ''))
const segSize = computed(() => (ui.isPhone ? 'sm' : 'md'))

watch(open, async (v) => {
  if (!v || !target.value) return
  // Трек — карточкой, списки — со списком; тема — как у интерфейса
  size.value = target.value.kind === 'track' ? 'compact' : 'large'
  theme.value = document.documentElement.dataset.theme === 'light' ? 'light' : 'dark'
  allowed.value = null
  if (target.value.kind !== 'playlist') return
  const id = target.value.id
  try {
    const on = await bridge.playlists.embed(id)
    if (target.value?.id === id) allowed.value = on
  }
  catch {
    if (target.value?.id === id) allowed.value = false
  }
})

async function setAllowed(on: boolean) {
  const t = target.value
  if (!t || saving.value) return
  saving.value = true
  const was = allowed.value
  allowed.value = on
  try {
    allowed.value = await bridge.playlists.setEmbed(t.id, on)
  }
  catch {
    allowed.value = was
    toast.error(tg('embed.failed'))
  }
  finally {
    saving.value = false
  }
}

async function copyCode() {
  try {
    await copy(code.value)
    bridge.haptic('light')
    toast.show({ text: tg('embed.copied'), icon: 'ph:code' })
  }
  catch {
    toast.error(tg('share.failed'))
  }
}
</script>

<template>
  <UiModal v-model:open="open" :title="tg('embed.title')" :description="target?.title" :width="560">
    <div v-if="target" class="em">
      <SettingsRow v-if="isPlaylist && target.owner" :title="tg('embed.allow')" :description="tg('embed.allowDesc')" class="em__allow">
        <UiSwitch :model-value="allowed === true" :disabled="allowed === null || saving" :label="tg('embed.allow')" hide-label @update:model-value="setAllowed" />
      </SettingsRow>
      <p v-else-if="isPlaylist && allowed === false" class="em__closed">{{ tg('embed.closed') }}</p>

      <template v-if="ready">
        <iframe
          :key="src"
          :src="src"
          :title="target.title"
          :style="{ height: `${EMBED_HEIGHT[size]}px` }"
          class="em__frame"
          allow="autoplay; encrypted-media"
        />
        <div class="em__opts">
          <UiSegmentedControl v-model="size" :options="[{ value: 'compact', label: tg('embed.compact') }, { value: 'large', label: tg('embed.large') }]" :label="tg('embed.size')" :size="segSize" />
          <UiSegmentedControl v-model="theme" :options="[{ value: 'dark', label: tg('embed.dark') }, { value: 'light', label: tg('embed.light') }]" :label="tg('embed.theme')" :size="segSize" />
        </div>
      </template>
    </div>
    <template v-if="ready" #footer>
      <UiButton variant="primary" :size="ui.isPhone ? 'lg' : 'md'" icon="ph:copy" class="em__copy" @click="copyCode">{{ tg('embed.copy') }}</UiButton>
    </template>
  </UiModal>
</template>

<style scoped>
.em { display: flex; flex-direction: column; gap: 16px; padding: 4px 20px 20px; }
.em__allow { border-bottom: 0; }
.em__frame { display: block; flex: none; width: 100%; border: 0; border-radius: 12px; color-scheme: normal; transition: height var(--dur-3) var(--ease-out); }
.em__opts { display: flex; flex-wrap: wrap; gap: 8px; }
.em__closed { margin: 0; padding: 12px 0; font-size: var(--fs-small); color: var(--text-2); }
.em__copy { width: 100%; }
@media (min-width: 700px) {
  .em__copy { width: auto; margin-left: auto; }
}
@media (prefers-reduced-motion: reduce) {
  .em__frame { transition: none; }
}
</style>
