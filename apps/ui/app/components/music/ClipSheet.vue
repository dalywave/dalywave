<script setup lang="ts">
import type { ClipCandidate, ClipChoice, ClipView } from '~/api/types'

// «Клип» (администратор): какой клип играет вместо обложки, другие варианты с YouTube, своя ссылка,
// «без клипа». Выбор закрепляется — автопоиск его не трогает; пока выбранный режется, у трека
// обложка. На телефоне — лист снизу, на компьютере — окно
const bridge = useBridge()
const toast = useToast()
const motion = useMotionStore()
const { track, open } = useClipSheet()
const { t } = useI18n({ useScope: 'local' })

const view = ref<ClipView | null>(null)
const failed = ref(false)
const busy = ref<string | null>(null)
const link = ref('')
let timer: ReturnType<typeof setTimeout> | undefined

const heading = computed(() => (track.value ? `${track.value.title} · ${track.value.artists.map(a => a.name).join(', ')}` : ''))

async function load() {
  const tr = track.value
  if (!tr) return
  clearTimeout(timer)
  try {
    const v = await bridge.video.clip(tr.id)
    if (track.value?.id !== tr.id) return
    const was = view.value?.state
    view.value = v
    failed.value = false
    // Выбранный клип нарезан — плеер берёт новую ссылку
    if (was === 'pending' && v.state !== 'pending') motion.refresh(tr.id)
  }
  catch {
    failed.value = !view.value
  }
  // Режется или ищутся варианты — смотрим ещё раз
  if (open.value && (view.value?.state === 'pending' || view.value?.searching)) timer = setTimeout(() => void load(), 4000)
}

watch(open, (v) => {
  clearTimeout(timer)
  if (!v) return
  view.value = null
  failed.value = false
  link.value = ''
  void load()
})
onBeforeUnmount(() => clearTimeout(timer))

async function choose(choice: ClipChoice, key: string, done?: string) {
  const tr = track.value
  if (!tr || busy.value) return
  busy.value = key
  try {
    view.value = await bridge.video.chooseClip(tr.id, choice)
    motion.refresh(tr.id)
    if (done) toast.show({ text: done, icon: 'ph:check-circle' })
    link.value = ''
    void load()
  }
  catch (e) {
    toast.error(e instanceof Error && e.message ? e.message : t('failed'))
  }
  finally {
    busy.value = null
  }
}

const pick = (c: ClipCandidate) => choose({ action: 'video', videoId: c.videoId }, c.videoId, t('chosen'))
const submitLink = () => link.value.trim() && choose({ action: 'url', url: link.value.trim() }, 'url', t('chosen'))

const current = computed(() => view.value?.videoId ?? null)
const candidates = computed(() => (view.value?.candidates ?? []).filter(c => c.videoId !== current.value || view.value?.state !== 'ready'))
const own = (source: string | null | undefined) => source === 'artist' || source === 'vevo'
const thumb = (id: string) => `https://i.ytimg.com/vi/${id}/mqdefault.jpg`

/** Чей канал — короткой отметкой; отвергнутый — первой причиной */
function tag(c: ClipCandidate): string | null {
  if (c.rejected?.length) {
    const r = c.rejected.find(x => !x.startsWith('excluded')) ?? c.rejected[0]!
    const key = r.split(':')[0]!
    return t(`reason.${['fan', 'version', 'not_official', 'duration', 'title', 'artist', 'topic_channel'].includes(key) ? key : 'other'}`)
  }
  if (own(c.source)) return t('source.artist')
  if (c.source) return t('source.official')
  return null
}

const status = computed(() => {
  const v = view.value
  if (!v) return null
  if (v.state === 'pending') return t('preparing')
  if (v.state === 'none') return v.pinned ? t('off') : t('none')
  return null
})
</script>

