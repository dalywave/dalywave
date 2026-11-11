<script setup lang="ts">
// Строка заголовка окна приложения (договор apps/shell/BRIDGE.md). На сайте не показывается.
// macOS: «светофор» рисует система (x 18–78, по центру строки 48); мы оставляем место и полосу,
// за которую окно тянется. Windows: своя строка 32 с кнопками 46 × 32; над «развернуть» —
// меню привязки окон (подсветка — по data-caption-*, потому что над кнопкой системный слой)
const props = withDefaults(defineProps<{
  /** Галерея: показать вид без окна */
  controls?: 'mac-inset' | 'windows'
  preview?: boolean
  focused?: boolean
  maximized?: boolean
}>(), { focused: true, maximized: false })

const bridge = useBridge()
const { t } = useI18n()
const kind = computed(() => props.controls ?? bridge.chrome?.controls ?? (bridge.platform === 'macos' ? 'mac-inset' : 'windows'))
const height = computed(() => bridge.chrome?.height ?? (kind.value === 'mac-inset' ? 48 : 32))
const isMax = ref(props.maximized)
watch(() => props.maximized, v => (isMax.value = v))

const maxBtn = ref<HTMLElement | null>(null)
let stopSnap: (() => void) | null = null
onMounted(() => {
  if (kind.value === 'windows' && maxBtn.value && !props.preview) stopSnap = bridge.window.trackSnap(maxBtn.value)
})
onBeforeUnmount(() => stopSnap?.())

async function toggleMax() {
  isMax.value = await bridge.window.toggleMaximize()
}
</script>

<template>
  <div v-if="kind === 'mac-inset'" class="tb tb--mac" :class="{ 'tb--preview': preview }" :style="{ height: `${height}px` }" data-tauri-drag-region="deep">
    <span v-if="preview" class="tb__lights" aria-hidden="true"><i /><i /><i /></span>
  </div>
  <div v-else class="tb tb--win" :class="{ 'tb--preview': preview, 'is-blurred': !focused }" :style="{ height: `${height}px` }" data-tauri-drag-region="deep">
    <span class="tb__title"><BrandGlyph class="tb__glyph" />DalyWave</span>
    <div class="tb__buttons">
      <button type="button" class="tb__btn" :aria-label="t('window.minimize')" @click="bridge.window.minimize()">
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5.5h10" /></svg>
      </button>
      <button ref="maxBtn" type="button" class="tb__btn tb__btn--max" :aria-label="isMax ? t('window.restore') : t('window.maximize')" @click="toggleMax">
        <svg v-if="isMax" viewBox="0 0 10 10" aria-hidden="true"><path d="M2.5 2.5V.5h7v7h-2M.5 2.5h7v7h-7z" /></svg>
        <svg v-else viewBox="0 0 10 10" aria-hidden="true"><path d="M.5.5h9v9h-9z" /></svg>
      </button>
      <button type="button" class="tb__btn tb__btn--close" :aria-label="t('window.close')" @click="bridge.window.close()">
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M.5.5l9 9M9.5.5l-9 9" /></svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.tb { position: fixed; top: 0; left: 0; z-index: 80; user-select: none; }
.tb--mac { width: var(--sidebar-w); }
.tb--win { right: 0; display: flex; align-items: center; justify-content: space-between; padding-left: 14px; background: var(--ink-0); border-bottom: 1px solid var(--hair); transition: opacity var(--dur-2); }
.tb--win.is-blurred .tb__title, .tb--win.is-blurred .tb__btn { color: var(--text-4); }
.tb--preview { position: relative; z-index: auto; width: 100%; overflow: hidden; border-radius: var(--r-3); }
.tb--mac.tb--preview { background: var(--ink-0); box-shadow: inset 0 0 0 1px var(--hair); }
.tb__lights { position: absolute; left: 18px; top: 50%; display: flex; gap: 9px; transform: translateY(-50%); }
.tb__lights i { width: 14px; height: 14px; border-radius: 50%; background: #ff5f57; }
.tb__lights i:nth-child(2) { background: #febc2e; }
.tb__lights i:nth-child(3) { background: #28c840; }
.tb__title { display: inline-flex; align-items: center; gap: 8px; font-size: var(--fs-caption); font-weight: 600; color: var(--text-2); }
.tb__glyph { width: 16px; height: 16px; }
.tb__buttons { display: flex; height: 100%; }
.tb__btn { display: grid; place-items: center; width: 46px; height: 100%; padding: 0; border: 0; background: transparent; color: var(--text-1); transition: background-color var(--dur-1), color var(--dur-1); }
.tb__btn svg { width: 10px; height: 10px; overflow: visible; fill: none; stroke: currentColor; stroke-width: 1; }
.tb__btn:hover, .tb__btn--max[data-caption-hover] { background: var(--fill-2); }
.tb__btn:active, .tb__btn--max[data-caption-pressed] { background: var(--fill-3); }
.tb__btn--close:hover { background: #c42b1c; color: #fff; }
.tb__btn--close:active { background: #c42b1ccc; }
</style>
