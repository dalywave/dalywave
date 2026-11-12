<script setup lang="ts">
import type { PlaylistDetail, PlaylistItem, PlaylistSummary } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Правка своего плейлиста, как в Apple Music: название, описание, обложка; строки — «⊖» слева,
// ручка справа. Убрать и переставить применяются сразу (у удаления — «Отменить»), название
// и описание — по «Готово». На телефоне строку можно смахнуть влево
const props = defineProps<{ detail: PlaylistDetail }>()
const emit = defineEmits<{ done: [] }>()

const bridge = useBridge()
const toast = useToast()
const ui = useUiStore()
const library = useLibraryStore()
const { t } = useI18n()

const pl = ref<PlaylistSummary>({ ...props.detail.playlist })
const items = ref<PlaylistItem[]>([...props.detail.items])
const title = ref(props.detail.playlist.title)
const description = ref(props.detail.playlist.description)
const saving = ref(false)
const coverBusy = ref(false)

const list = ref<HTMLElement | null>(null)
const sortable = useSortable({ container: list, onMove: move })

const titleOk = computed(() => !!title.value.trim())
const covers = computed(() => {
  // Мозаика по текущему порядку: первые четыре разных альбома
  const seen = new Set<string>()
  const out = []
  for (const i of items.value) {
    const a = i.track.album
    if (!a?.image || seen.has(a.id)) continue
    seen.add(a.id)
    out.push(a.image)
    if (out.length === 4) break
  }
  return out
})

// ---------- Перестановка ----------

async function move(from: number, to: number) {
  if (from === to || to < 0 || to >= items.value.length) return
  const before = [...items.value]
  const next = [...before]
  const [it] = next.splice(from, 1)
  next.splice(to, 0, it!)
  items.value = next
  bridge.haptic('selection')
  try {
    await bridge.playlists.moveItem(pl.value.id, it!.id, to === 0 ? null : next[to - 1]!.id)
  }
  catch {
    items.value = before
    toast.error(t('pl.failed'))
  }
}

/** Alt + ↑/↓ на строке — на место выше или ниже; фокус едет со строкой */
async function onKey(i: number, e: KeyboardEvent) {
  if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return
  e.preventDefault()
  const to = e.key === 'ArrowUp' ? i - 1 : i + 1
  if (to < 0 || to >= items.value.length) return
  const id = items.value[i]!.id
  await move(i, to)
  await nextTick()
  list.value?.querySelector<HTMLElement>(`[data-item="${id}"]`)?.focus()
}

// ---------- Убрать ----------

async function removeAt(i: number) {
  const it = items.value[i]
  if (!it) return
  const prev = items.value[i - 1]?.id ?? null
  items.value = items.value.filter(x => x.id !== it.id)
  swiped.value = null
  try {
    await bridge.playlists.removeItem(pl.value.id, it.id)
  }
  catch {
    items.value.splice(i, 0, it)
    toast.error(t('pl.failed'))
    return
  }
  toast.show({
    text: t('pl.removed', { title: it.track.title }),
    icon: 'ph:minus-circle',
    undo: () => void restore(it, prev, i),
  })
}

async function restore(it: PlaylistItem, after: string | null, index: number) {
  try {
    const res = await bridge.playlists.addItems(pl.value.id, after ? { trackIds: [it.track.id], afterItemId: after } : { trackIds: [it.track.id], atStart: true })
    const back = res.items[0]
    if (!back) return
    const at = after ? items.value.findIndex(x => x.id === after) + 1 : 0
    items.value.splice(at >= 0 ? at : Math.min(index, items.value.length), 0, back)
  }
  catch {
    toast.error(t('pl.failed'))
  }
}

// ---------- Смахнуть влево (палец) ----------

const swiped = ref<string | null>(null)
const swipeX = ref(0)
const swipeId = ref<string | null>(null)
let sx = 0
let sy = 0
let horizontal: boolean | null = null
const REVEAL = 88