<template>
  <UiModal v-model:open="open" :title="t('title')" :description="heading" :width="520" tall>
    <div class="cs">
      <p class="cs__what ellipsis">{{ heading }}</p>

      <div v-if="!view && !failed" class="cs__now cs__now--empty" aria-busy="true" />
      <p v-else-if="failed" class="cs__note">{{ t('loadFailed') }}</p>
      <template v-else-if="view">
        <!-- Сейчас: клип, «Клип готовится» или обложка -->
        <div class="cs__now">
          <MusicLoopVideo v-if="view.state === 'ready' && view.url" :src="view.url" :active="open" class="cs__video" />
          <!-- Без клипа — обложка, подпись под ней -->
          <div v-else class="cs__cover">
            <MusicCoverArt :image="track?.album?.image" size="medium" :alt="track?.album?.title ?? ''" />
            <span v-if="status" class="cs__status">
              <Icon v-if="view.state === 'pending'" name="ph:circle-notch" class="cs__spin" />
              {{ status }}
            </span>
          </div>
        </div>
        <div v-if="view.state === 'ready'" class="cs__meta">
          <div class="cs__meta-text">
            <span class="cs__meta-title ellipsis">{{ view.videoTitle ?? view.videoId }}</span>
            <span class="cs__meta-sub ellipsis">{{ view.channel }}<template v-if="view.pinned"> · {{ t('pinned') }}</template><template v-else-if="own(view.source)"> · {{ t('source.artist') }}</template></span>
          </div>
        </div>

        <!-- Жалобы: скрыли у себя -->
        <div v-if="view.reports > 0" class="cs__reports">
          <span>{{ t('reports', { n: view.reports }) }}</span>
          <UiButton size="sm" variant="ghost" :loading="busy === 'keep'" @click="choose({ action: 'keep' }, 'keep', t('kept'))">{{ t('keep') }}</UiButton>
        </div>

        <!-- Другие варианты -->
        <section class="cs__list" :aria-label="t('others')">
          <header class="cs__list-head">
            <h3>{{ t('others') }}</h3>
            <UiButton size="sm" variant="ghost" icon="ph:arrows-clockwise" :loading="busy === 'search' || (view.searching && !candidates.length)" @click="choose({ action: 'search' }, 'search')">{{ t('refresh') }}</UiButton>
          </header>
          <p v-if="!candidates.length" class="cs__note">{{ view.searching ? t('searching') : t('noCandidates') }}</p>
          <button
            v-for="c in candidates"
            :key="c.videoId"
            type="button"
            class="cs__row"
            :class="{ 'is-rejected': !!c.rejected?.length }"
            :disabled="!!busy"
            @click="pick(c)"
          >
            <span class="cs__thumb"><img :src="thumb(c.videoId)" alt="" loading="lazy"></span>
            <span class="cs__row-text">
              <span class="cs__row-title">{{ c.title }}</span>
              <span class="cs__row-sub ellipsis">{{ c.channel }}<template v-if="c.durationS"> · {{ formatDuration(c.durationS * 1000) }}</template></span>
              <!-- Телефон: отметка — строкой под каналом, чтобы название не сжималось -->
              <span v-if="tag(c)" class="cs__tag cs__tag--inline" :class="{ 'is-own': own(c.source) && !c.rejected?.length }">{{ tag(c) }}</span>
            </span>
            <span v-if="tag(c)" class="cs__tag cs__tag--side" :class="{ 'is-own': own(c.source) && !c.rejected?.length }">{{ tag(c) }}</span>
            <Icon v-if="busy === c.videoId" name="ph:circle-notch" class="cs__spin cs__row-busy" />
          </button>
        </section>

        <!-- Своя ссылка -->
        <form class="cs__link" @submit.prevent="submitLink">
          <input v-model="link" class="cs__input" type="url" inputmode="url" :placeholder="t('linkPlaceholder')" :aria-label="t('linkLabel')">
          <UiButton type="submit" :disabled="!link.trim()" :loading="busy === 'url'">{{ t('use') }}</UiButton>
        </form>

        <div class="cs__actions">
          <UiButton v-if="view.pinned" variant="ghost" icon="ph:sparkle" :loading="busy === 'auto'" @click="choose({ action: 'auto' }, 'auto', t('auto'))">{{ t('autoLabel') }}</UiButton>
          <UiButton v-if="!(view.pinned && view.state === 'none')" variant="ghost" icon="ph:image" :loading="busy === 'none'" @click="choose({ action: 'none' }, 'none', t('offDone'))">{{ t('offLabel') }}</UiButton>
        </div>
      </template>
    </div>
  </UiModal>
</template>

