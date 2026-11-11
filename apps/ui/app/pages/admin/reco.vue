<script setup lang="ts">
import type { FacetView, RecoOverview, RecoSettingsPatch, RecoTask } from '~/api/types'

// Рекомендации: размер индекса каталога и доля проанализированного, находки разведчика, грани вкуса,
// последние показы с причинами, сервис анализа и настройки разведчика ползунками
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const { data, state, load } = useLoad<RecoOverview>(() => bridge.admin.reco())
const ix = computed(() => data.value?.index)
const n = (v: number) => formatNumber(v, lang.value)
const analysedShare = computed(() => {
  const i = ix.value
  if (!i) return null
  const all = i.full + i.readyWithoutAnalysis
  return all ? Math.round((i.full / all) * 100) : null
})

// ---------- Настройки ----------

const form = reactive({ tracksPerDay: 3000, maxIndex: 200000, maxHops: 2, hopDecay: 0.5 })
watch(() => data.value?.settings, (s) => {
  if (s) Object.assign(form, { tracksPerDay: s.tracksPerDay, maxIndex: s.maxIndex, maxHops: s.maxHops, hopDecay: s.hopDecay })
}, { immediate: true })
const saving = ref(false)
const save = useDebounced(async () => {
  const s = data.value?.settings
  if (!s) return
  const patch: RecoSettingsPatch = {}
  if (form.tracksPerDay !== s.tracksPerDay) patch.tracksPerDay = form.tracksPerDay
  if (form.maxIndex !== s.maxIndex) patch.maxIndex = form.maxIndex
  if (form.maxHops !== s.maxHops) patch.maxHops = form.maxHops
  if (Math.abs(form.hopDecay - s.hopDecay) > 1e-3) patch.hopDecay = Math.round(form.hopDecay * 100) / 100
  if (!Object.keys(patch).length) return
  saving.value = true
  try {
    const next = await bridge.admin.updateReco(patch)
    if (data.value) data.value = { ...data.value, settings: next }
    toast.show({ text: t('settings.saved'), icon: 'ph:check' })
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    saving.value = false
  }
}, 700)
watch(form, save)
const hops = computed({ get: () => String(form.maxHops), set: (v: string) => (form.maxHops = Number(v)) })
const decayText = computed(() => (form.hopDecay < 0.35 ? t('settings.careful') : form.hopDecay < 0.65 ? t('settings.balanced') : t('settings.bold')))

// ---------- Запуск вне расписания ----------

const running = ref<RecoTask | null>(null)
async function run(task: RecoTask) {
  running.value = task
  try {
    const r = await bridge.admin.runReco(task)
    toast.show({ text: r.queued ? t('run.queued', { n: r.queued }) : t('run.nothing'), icon: 'ph:play-circle' })
    await load()
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    running.value = null
  }
}

// ---------- Грани и показы ----------