function swipeStart(it: PlaylistItem, e: PointerEvent) {
  if (e.pointerType !== 'touch' || sortable.dragging.value) return
  if ((e.target as HTMLElement).closest('.ed__handle, .ed__minus, .ed__reveal')) return
  sx = e.clientX
  sy = e.clientY
  horizontal = null
  swipeId.value = it.id
  swipeX.value = swiped.value === it.id ? -REVEAL : 0
}
function swipeMove(e: PointerEvent) {
  if (!swipeId.value) return
  const dx = e.clientX - sx
  const dy = e.clientY - sy
  if (horizontal === null && Math.hypot(dx, dy) > 8) horizontal = Math.abs(dx) > Math.abs(dy)
  if (!horizontal) return
  const base = swiped.value === swipeId.value ? -REVEAL : 0
  swipeX.value = Math.min(0, base + dx)
}
function swipeEnd(i: number, e: PointerEvent) {
  const id = swipeId.value
  if (!id) return
  swipeId.value = null
  if (!horizontal) return
  const width = (e.currentTarget as HTMLElement).offsetWidth
  if (swipeX.value < -width * 0.5) {
    void removeAt(i)
    return
  }
  swiped.value = swipeX.value < -REVEAL / 2 ? id : null
}
function rowX(it: PlaylistItem): number {
  if (swipeId.value === it.id && horizontal) return swipeX.value
  return swiped.value === it.id ? -REVEAL : 0
}

// ---------- Обложка ----------

const fileInput = ref<HTMLInputElement | null>(null)
const coverMenu = computed<MenuEntry[]>(() => [
  { label: t('pl.coverChoose'), icon: 'ph:image', onSelect: () => fileInput.value?.click() },
  ...(pl.value.image ? [{ label: t('pl.coverMosaic'), icon: 'ph:squares-four', onSelect: () => void dropCover() }] : []),
])

async function onFile(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  input.value = ''
  if (!file) return
  coverBusy.value = true
  try {
    pl.value = await bridge.playlists.setCover(pl.value.id, file)
    void library.loadPlaylists(true).catch(() => {})
  }
  catch {
    toast.error(t('pl.coverFailed'))
  }
  finally {
    coverBusy.value = false
  }
}

async function dropCover() {
  coverBusy.value = true
  try {
    pl.value = await bridge.playlists.removeCover(pl.value.id)
    void library.loadPlaylists(true).catch(() => {})
  }
  catch {
    toast.error(t('pl.failed'))
  }
  finally {
    coverBusy.value = false
  }
}

// ---------- Готово ----------

async function done() {
  if (!titleOk.value) {
    toast.error(t('pl.titleRequired'))
    return
  }
  const patch: { title?: string, description?: string } = {}
  if (title.value.trim() !== props.detail.playlist.title) patch.title = title.value.trim()
  if (description.value.trim() !== props.detail.playlist.description) patch.description = description.value.trim()
  if (patch.title !== undefined || patch.description !== undefined) {
    saving.value = true
    try {
      await bridge.playlists.update(pl.value.id, patch)
      void library.loadPlaylists(true).catch(() => {})
    }
    catch {
      toast.error(t('pl.failed'))
      saving.value = false
      return
    }
    saving.value = false
  }
  emit('done')
}