<style scoped>
.cs { display: flex; flex-direction: column; gap: 16px; padding: 0 20px 16px; }
.cs__what { margin: -4px 0 0; font-size: var(--fs-small); color: var(--text-2); }
.cs__now { position: relative; overflow: hidden; aspect-ratio: 16 / 9; border-radius: var(--r-3); background: var(--fill-1); }
.cs__now--empty { animation: cs-pulse 1.4s ease-in-out infinite; }
@keyframes cs-pulse { 50% { opacity: 0.55; } }
.cs__video { position: absolute; inset: 0; }
.cs__video :deep(video) { width: 100%; height: 100%; object-fit: cover; }
.cs__cover { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; }
.cs__cover :deep(.art) { width: auto; height: 58%; aspect-ratio: 1; border-radius: var(--r-2); box-shadow: var(--shadow-1); }
.cs__status { display: inline-flex; align-items: center; gap: 8px; font-size: var(--fs-small); font-weight: 600; color: var(--text-2); }
.cs__spin { animation: cs-spin 0.9s linear infinite; }
@keyframes cs-spin { to { transform: rotate(360deg); } }
.cs__meta { display: flex; align-items: center; gap: 12px; margin-top: -6px; }
.cs__meta-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.cs__meta-title { font-weight: 600; }
.cs__meta-sub { font-size: var(--fs-small); color: var(--text-2); }
.cs__reports { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 44px; padding: 0 4px 0 12px; border-radius: var(--r-3); background: var(--fill-1); font-size: var(--fs-small); color: var(--text-2); }
.cs__list { display: flex; flex-direction: column; }
.cs__list-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
.cs__list-head h3 { margin: 0; font-size: var(--fs-body); font-weight: 650; }
.cs__note { margin: 0; padding: 8px 0; font-size: var(--fs-small); color: var(--text-3); }
.cs__row {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 64px;
  padding: 8px 4px;
  border: 0;
  border-bottom: 1px solid var(--hair);
  background: none;
  color: var(--text-1);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.cs__row:last-child { border-bottom: 0; }
.cs__row:hover { background: var(--fill-1); }
.cs__row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; border-radius: var(--r-2); }
.cs__row:disabled { cursor: default; }
.cs__row.is-rejected .cs__thumb, .cs__row.is-rejected .cs__row-text { opacity: 0.55; }
.cs__thumb { position: relative; flex: none; width: 96px; aspect-ratio: 16 / 9; overflow: hidden; border-radius: var(--r-1); background: var(--fill-2); }
.cs__thumb img { width: 100%; height: 100%; object-fit: cover; }
.cs__row-text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.cs__row-title { display: -webkit-box; overflow: hidden; font-size: var(--fs-small); font-weight: 550; line-height: 1.3; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; }
.cs__row-sub { font-size: var(--fs-caption); color: var(--text-3); }
.cs__tag { flex: none; font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); white-space: nowrap; }
.cs__tag.is-own { color: var(--accent); }
.cs__tag--inline { display: none; }
.cs__row-busy { flex: none; color: var(--text-2); }
.cs__link { display: flex; gap: 8px; }
.cs__input {
  flex: 1;
  min-width: 0;
  height: 40px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--r-3);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  color: var(--text-1);
  font: inherit;
  font-size: var(--fs-body);
  outline: none;
}
.cs__input::placeholder { color: var(--text-3); }
.cs__input:focus { box-shadow: inset 0 0 0 1px var(--focus); }
.cs__actions { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; padding-top: 4px; border-top: 1px solid var(--hair); }
.cs__actions :deep(.btn) { padding-left: 8px; padding-right: 8px; }
@media (max-width: 699px) {
  .cs { padding: 0 20px 12px; }
  .cs__tag--inline { display: block; }
  .cs__tag--side { display: none; }
  .cs__row { min-height: 72px; padding: 10px 0; }
  .cs__thumb { width: 104px; }
  .cs__input { height: 44px; font-size: 16px; }
  .cs__link :deep(.btn) { --h: 44px; }
  .cs__actions :deep(.btn) { --h: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Клип",
    "loadFailed": "Не получилось загрузить — закройте и откройте снова",
    "preparing": "Клип готовится",
    "none": "Клипа нет — показывается обложка",
    "off": "Без клипа",
    "pinned": "выбран вручную",
    "reports": "Скрыли у себя: {n}",
    "keep": "Оставить",
    "kept": "Клип оставлен",
    "others": "Другие варианты",
    "refresh": "Обновить",
    "searching": "Ищем варианты…",
    "noCandidates": "Вариантов пока нет",
    "source": { "artist": "канал артиста", "official": "официальный" },
    "reason": {
      "fan": "фанатское",
      "version": "не клип",
      "not_official": "не официальный",
      "duration": "другая длина",
      "title": "другая песня",
      "artist": "другой артист",
      "topic_channel": "только звук",
      "other": "не подходит"
    },
    "linkLabel": "Ссылка на YouTube",
    "linkPlaceholder": "Вставить ссылку на YouTube",
    "use": "Выбрать",
    "chosen": "Клип выбран — готовится",
    "autoLabel": "Подобрать заново",
    "auto": "Клип подберётся заново",
    "offLabel": "Без клипа",
    "offDone": "Клип убран — будет обложка",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "en": {
    "title": "Video",
    "loadFailed": "Couldn’t load — close and open again",
    "preparing": "Video is being prepared",
    "none": "No video — the cover is shown",
    "off": "No video",
    "pinned": "chosen by hand",
    "reports": "Hidden by listeners: {n}",
    "keep": "Keep",
    "kept": "Video kept",
    "others": "Other options",
    "refresh": "Refresh",
    "searching": "Looking for options…",
    "noCandidates": "No options yet",
    "source": { "artist": "artist channel", "official": "official" },
    "reason": {
      "fan": "fan-made",
      "version": "not a video",
      "not_official": "not official",
      "duration": "different length",
      "title": "different song",
      "artist": "different artist",
      "topic_channel": "audio only",
      "other": "doesn’t fit"
    },
    "linkLabel": "YouTube link",
    "linkPlaceholder": "Paste a YouTube link",
    "use": "Use",
    "chosen": "Video chosen — preparing",
    "autoLabel": "Pick again",
    "auto": "The video will be picked again",
    "offLabel": "No video",
    "offDone": "Video removed — the cover will show",
    "failed": "Didn’t work — try again"
  }
}
</i18n>
