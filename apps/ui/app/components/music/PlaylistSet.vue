<script setup lang="ts">
import { ApiError } from '~/api/types'
import type { PlaylistDetail, PlaylistItem, SetCurve, SetOptions, SetPreview } from '~/api/types'

// «Собрать как сет»: кривая энергии, первый трек, предпросмотр нового порядка — линия энергии
// и список с мягкими подсказками («в тон», «плавно»), затем «Применить» с «Отменить» в уведомлении.
// Одно окно на приложение — открывается из меню плейлиста и из шапки (usePlaylistUi().openSet).
// Микс собирается сам: его сначала сохраняют копией, а собирают уже копию
const pui = usePlaylistUi()
const bridge = useBridge()
const toast = useToast()
const library = useLibraryStore()
const connect = useConnectStore()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n()

const CURVES: { value: SetCurve, path: string }[] = [
  { value: 'rise', path: 'M2 15 C 9 14.5, 16 10, 26 3' },
  { value: 'wave', path: 'M2 15 C 7 14, 12 3, 18 3 S 24 9, 26 10' },
  { value: 'flat', path: 'M2 9.5 C 7 8, 11 11, 15 9.5 S 22 8, 26 9.5' },
]

const curve = ref<SetCurve>('rise')
const firstId = ref<string | null>(null)
const view = ref<'set' | 'first'>('set')
const q = ref('')
const detail = shallowRef<PlaylistDetail | null>(null)
const preview = shallowRef<SetPreview | null>(null)
const loading = ref(false)
const failed = ref<string | null>(null)
const busy = ref(false)
const dj = ref(true)

const pl = computed(() => pui.setFor)
const readOnly = computed(() => !!pl.value && pl.value.kind !== 'user')
/** «Диджей» есть там, где играет звук, и он ещё не включён */
const offerDj = computed(() => connect.djAvailable && connect.transition !== 'dj')

let seq = 0
async function loadPreview() {
  const p = pl.value
  if (!p) return
  const my = ++seq
  loading.value = true
  failed.value = null
  try {
    const opts: SetOptions = { curve: curve.value, firstItemId: firstId.value }
    const res = await bridge.playlists.setPreview(p.id, opts)
    if (my === seq) preview.value = res
  }
  catch (e) {
    if (my === seq) failed.value = e instanceof ApiError && e.code === 'too_long' ? t('pl.set.tooLong') : t('pl.failed')
  }
  finally {
    if (my === seq) loading.value = false
  }
}

watch(() => pui.setOpen, async (open) => {
  if (!open) return
  curve.value = 'rise'
  firstId.value = null
  view.value = 'set'
  q.value = ''
  dj.value = true
  preview.value = null
  detail.value = null
  const p = pl.value
  if (!p) return
  void loadPreview()
  try {
    detail.value = await bridge.playlists.get(p.id)
  }
  catch {
    failed.value = t('pl.failed')
  }
})

watch([curve, firstId], () => {
  if (pui.setOpen) void loadPreview()
})

const byId = computed(() => new Map((detail.value?.items ?? []).map(i => [i.id, i])))
const rows = computed(() => (preview.value?.entries ?? []).flatMap((e) => {
  const item = byId.value.get(e.itemId)
  return item ? [{ entry: e, item }] : []
}))
const firstItem = computed(() => (firstId.value ? byId.value.get(firstId.value) ?? null : null))
const ready = computed(() => !!preview.value && !!detail.value)

// ---------- Линия энергии ----------

const W = 320
const H = 56
const PAD = 3

/** Энергия расставленных треков, сглаженная скользящим средним: на длинном плейлисте линия
 *  показывает ход сета, а не каждый трек */
const line = computed(() => {
  const raw = (preview.value?.entries ?? []).filter(e => e.placed && e.energy != null).map(e => e.energy as number)
  if (raw.length < 2) return ''
  const r = Math.max(0, Math.round(raw.length / 24))
  const pts = raw.map((_, i) => {
    const s = raw.slice(Math.max(0, i - r), i + r + 1)
    return s.reduce((a, b) => a + b, 0) / s.length
  })
  const lo = Math.min(...pts)
  const span = Math.max(0.08, Math.max(...pts) - lo)
  const xy = pts.map((v, i) => [PAD + (i * (W - 2 * PAD)) / (pts.length - 1), H - PAD - ((v - lo) / span) * (H - 2 * PAD)] as const)
  // Сплайн Катмулла — Рома в кривые Безье: плавно и точно через точки
  let d = `M${xy[0]![0].toFixed(1)} ${xy[0]![1].toFixed(1)}`
  for (let i = 0; i < xy.length - 1; i++) {
    const p0 = xy[Math.max(0, i - 1)]!
    const p1 = xy[i]!
    const p2 = xy[i + 1]!
    const p3 = xy[Math.min(xy.length - 1, i + 2)]!
    const c1 = [p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6]
    const c2 = [p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6]
    d += ` C${c1[0]!.toFixed(1)} ${c1[1]!.toFixed(1)}, ${c2[0]!.toFixed(1)} ${c2[1]!.toFixed(1)}, ${p2[0].toFixed(1)} ${p2[1].toFixed(1)}`
  }
  return d
})