const byUser = computed(() => {
  const groups = new Map<string, { name: string, facets: FacetView[] }>()
  for (const f of data.value?.facets ?? []) {
    const g = groups.get(f.userId) ?? { name: f.userName, facets: [] }
    g.facets.push(f)
    groups.set(f.userId, g)
  }
  return [...groups.values()]
})
const OUTCOME_TONE: Record<string, string> = { like: 'ap-ok', completed: 'ap-ok', more_like_this: 'ap-ok', skip: 'ap-warn', not_now: 'ap-warn', dislike: 'ap-bad' }
const outcome = (o: string | null) => (o ? t(`outcome.${o}`) : '—')
const ml = computed(() => data.value?.ml)
const mlVersion = computed(() => (ml.value?.health as { version?: string } | null)?.version ?? null)
const mlModels = computed(() => (ml.value?.health as { modelsLoaded?: boolean } | null)?.modelsLoaded)
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <span v-if="data?.modelVersion" class="rc__model">{{ t('model', { v: data.modelVersion }) }}</span>
    </header>

    <AdminState :loading="state === 'loading'" :error="state === 'error'" :rows="6" @retry="load">
      <template v-if="data && ix">
        <div class="ap-stats" style="--n: 5">
          <div class="ap-stat"><span class="ap-stat__value">{{ n(ix.size) }}</span><span class="ap-stat__label">{{ t('stats.index') }}</span><span class="ap-stat__extra">{{ t('stats.of', { n: n(ix.maxIndex) }) }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ analysedShare != null ? `${analysedShare}%` : '—' }}</span><span class="ap-stat__label">{{ t('stats.analysed') }}</span><span class="ap-stat__extra">{{ t('stats.files', { n: n(ix.full) }) }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ n(ix.analyzed24h) }}</span><span class="ap-stat__label">{{ t('stats.day') }}</span><span v-if="ix.failed24h" class="ap-stat__extra ap-bad">{{ t('stats.failed', { n: ix.failed24h }) }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ n(ix.queuedPreview + ix.queuedFull) }}</span><span class="ap-stat__label">{{ t('stats.queue') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ n(ix.artists) }}</span><span class="ap-stat__label">{{ t('stats.artists') }}</span><span class="ap-stat__extra">{{ t('stats.seeds', { n: ix.seeds }) }}</span></div>
        </div>

        <div class="ap-cols">
          <AdminSection :title="t('settings.title')" :hint="t('settings.hint')">
            <SettingsGroup>
              <SettingsRow :title="t('settings.perDay')" stack>
                <div class="rc__slider">
                  <UiSlider v-model="form.tracksPerDay" :label="t('settings.perDay')" :min="0" :max="10000" :step="250" :value-text="n(form.tracksPerDay)" />
                  <b class="tnum">{{ n(form.tracksPerDay) }}</b>
                </div>
              </SettingsRow>
              <SettingsRow :title="t('settings.maxIndex')" stack>
                <div class="rc__slider">
                  <UiSlider v-model="form.maxIndex" :label="t('settings.maxIndex')" :min="10000" :max="1000000" :step="10000" :value-text="n(form.maxIndex)" />
                  <b class="tnum">{{ n(form.maxIndex) }}</b>
                </div>
              </SettingsRow>
              <SettingsRow :title="t('settings.hops')" :description="t('settings.hopsHint')">
                <UiSegmentedControl v-model="hops" :options="['0', '1', '2', '3'].map(v => ({ value: v, label: v }))" :label="t('settings.hops')" size="sm" />
              </SettingsRow>
              <SettingsRow :title="t('settings.decay')" stack>
                <div class="rc__slider">
                  <UiSlider v-model="form.hopDecay" :label="t('settings.decay')" :min="0.05" :max="0.95" :step="0.05" :value-text="decayText" />
                  <b>{{ decayText }}</b>
                </div>
              </SettingsRow>
            </SettingsGroup>
          </AdminSection>

          <AdminSection :title="t('index.title')">
            <div class="ap-table rc__src">
              <div class="ap-tr ap-tr--head">
                <span>{{ t('index.source') }}</span>
                <span class="ap-num">{{ t('index.found') }}</span>
                <span class="ap-num">{{ t('index.analyzed') }}</span>
                <span class="ap-num">{{ t('index.pending') }}</span>
              </div>
              <div v-for="s in ix.bySource" :key="s.source" class="ap-tr">
                <span>{{ t(`index.kind.${s.source}`) }}</span>
                <span class="ap-num">{{ n(s.found) }}</span>
                <span class="ap-num">{{ n(s.analyzed) }}</span>
                <span class="ap-num ap-muted">{{ n(s.pending) }}</span>
              </div>
            </div>
            <div class="rc__run">
              <UiButton size="sm" icon="ph:compass" :loading="running === 'scout'" @click="run('scout')">{{ t('run.scout') }}</UiButton>
              <UiButton size="sm" icon="ph:waveform" :loading="running === 'analysis'" @click="run('analysis')">{{ t('run.analysis') }}</UiButton>
              <UiButton size="sm" icon="ph:sparkle" :loading="running === 'taste'" @click="run('taste')">{{ t('run.taste') }}</UiButton>
            </div>
          </AdminSection>
        </div>

        <AdminSection :title="t('facets.title')" :hint="t('facets.hint')">
          <p v-if="!byUser.length" class="ap-hint">{{ t('facets.empty') }}</p>
          <div v-for="g in byUser" :key="g.name" class="rc__user">
            <h3 v-if="byUser.length > 1" class="rc__uname">{{ g.name }}</h3>
            <div class="ap-table rc__facets">
              <div v-for="f in g.facets" :key="f.id" class="ap-tr">
                <span class="ap-cell"><b>{{ f.label }}</b><small>{{ f.examples.slice(0, 3).map(x => `${x.title} — ${artistNames(x.artists)}`).join(' · ') }}</small></span>
                <span class="rc__weight"><i :style="{ width: `${f.weight * 100}%` }" /></span>
                <span class="ap-num">{{ Math.round(f.weight * 100) }}%</span>
                <span class="ap-num ap-muted">{{ formatCount(f.tracks, 'track', lang) }}</span>
              </div>
            </div>
          </div>
        </AdminSection>

        <AdminSection :title="t('shown.title')" :hint="t('shown.hint')">
          <p v-if="!data.impressions.length" class="ap-hint">{{ t('shown.empty') }}</p>
          <div v-else class="ap-table rc__shown">
            <div v-for="i in data.impressions.slice(0, 30)" :key="i.id" class="ap-tr">
              <span class="ap-muted tnum ap-hide-phone">{{ formatDateTime(i.shownAt, lang) }}</span>
              <span class="ap-cell"><b>{{ i.track.title }}</b><small>{{ artistNames(i.track.artists) }}</small></span>
              <span class="ap-cell ap-hide-phone"><b class="rc__reason">{{ i.reason?.text ?? '—' }}</b><small>{{ t(`surface.${i.surface}`) }}</small></span>
              <span class="ap-num" :class="OUTCOME_TONE[i.outcome ?? '']">{{ outcome(i.outcome) }}</span>
            </div>
          </div>
        </AdminSection>

        <AdminSection :title="t('ml.title')">
          <dl v-if="ml" class="ap-kv rc__ml">
            <div><dt>{{ t('ml.state') }}</dt><dd :class="ml.reachable ? 'ap-ok' : 'ap-bad'">{{ ml.reachable ? t('ml.works') : t('ml.down') }}</dd></div>
            <div v-if="ml.error"><dt>{{ t('ml.error') }}</dt><dd class="ap-bad rc__err">{{ ml.error }}</dd></div>
            <div v-if="mlModels != null"><dt>{{ t('ml.models') }}</dt><dd>{{ mlModels ? t('ml.loaded') : t('ml.notLoaded') }}</dd></div>
            <div v-if="mlVersion"><dt>{{ t('ml.version') }}</dt><dd>{{ mlVersion }}</dd></div>
            <div><dt>{{ t('ml.address') }}</dt><dd class="ap-mono">{{ ml.url }}</dd></div>
          </dl>
        </AdminSection>
      </template>
    </AdminState>
  </div>