// Описание растёт по тексту
const descEl = ref<HTMLTextAreaElement | null>(null)
function grow() {
  const el = descEl.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${el.scrollHeight}px`
}
onMounted(() => nextTick(grow))
</script>

<template>
  <div class="ed" :class="{ 'is-sorting': sortable.dragging.value }">
    <!-- Телефон: своя верхняя строка с «Готово» -->
    <div v-if="ui.isPhone" class="ed__bar">
      <span class="ed__bar-title">{{ t('pl.edit') }}</span>
      <UiButton variant="ghost" class="ed__bar-done" :loading="saving" :disabled="!titleOk" @click="done">{{ t('pl.done') }}</UiButton>
    </div>

    <header class="ed__head">
      <UiMenu :items="coverMenu" align="start">
        <button type="button" class="ed__cover" :aria-label="t('pl.coverChange')" :aria-busy="coverBusy || undefined">
          <MusicPlaylistCover :id="pl.id" :title="title" :covers="covers" :image="pl.image" :kind="pl.kind" :show-title="false" />
          <span class="ed__cover-badge" aria-hidden="true">
            <span v-if="coverBusy" class="ed__spin" />
            <Icon v-else name="ph:camera" />
          </span>
        </button>
      </UiMenu>
      <input ref="fileInput" type="file" accept="image/jpeg,image/png,image/webp" class="ed__file" tabindex="-1" aria-hidden="true" @change="onFile">
      <div class="ed__fields">
        <label class="ed__label" for="pl-title">{{ t('pl.titleLabel') }}</label>
        <input id="pl-title" v-model="title" class="ed__title" maxlength="200" :placeholder="t('pl.titleLabel')" enterkeyhint="done" @keydown.enter.prevent="done">
        <label class="ed__label" for="pl-desc">{{ t('pl.descLabel') }}</label>
        <textarea id="pl-desc" ref="descEl" v-model="description" class="ed__desc" rows="1" maxlength="2000" :placeholder="t('pl.descPlaceholder')" @input="grow" />
        <div v-if="!ui.isPhone" class="ed__actions">
          <UiButton variant="primary" size="lg" :loading="saving" :disabled="!titleOk" @click="done">{{ t('pl.done') }}</UiButton>
        </div>
      </div>
    </header>

    <div ref="list" class="ed__list" role="list" :aria-label="title">
      <div
        v-for="(it, i) in items"
        :key="it.id"
        :data-sort-i="i"
        :data-item="it.id"
        class="ed__row"
        :class="{ 'is-drag': sortable.from.value === i, 'is-swiped': swiped === it.id }"
        role="listitem"
        tabindex="0"
        :aria-label="`${it.track.title} — ${it.track.artists.map(a => a.name).join(', ')}`"
        :style="{ transform: `translate3d(0, ${sortable.offset(i)}px, 0)` }"
        @keydown="onKey(i, $event)"
      >
        <button type="button" class="ed__reveal" :tabindex="swiped === it.id ? 0 : -1" @click="removeAt(i)">{{ t('pl.remove') }}</button>
        <div
          class="ed__slide"
          :style="{ transform: rowX(it) ? `translate3d(${rowX(it)}px, 0, 0)` : undefined }"
          :class="{ 'is-moving': swipeId === it.id }"
          @pointerdown="swipeStart(it, $event)"
          @pointermove="swipeMove"
          @pointerup="swipeEnd(i, $event)"
          @pointercancel="swipeId = null"
        >
          <button type="button" class="ed__minus" :aria-label="t('pl.removeTrack', { title: it.track.title })" @click="removeAt(i)">
            <Icon name="ph:minus-circle-fill" />
          </button>
          <MusicCoverArt :image="it.track.album?.image" size="small" :alt="''" radius="6px" class="ed__art" />
          <span class="ed__text">
            <span class="ed__name ellipsis">{{ it.track.title }}</span>
            <span class="ed__sub ellipsis">{{ it.track.artists.map(a => a.name).join(', ') }}</span>
          </span>
          <span v-if="!ui.isPhone" class="ed__dur tnum">{{ formatDuration(it.track.durationMs) }}</span>
          <span
            class="ed__handle"
            role="button"
            tabindex="-1"
            :aria-label="t('pl.drag', { title: it.track.title })"
            @pointerdown="sortable.start(i, $event)"
          >
            <Icon name="ph:list" />
          </span>
        </div>
      </div>
      <p v-if="!items.length" class="ed__empty">{{ t('pl.editEmpty') }}</p>
    </div>
  </div>
</template>

<style scoped>
.ed { display: flex; flex-direction: column; gap: 24px; }
.ed__bar {
  position: sticky;
  top: 0;
  z-index: 30;
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  align-items: center;
  height: calc(var(--safe-top) + 52px);
  margin-top: calc(var(--safe-top) * -1);
  padding: var(--safe-top) 8px 0 16px;
  background: var(--glass-bar);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
  backdrop-filter: blur(30px) saturate(1.4);
  box-shadow: 0 1px 0 var(--hair);
}
.ed__bar-title { grid-column: 2; font-weight: 650; }
.ed__bar-done { grid-column: 3; justify-self: end; height: 44px; color: var(--accent); font-weight: 650; }

.ed__head { display: flex; align-items: flex-end; gap: 32px; padding: 24px 0 0; }
.ed__cover { position: relative; display: block; width: 232px; flex: none; padding: 0; border: 0; border-radius: var(--r-4); background: none; cursor: pointer; }
.ed__cover :deep(.pc) { width: 100%; border-radius: var(--r-4); box-shadow: var(--shadow-cover); }
.ed__cover:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.ed__cover-badge { position: absolute; right: 10px; bottom: 10px; display: grid; place-items: center; width: 40px; height: 40px; border-radius: 50%; background: rgba(10, 12, 16, 0.72); color: #fff; font-size: 20px; transition: transform var(--dur-2) var(--ease-out); }
.ed__cover:hover .ed__cover-badge { transform: scale(1.06); }
.ed__spin { width: 18px; height: 18px; border: 2px solid rgba(255, 255, 255, 0.3); border-top-color: #fff; border-radius: 50%; animation: spin 0.8s linear infinite; }
.ed__file { position: absolute; width: 1px; height: 1px; opacity: 0; pointer-events: none; }

.ed__fields { display: flex; flex: 1; flex-direction: column; gap: 6px; min-width: 0; }
.ed__label { font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.ed__title, .ed__desc { width: 100%; margin: 0 0 10px; padding: 8px 0; border: 0; border-bottom: 1px solid var(--line); border-radius: 0; background: none; color: var(--text-1); font: inherit; outline: none; transition: border-color var(--dur-1); }
.ed__title { font-size: clamp(28px, 3.4vw, 44px); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.1; }
.ed__desc { min-height: 40px; resize: none; overflow: hidden; font-size: var(--fs-body); color: var(--text-2); line-height: 1.45; }
.ed__title:focus, .ed__desc:focus { border-bottom-color: var(--text-2); }
.ed__title::placeholder, .ed__desc::placeholder { color: var(--text-4); }
.ed__actions { display: flex; gap: 10px; margin-top: 6px; }

.ed__list { position: relative; display: flex; flex-direction: column; }
.ed__row { position: relative; border-radius: var(--r-3); outline: none; transition: transform 180ms var(--ease-out); }
.ed__row:focus-visible { box-shadow: inset 0 0 0 2px var(--focus); }
.ed__row.is-drag { z-index: 5; transition: none; }
.ed__row.is-drag .ed__slide { background: var(--ink-3); box-shadow: 0 12px 32px -10px rgba(0, 0, 0, 0.8), 0 0 0 1px var(--line); }
.ed:not(.is-sorting) .ed__row { transition: none; }
.ed__slide {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: 32px 40px minmax(0, 1fr) auto auto;
  align-items: center;
  column-gap: 12px;
  min-height: 56px;
  padding: 8px 4px 8px 6px;
  border-radius: var(--r-3);
  touch-action: pan-y;
  transition: transform 220ms var(--ease-out), background-color var(--dur-1);
}
.ed__slide.is-moving { transition: none; }
/* Под смахнутой строкой — красная кнопка: строка закрывает её своим фоном */
.ed__row.is-swiped .ed__slide, .ed__slide.is-moving { background: var(--ink-1); }
.ed__row:hover .ed__slide { background: var(--fill-1); }
.ed__row.is-drag .ed__slide, .ed__row:hover.is-drag .ed__slide { background: var(--ink-3); }
.ed__minus { display: grid; place-items: center; width: 32px; height: 32px; padding: 0; border: 0; border-radius: 50%; background: none; color: var(--danger); font-size: 22px; cursor: pointer; }
.ed__minus:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.ed__art { width: 40px; height: 40px; }
.ed__text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
.ed__name { font-weight: 550; }
.ed__sub { font-size: var(--fs-small); color: var(--text-2); }
.ed__dur { width: 44px; font-size: var(--fs-small); color: var(--text-3); text-align: right; }
.ed__handle { display: grid; place-items: center; width: 44px; height: 44px; color: var(--text-3); font-size: 20px; cursor: grab; touch-action: none; }
.ed__handle:hover { color: var(--text-1); }
.is-sorting, .is-sorting * { cursor: grabbing !important; user-select: none; }
.ed__reveal { position: absolute; top: 2px; right: 0; bottom: 2px; width: calc(88px + 16px); padding-left: 16px; border: 0; border-radius: var(--r-3); background: var(--danger); color: #fff; font: inherit; font-weight: 650; cursor: pointer; }
.ed__row:not(.is-swiped) .ed__reveal { visibility: hidden; }
.ed__row:has(.ed__slide.is-moving) .ed__reveal { visibility: visible; }
.ed__empty { margin: 12px 0; color: var(--text-3); }

@media (max-width: 1023px) {
  .ed__head { gap: 24px; }
  .ed__cover { width: 184px; }
}
@media (max-width: 699px) {
  .ed { gap: 16px; }
  .ed__head { flex-direction: column; align-items: stretch; gap: 20px; padding: 8px 16px 0; }
  .ed__cover { align-self: center; width: min(220px, 58vw); }
  .ed__desc { font-size: 16px; }
  .ed__title { font-size: 22px; font-weight: 700; }
  .ed__list { padding: 0 8px; }
  .ed__slide { grid-template-columns: 32px 40px minmax(0, 1fr) auto; min-height: 60px; padding-right: 0; }
  .ed__row:hover:not(.is-swiped) .ed__slide:not(.is-moving) { background: none; }
}
</style>