const summary = computed(() => {
  const p = preview.value
  if (!p) return ''
  if (!p.changed) return t('pl.set.same')
  const total = Math.max(0, p.entries.length - 1)
  return t('pl.set.smooth', { after: p.smoothAfter, total, before: p.smoothBefore })
})

// ---------- Первый трек ----------

const picks = computed(() => {
  const all = detail.value?.items ?? []
  const needle = q.value.trim().toLocaleLowerCase()
  if (!needle) return all
  return all.filter(i => `${i.track.title} ${artistNames(i.track.artists)}`.toLocaleLowerCase().includes(needle))
})

function chooseFirst(item: PlaylistItem | null) {
  firstId.value = item?.id ?? null
  view.value = 'set'
  q.value = ''
}

// ---------- Применить ----------

async function apply() {
  const p = pl.value
  if (!p || busy.value || !preview.value) return
  busy.value = true
  try {
    let targetId = p.id
    let first = firstId.value
    if (readOnly.value) {
      // Копия хранит тот же порядок: первый трек — на том же месте среди её позиций
      const copy = await bridge.playlists.duplicate(p.id, t('pl.set.copyTitle', { title: p.title }))
      targetId = copy.id
      if (first) {
        const at = (detail.value?.items ?? []).findIndex(i => i.id === first)
        first = at >= 0 ? ((await bridge.playlists.get(copy.id)).items[at]?.id ?? null) : null
      }
    }
    const res = await bridge.playlists.applySet(targetId, { curve: curve.value, firstItemId: first })
    const prevMode = connect.transition
    const djOn = offerDj.value && dj.value
    if (djOn) connect.transition = 'dj'
    pui.setOpen = false
    pui.reordered = { id: targetId, at: Date.now() }
    void library.loadPlaylists(true).catch(() => {})
    if (readOnly.value) await ui.navigate(`/playlist/${targetId}`)
    toast.show({
      text: djOn ? t('pl.set.appliedDj') : t('pl.set.applied'),
      icon: 'ph:vinyl-record',
      // Копию микса отменять нечем: сам микс не тронут
      undo: readOnly.value || !res.set.changed
        ? undefined
        : async () => {
          try {
            await bridge.playlists.setOrder(targetId, res.previousItemIds)
            if (djOn) connect.transition = prevMode
            pui.reordered = { id: targetId, at: Date.now() }
          }
          catch {
            toast.error(t('pl.failed'))
          }
        },
    })
  }
  catch {
    toast.error(t('pl.failed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <UiModal v-model:open="pui.setOpen" :title="t('pl.set.title')" :description="pl?.title" tall :width="460">
    <!-- Первый трек: «Автоматически» и треки плейлиста по нынешнему порядку -->
    <div v-if="view === 'first'" class="ps ps--pick">
      <button type="button" class="ps__back" :aria-label="`${t('pl.set.back')}: ${t('pl.set.first')}`" @click="view = 'set'">
        <Icon name="ph:caret-left" class="ps__back-icon" />
        <span>{{ t('pl.set.first') }}</span>
      </button>
      <UiSearchField v-if="(detail?.items.length ?? 0) > 8" v-model="q" :placeholder="t('pl.set.search')" class="ps__search" />
      <button v-if="!q.trim()" type="button" class="ps__pick" :class="{ 'is-on': !firstId }" @click="chooseFirst(null)">
        <span class="ps__auto" aria-hidden="true"><Icon name="ph:sparkle" /></span>
        <span class="ps__name">{{ t('pl.set.auto') }}</span>
        <Icon v-if="!firstId" name="ph:check" class="ps__check" />
      </button>
      <button v-for="it in picks" :key="it.id" type="button" class="ps__pick" :class="{ 'is-on': firstId === it.id }" @click="chooseFirst(it)">
        <MusicCoverArt :image="it.track.album?.image" size="small" :alt="it.track.album?.title ?? ''" radius="6px" class="ps__art" />
        <span class="ps__text">
          <span class="ps__name ellipsis">{{ it.track.title }}</span>
          <span class="ps__sub ellipsis">{{ artistNames(it.track.artists) }}</span>
        </span>
        <Icon v-if="firstId === it.id" name="ph:check" class="ps__check" />
      </button>
    </div>

    <div v-else class="ps">
      <!-- Кривая -->
      <div class="ps__curves" role="radiogroup" :aria-label="t('pl.set.curve')">
        <button
          v-for="c in CURVES"
          :key="c.value"
          type="button"
          role="radio"
          class="ps__curve"
          :class="{ 'is-on': curve === c.value }"
          :aria-checked="curve === c.value"
          @click="curve = c.value"
        >
          <svg viewBox="0 0 28 18" class="ps__glyph" aria-hidden="true"><path :d="c.path" /></svg>
          <span>{{ t(`pl.set.${c.value}`) }}</span>
        </button>
      </div>
      <p class="ps__hint">{{ t(`pl.set.${curve}Desc`) }}</p>

      <!-- Первый трек -->
      <button type="button" class="ps__row" :disabled="!detail" @click="view = 'first'">
        <span class="ps__label">{{ t('pl.set.start') }}</span>
        <span class="ps__value ellipsis">{{ firstItem ? firstItem.track.title : t('pl.set.auto') }}</span>
        <Icon name="ph:caret-right" class="ps__caret" />
      </button>
      <div v-if="offerDj && !readOnly" class="ps__row ps__row--switch">
        <UiSwitch v-model="dj" :label="t('pl.set.dj')" :hint="t('pl.set.djDesc')" />
      </div>
      <div class="ps__hair" />

      <!-- Предпросмотр -->
      <p v-if="failed" class="ps__failed">{{ failed }}</p>
      <template v-else-if="ready">
        <div class="ps__energy" :class="{ 'is-loading': loading }">
          <svg v-if="line" :viewBox="`0 0 ${W} ${H}`" preserveAspectRatio="none" class="ps__line" role="img" :aria-label="t('pl.set.energy')">
            <path :d="line" vector-effect="non-scaling-stroke" />
          </svg>
        </div>
        <p class="ps__summary">{{ summary }}<template v-if="preview!.unplaced"><br>{{ t('pl.set.unplaced', { n: formatCount(preview!.unplaced, 'track', lang) }) }}</template></p>
        <ol class="ps__list" :class="{ 'is-loading': loading }">
          <li v-for="(r, i) in rows" :key="r.entry.itemId" class="ps__item" :class="{ 'is-unplaced': !r.entry.placed }">
            <span class="ps__idx tnum">{{ i + 1 }}</span>
            <MusicCoverArt :image="r.item.track.album?.image" size="small" :alt="r.item.track.album?.title ?? ''" radius="6px" class="ps__art" />
            <span class="ps__text">
              <span class="ps__name ellipsis">{{ r.item.track.title }}</span>
              <span class="ps__sub ellipsis">{{ artistNames(r.item.track.artists) }}</span>
            </span>
            <span v-if="r.entry.fit" class="ps__fit" :class="{ 'is-good': r.entry.fit === 'in_key' || r.entry.fit === 'smooth' }">{{ t(`pl.set.fit.${r.entry.fit}`) }}</span>
          </li>
        </ol>
      </template>
      <div v-else class="ps__skel" aria-hidden="true">
        <UiSkeleton height="56px" />
        <UiSkeleton variant="line" width="60%" />
        <div v-for="n in 6" :key="n" class="ps__skel-row">
          <UiSkeleton width="40px" height="40px" />
          <span class="ps__skel-text"><UiSkeleton variant="line" width="55%" /><UiSkeleton variant="line" width="35%" /></span>
        </div>
      </div>
    </div>

    <template v-if="view === 'set'" #footer="{ close }">
      <div class="ps__actions">
        <UiButton size="lg" block @click="close">{{ t('common.cancel') }}</UiButton>
        <UiButton size="lg" block variant="primary" :loading="busy" :disabled="!ready || !!failed || (!preview?.changed && !readOnly)" @click="apply">
          {{ readOnly ? t('pl.set.applyCopy') : t('pl.set.apply') }}
        </UiButton>
      </div>
    </template>
  </UiModal>
</template>

<style scoped>
.ps { display: flex; flex-direction: column; padding: 0 20px 16px; }
.ps--pick { padding: 0 12px 16px; }

/* Кривая: три варианта с маленьким графиком */
.ps__curves { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; }
.ps__curve { display: flex; flex-direction: column; align-items: center; gap: 6px; min-height: 72px; padding: 12px 6px 10px; border: 0; border-radius: var(--r-3); background: var(--fill-1); color: var(--text-2); font: inherit; font-size: var(--fs-small); font-weight: 550; cursor: pointer; transition: background-color var(--dur-1), color var(--dur-1), box-shadow var(--dur-1); -webkit-tap-highlight-color: transparent; }
.ps__curve:hover { background: var(--fill-2); color: var(--text-1); }
.ps__curve:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.ps__curve.is-on { background: var(--fill-2); color: var(--text-1); box-shadow: inset 0 0 0 1.5px var(--accent); }
.ps__glyph { width: 34px; height: 22px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; }
.ps__curve.is-on .ps__glyph { stroke: var(--accent); }
.ps__hint { margin: 10px 2px 6px; font-size: var(--fs-small); color: var(--text-3); }

/* Строки настроек: подпись слева — значение справа */
.ps__row { display: flex; align-items: center; gap: 12px; min-height: 48px; padding: 0 2px; border: 0; border-top: 1px solid var(--hair); background: none; color: var(--text-1); font: inherit; text-align: left; cursor: pointer; -webkit-tap-highlight-color: transparent; }
.ps__row:disabled { cursor: default; }
.ps__row:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); border-radius: var(--r-2); }
.ps__row--switch { cursor: default; }
.ps__row--switch > :deep(.sw) { flex: 1; }
.ps__label { flex: none; font-weight: 550; }
.ps__value { flex: 1; min-width: 0; color: var(--text-3); text-align: right; }
.ps__caret { flex: none; color: var(--text-4); }
.ps__row:hover .ps__value { color: var(--text-2); }
.ps__hair { height: 1px; margin-bottom: 16px; background: var(--hair); }

/* Предпросмотр */
.ps__energy { height: 56px; transition: opacity var(--dur-2); }
.ps__line { display: block; width: 100%; height: 100%; overflow: visible; }
.ps__line path { fill: none; stroke: var(--accent); stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
.ps__summary { margin: 10px 2px 8px; font-size: var(--fs-small); color: var(--text-3); text-wrap: pretty; }
.ps__failed { margin: 8px 2px; font-size: var(--fs-small); color: var(--text-3); }
.ps__list { margin: 0 -8px; padding: 0; list-style: none; transition: opacity var(--dur-2); }
.is-loading { opacity: 0.45; }
.ps__item { display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 6px 8px; border-radius: var(--r-2); }
.ps__item.is-unplaced { opacity: 0.55; }
.ps__idx { width: 22px; flex: none; font-size: var(--fs-small); color: var(--text-4); text-align: right; }
.ps__art { width: 40px; height: 40px; flex: none; }
.ps__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.ps__name { font-weight: 550; }
.ps__sub { font-size: var(--fs-small); color: var(--text-3); }
.ps__fit { flex: none; font-size: var(--fs-caption); color: var(--text-4); }
.ps__fit.is-good { color: var(--text-3); }

/* Выбор первого трека */
.ps__back { display: flex; align-items: center; gap: 6px; align-self: flex-start; min-height: 44px; margin: -4px 0 4px; padding: 0 12px 0 4px; border: 0; border-radius: var(--r-pill); background: none; color: var(--text-1); font: inherit; font-weight: 600; cursor: pointer; -webkit-tap-highlight-color: transparent; }
.ps__back:hover, .ps__back:focus-visible { background: var(--fill-1); outline: none; }
.ps__back-icon { font-size: 18px; color: var(--text-2); }
.ps__search { margin: 0 8px 8px; }
.ps__pick { display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 8px; border: 0; border-radius: var(--r-3); background: none; color: var(--text-1); font: inherit; text-align: left; cursor: pointer; transition: background-color var(--dur-1); -webkit-tap-highlight-color: transparent; }
.ps__pick:hover, .ps__pick:focus-visible { background: var(--fill-1); outline: none; }
.ps__pick:active { background: var(--fill-2); }
.ps__auto { display: grid; place-items: center; width: 40px; height: 40px; flex: none; border-radius: var(--r-2); background: var(--fill-2); color: var(--text-2); font-size: 18px; }
.ps__check { flex: none; margin-left: auto; color: var(--accent); font-size: 18px; }

.ps__actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.ps__skel { display: flex; flex-direction: column; gap: 12px; }
.ps__skel-row { display: flex; align-items: center; gap: 12px; }
.ps__skel-text { display: flex; flex: 1; flex-direction: column; gap: 6px; }

@media (max-width: 699px) {
  .ps { padding: 0 16px 12px; }
  .ps--pick { padding: 0 8px 12px; }
  .ps__row { min-height: 52px; }
  /* Лист: кнопки у нижнего края, под ними — безопасная зона */
  .ps__actions { padding: 12px 16px calc(var(--safe-bottom) + 12px); border-top: 1px solid var(--hair); }
}
</style>
