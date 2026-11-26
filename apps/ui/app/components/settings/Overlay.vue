<script setup lang="ts">
import type { OverlayLink } from '~/api/types'
import type { PlateSide, PlateSize, PlateStyle, PlateTrack } from '~/components/overlay/Plate.vue'
import { copyText } from '~/utils/web-api'

// «Оверлей для стрима»: плашка «Сейчас играет» для OBS (страница /overlay, прозрачный фон). Ссылка
// секретная и даёт только «что играет»; «Сменить ссылку» — прежняя перестаёт работать. Вид (строка или
// карточка, сторона, размер) — параметры ссылки, выбор запоминается на этом устройстве.
// Предпросмотр — та же плашка на шахматке прозрачности с тем, что играет сейчас
const bridge = useBridge()
const player = usePlayerStore()
const pui = usePlaylistUi()
const toast = useToast()
const ui = useUiStore()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const STORE = 'dw.overlay.look'
const link = ref<OverlayLink | null>(null)
const loaded = ref(false)
const busy = ref(false)
const look = ref<PlateStyle>('card')
const side = ref<PlateSide>('left')
const size = ref<PlateSize>('m')

onMounted(async () => {
  try {
    const saved = JSON.parse(localStorage.getItem(STORE) ?? '{}') as { look?: PlateStyle, side?: PlateSide, size?: PlateSize }
    if (saved.look) look.value = saved.look
    if (saved.side) side.value = saved.side
    if (saved.size) size.value = saved.size
  }
  catch {
    // Без хранилища — вид по умолчанию
  }
  try {
    link.value = (await bridge.overlay.get()).link
  }
  catch {
    // Нет сети — строки останутся без ссылки
  }
  loaded.value = true
})

watch([look, side, size], () => {
  try {
    localStorage.setItem(STORE, JSON.stringify({ look: look.value, side: side.value, size: size.value }))
  }
  catch {
    // Запомнится в следующий раз
  }
})

/** Ссылка с выбранным видом */
const url = computed(() => {
  if (!link.value) return ''
  const params = new URLSearchParams()
  if (look.value !== 'card') params.set('style', look.value)
  if (side.value !== 'left') params.set('side', side.value)
  if (size.value !== 'm') params.set('size', size.value)
  const extra = params.toString()
  return extra ? `${link.value.url}&${extra}` : link.value.url
})

const SAMPLE: PlateTrack = { id: 'sample', title: 'Float', artist: 'Call Me Karizma', cover: null }
const preview = computed<PlateTrack>(() => {
  const tr = player.current?.track
  if (!tr) return SAMPLE
  // В приложении адрес картинки относительный к серверу — как у всех обложек, через мост
  const cover = tr.album?.image?.small
  return { id: tr.id, title: tr.title, artist: tr.artists.map(a => a.name).join(', '), cover: cover ? bridge.resolveUrl(cover) : null }
})
const previewAt = ref(Date.now())
watch(() => preview.value.id, () => (previewAt.value = Date.now()))