</template>

<style scoped>
.rc__model { font-size: var(--fs-caption); font-weight: 600; color: var(--text-2); }
.rc__slider { display: flex; align-items: center; gap: 16px; width: 280px; max-width: 100%; }
.rc__slider > :first-child { flex: 1; min-width: 0; }
.rc__slider b { min-width: 72px; font-size: var(--fs-small); font-weight: 600; text-align: right; }
.rc__src { --cols: minmax(0, 1fr) 80px 96px 80px; }
.rc__src .ap-tr { min-height: 44px; }
.rc__run { display: flex; flex-wrap: wrap; gap: 8px; padding-top: 8px; }
.rc__user { display: flex; flex-direction: column; gap: 4px; }
.rc__uname { margin: 8px 0 0; font-size: var(--fs-small); font-weight: 650; color: var(--text-2); }
.rc__facets { --cols: minmax(0, 2fr) minmax(80px, 1fr) 52px 84px; }
.rc__weight { display: block; height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-1); }
.rc__weight i { display: block; height: 100%; border-radius: inherit; background: var(--accent); }
.rc__shown { --cols: 112px minmax(0, 1.2fr) minmax(0, 1.4fr) 112px; }
.rc__reason { font-weight: 500 !important; }
.rc__err { max-width: 60%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
@media (max-width: 1099px) {
  .ap-stats { --n: 3 !important; row-gap: 16px; }
  .ap-stat:nth-child(4) { padding-left: 0; border-left: 0; }
}
@media (max-width: 699px) {
  .ap-stat:nth-child(4) { padding-left: 16px; border-left: 1px solid var(--hair); }
  .ap-stat:nth-child(odd) { padding-left: 0; border-left: 0; }
  .rc__slider { width: 100%; }
  .rc__facets { --cols: minmax(0, 1fr) 48px; }
  .rc__facets .ap-tr > :nth-child(2), .rc__facets .ap-tr > :nth-child(4) { display: none; }
  .rc__shown { --cols: minmax(0, 1fr) auto; }
  .rc__src { --cols: minmax(0, 1fr) 64px 72px 64px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Рекомендации",
    "model": "Модель {v}",
    "failed": "Не получилось — попробуйте ещё раз",
    "stats": {
      "index": "в индексе каталога",
      "of": "из {n}",
      "analysed": "скачанного разобрано",
      "files": "файлов: {n}",
      "day": "разобрано за сутки",
      "failed": "{n} с ошибкой",
      "queue": "ждут анализа",
      "artists": "артистов вокруг вкуса",
      "seeds": "из них любимых: {n}"
    },
    "settings": {
      "title": "Разведка",
      "hint": "Применяется со следующего прохода разведчика",
      "perDay": "Треков на анализ в сутки",
      "maxIndex": "Предел индекса",
      "hops": "Смелость разведки",
      "hopsHint": "Шагов по похожим артистам",
      "decay": "Доля нового",
      "careful": "Осторожно",
      "balanced": "Поровну",
      "bold": "Смело",
      "saved": "Настройки сохранены"
    },
    "index": {
      "title": "Находки разведчика",
      "source": "Откуда",
      "found": "Найдено",
      "analyzed": "Разобрано",
      "pending": "Ждут",
      "kind": { "top": "Популярное артистов", "release": "Свежие релизы", "radio": "Радио артистов", "chart": "Чарты", "editorial": "Новинки редакции" }
    },
    "run": {
      "scout": "Разведать сейчас",
      "analysis": "Разобрать скачанное",
      "taste": "Пересчитать вкус",
      "queued": "Поставлено задач: {n}",
      "nothing": "Всё уже в очереди"
    },
    "facets": { "title": "Грани вкуса", "hint": "Из лайков и дослушанного; пересчитываются ночью", "empty": "Граней пока нет: нужно больше лайков и прослушиваний" },
    "shown": { "title": "Последние показы", "hint": "Что и почему подсказала волна и чем это кончилось", "empty": "Показов пока не было" },
    "surface": { "wave": "волна", "radio": "радио", "similar": "похожие", "mix": "микс", "discovery": "открытия" },
    "outcome": { "like": "Лайк", "dislike": "Не нравится", "skip": "Пропуск", "not_now": "Не сейчас", "more_like_this": "Ещё такого", "completed": "Дослушан" },
    "ml": {
      "title": "Сервис анализа звука",
      "state": "Состояние",
      "works": "работает",
      "down": "не отвечает",
      "error": "Ошибка",
      "models": "Модели",
      "loaded": "загружены",
      "notLoaded": "не загружены",
      "version": "Версия",
      "address": "Адрес"
    }
  },
  "en": {
    "title": "Recommendations",
    "model": "Model {v}",
    "failed": "Didn't work — try again",
    "stats": {
      "index": "in the catalog index",
      "of": "of {n}",
      "analysed": "of downloads analysed",
      "files": "files: {n}",
      "day": "analysed in 24 h",
      "failed": "{n} failed",
      "queue": "waiting for analysis",
      "artists": "artists around your taste",
      "seeds": "favourites: {n}"
    },
    "settings": {
      "title": "Scouting",
      "hint": "Applies from the next scout pass",
      "perDay": "Tracks to analyse per day",
      "maxIndex": "Index limit",
      "hops": "Scouting boldness",
      "hopsHint": "Steps through similar artists",
      "decay": "Share of new",
      "careful": "Careful",
      "balanced": "Balanced",
      "bold": "Bold",
      "saved": "Settings saved"
    },
    "index": {
      "title": "Scout finds",
      "source": "From",
      "found": "Found",
      "analyzed": "Analysed",
      "pending": "Waiting",
      "kind": { "top": "Artists' top tracks", "release": "Fresh releases", "radio": "Artist radio", "chart": "Charts", "editorial": "Editorial picks" }
    },
    "run": {
      "scout": "Scout now",
      "analysis": "Analyse downloads",
      "taste": "Rebuild taste",
      "queued": "Jobs queued: {n}",
      "nothing": "Everything is already queued"
    },
    "facets": { "title": "Taste facets", "hint": "From likes and full listens; rebuilt at night", "empty": "No facets yet: more likes and plays needed" },
    "shown": { "title": "Recent impressions", "hint": "What the wave suggested, why, and how it ended", "empty": "Nothing shown yet" },
    "surface": { "wave": "wave", "radio": "radio", "similar": "similar", "mix": "mix", "discovery": "discovery" },
    "outcome": { "like": "Like", "dislike": "Dislike", "skip": "Skip", "not_now": "Not now", "more_like_this": "More like this", "completed": "Finished" },
    "ml": {
      "title": "Audio analysis service",
      "state": "State",
      "works": "working",
      "down": "not responding",
      "error": "Error",
      "models": "Models",
      "loaded": "loaded",
      "notLoaded": "not loaded",
      "version": "Version",
      "address": "Address"
    }
  }
}
</i18n>