async function create() {
  busy.value = true
  try {
    link.value = await bridge.overlay.rotate()
    await copy()
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}

async function rotate() {
  const ok = await pui.confirm({ title: t('rotateTitle'), text: t('rotateText'), action: t('rotate') })
  if (!ok) return
  await create()
}

async function disable() {
  const ok = await pui.confirm({ title: t('offTitle'), text: t('offText'), action: t('off'), danger: true })
  if (!ok) return
  try {
    await bridge.overlay.disable()
    link.value = null
  }
  catch {
    toast.error(t('failed'))
  }
}

async function copy() {
  try {
    if (bridge.kind === 'tauri' && bridge.isDesktopApp) {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('plugin:clipboard-manager|write_text', { text: url.value })
    }
    else await copyText(url.value)
    bridge.haptic('light')
    toast.show({ text: t('copied'), icon: 'ph:link' })
  }
  catch {
    toast.error(tg('share.failed'))
  }
}

const hidePrivate = computed({
  get: () => link.value?.hidePrivate ?? true,
  set: (v: boolean) => {
    if (!link.value) return
    const before = link.value
    link.value = { ...before, hidePrivate: v }
    bridge.overlay.update({ hidePrivate: v })
      .then(r => (link.value = r.link))
      .catch(() => {
        link.value = before
        toast.error(t('failed'))
      })
  },
})

const seg = computed(() => (ui.isPhone ? 'sm' : 'md'))
</script>

<template>
  <SettingsGroup v-if="loaded" :title="t('group')">
    <template v-if="!link">
      <SettingsRow :title="t('title')" :description="t('desc')" :stack="ui.isPhone">
        <UiButton size="sm" icon="ph:link" :loading="busy" @click="create">{{ t('create') }}</UiButton>
      </SettingsRow>
    </template>
    <template v-else>
      <div class="ovs__preview" role="img" :aria-label="t('preview')">
        <OverlayPlate
          :track="preview"
          visible
          playing
          :position-ms="player.current ? player.positionMs : 62_000"
          :duration-ms="player.current ? player.durationMs : 169_000"
          :at="previewAt"
          :look="look"
          :side="side"
          :size="size"
        />
      </div>
      <SettingsRow :title="t('look')">
        <UiSegmentedControl v-model="look" :options="[{ value: 'line', label: t('line') }, { value: 'card', label: t('card') }]" :label="t('look')" :size="seg" />
      </SettingsRow>
      <SettingsRow :title="t('side')">
        <UiSegmentedControl v-model="side" :options="[{ value: 'left', label: t('left') }, { value: 'right', label: t('right') }]" :label="t('side')" :size="seg" />
      </SettingsRow>
      <SettingsRow :title="t('size')">
        <UiSegmentedControl v-model="size" :options="[{ value: 's', label: 'S' }, { value: 'm', label: 'M' }, { value: 'l', label: 'L' }]" :label="t('size')" :size="seg" />
      </SettingsRow>
      <SettingsRow :title="t('private')">
        <UiSwitch v-model="hidePrivate" :label="t('private')" hide-label />
      </SettingsRow>
      <SettingsRow :title="t('link')" :description="t('linkDesc')" :stack="ui.isPhone">
        <div class="ovs__actions">
          <UiButton size="sm" variant="ghost" :loading="busy" @click="rotate">{{ t('rotate') }}</UiButton>
          <UiButton size="sm" icon="ph:copy" @click="copy">{{ t('copy') }}</UiButton>
        </div>
      </SettingsRow>
      <SettingsRow as="button" :title="t('off')" danger @click="disable" />
    </template>
  </SettingsGroup>
</template>

<style scoped>
/* Шахматка — как прозрачность в редакторах: видно, что фона у плашки нет */
.ovs__preview {
  --cell: 12px;
  display: block;
  margin: 8px 0;
  padding: 20px 16px;
  border-radius: var(--r-3);
  background: repeating-conic-gradient(var(--fill-2) 0 25%, transparent 0 50%) 0 0 / calc(var(--cell) * 2) calc(var(--cell) * 2);
  overflow: hidden;
}
.ovs__actions { display: flex; gap: 8px; }
@media (max-width: 699px) {
  .ovs__actions { width: 100%; }
  .ovs__actions > * { flex: 1; min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "group": "Оверлей для стрима",
    "title": "Плашка «Сейчас играет»",
    "desc": "Для OBS: прозрачная страница с тем, что играет",
    "create": "Получить ссылку",
    "preview": "Как плашка выглядит на стриме",
    "look": "Вид",
    "line": "Строка",
    "card": "Карточка",
    "side": "Сторона",
    "left": "Слева",
    "right": "Справа",
    "size": "Размер",
    "private": "Прятать в приватной сессии",
    "link": "Ссылка",
    "linkDesc": "OBS → Источник → Браузер",
    "copy": "Скопировать ссылку",
    "copied": "Ссылка скопирована",
    "rotate": "Сменить ссылку",
    "rotateTitle": "Сменить ссылку?",
    "rotateText": "Прежняя ссылка перестанет работать — в OBS нужно будет вставить новую.",
    "off": "Выключить оверлей",
    "offTitle": "Выключить оверлей?",
    "offText": "Ссылка перестанет работать. Новую можно получить в любой момент.",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "en": {
    "group": "Stream overlay",
    "title": "Now Playing card",
    "desc": "For OBS: a transparent page with what’s playing",
    "create": "Get link",
    "preview": "How the card looks on stream",
    "look": "Style",
    "line": "Line",
    "card": "Card",
    "side": "Side",
    "left": "Left",
    "right": "Right",
    "size": "Size",
    "private": "Hide in a private session",
    "link": "Link",
    "linkDesc": "OBS → Source → Browser",
    "copy": "Copy link",
    "copied": "Link copied",
    "rotate": "New link",
    "rotateTitle": "Make a new link?",
    "rotateText": "The old link will stop working — paste the new one into OBS.",
    "off": "Turn off overlay",
    "offTitle": "Turn off the overlay?",
    "offText": "The link will stop working. You can get a new one any time.",
    "failed": "Didn’t work — try again"
  }
}
</i18n>
