<script setup lang="ts">
import type { FacetView, RecoListener, RecoOverview, RecoSettingsPatch, RecoTask } from '~/api/types'

// Рекомендации. Вкус у каждого слушателя свой — выбор слушателя сверху (по умолчанию — свой): грани,
// последние показы с исходами, сессии волны, заготовки, миксы. Индекс каталога общий для всех — отдельным
// разделом: размер и анализ, находки разведчика, его настройки, сеть вкуса (версии с метриками, «Обучить»),
// сервис анализа
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const route = useRoute()
const router = useRouter()
const toast = useToast()
const lang = useLang()
const { t } = useScopedI18n('pages.admin.reco')
useHead({ title: () => t('title') })

const userId = ref<string>(typeof route.query.user === 'string' ? route.query.user : '')
const { data, state, load } = useLoad<RecoOverview>(() => bridge.admin.reco(userId.value || null), () => userId.value)
// Список слушателей не пропадает, пока грузится выбранный
const listeners = ref<RecoListener[]>([])
watch(data, (d) => {
  if (d) listeners.value = d.listeners
})
const viewerId = computed(() => listeners.value[0]?.id ?? null)
const selected = computed({
  get: () => userId.value || data.value?.user.id || viewerId.value || '',
  set: (v: string) => {
    userId.value = v === viewerId.value ? '' : v
    void router.replace({ query: { ...route.query, user: userId.value || undefined } })
  },
})
const user = computed(() => data.value?.user)
// В списке слушателей — сразу видно, у кого вкус уже есть: лайки и прослушивания
const listenerOptions = computed(() => listeners.value.map(l => ({
  value: l.id,
  label: l.id === viewerId.value ? t('me', { name: l.name }) : l.name,
  hint: t('whoHint', { l: formatNumber(l.likes, lang.value), p: formatNumber(l.plays, lang.value) }),
})))

const ix = computed(() => data.value?.index)
const n = (v: number) => formatNumber(v, lang.value)
const analysedShare = computed(() => {
  const i = ix.value
  if (!i) return null
  const all = i.full + i.readyWithoutAnalysis
  return all ? Math.round((i.full / all) * 100) : null
})
const poolTotals = computed(() => {
  const pools = data.value?.pools ?? []
  return { ready: pools.reduce((s, p) => s + p.ready, 0), all: pools.reduce((s, p) => s + p.tracks, 0) }
})

// ---------- Настройки разведчика ----------

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
    // Вкус — только выбранного слушателя; разведка и анализ — общие
    const r = await bridge.admin.runReco(task, task === 'taste' ? user.value?.id : null)
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

// ---------- Вкус слушателя ----------

const facets = computed<FacetView[]>(() => data.value?.facets ?? [])
const OUTCOME_TONE: Record<string, string> = { like: 'ap-ok', completed: 'ap-ok', more_like_this: 'ap-ok', skip: 'ap-warn', not_now: 'ap-warn', dislike: 'ap-bad' }
const outcome = (o: string | null) => (o ? t(`outcome.${o}`) : '—')
// Настройки из API — с необязательными полями: не переданное — по умолчанию
const summary = (s: RecoOverview['sessions'][number]['settings']) => waveSummary({
  activity: s.activity ?? 'any',
  character: s.character ?? 'mix',
  mood: s.mood ?? 'any',
  language: s.language ?? 'any',
}, lang.value)
// ---------- Сеть вкуса ----------

const models = computed(() => data.value?.models ?? [])
const training = computed(() => data.value?.training)
const metric = (v: number | null | undefined) => (v == null ? '—' : v.toFixed(3).replace('.', lang.value === 'en' ? '.' : ','))
const dataShare = computed(() => {
  const tr = training.value
  return tr && tr.minRows ? Math.min(1, tr.rows / tr.minRows) : 0
})
function modelState(m: RecoOverview['models'][number]) {
  if (m.active) return { text: t('nn.state.active'), tone: 'ap-ok' }
  if (!m.learned) return { text: t('nn.state.rulesOff'), tone: 'ap-muted' }
  if (m.beatsRules && m.beatsCurrent === false) return { text: t('nn.state.notBetter'), tone: 'ap-muted' }
  return m.beatsRules ? { text: t('nn.state.replaced'), tone: 'ap-muted' } : { text: t('nn.state.worse'), tone: 'ap-warn' }
}
/** Где и для кого обучена: «Сервер · общая», «Mac Zakhar · своя: Аня» */
function modelOrigin(m: RecoOverview['models'][number]) {
  const where = m.origin === 'device' ? (m.deviceName ?? t('nn.device')) : t('nn.server')
  const whom = m.userName ? t('nn.personal', { name: m.userName }) : t('nn.global')
  return `${where} · ${whom}`
}
const tasks = computed(() => training.value?.deviceTasks ?? [])
const TASK_TONE: Record<string, string> = { done: 'ap-ok', failed: 'ap-bad', expired: 'ap-muted', leased: '' }
const ml = computed(() => data.value?.ml)
const mlVersion = computed(() => (ml.value?.health as { version?: string } | null)?.version ?? null)
const mlModels = computed(() => (ml.value?.health as { modelsLoaded?: boolean } | null)?.modelsLoaded)
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminState :loading="state === 'loading'" :error="state === 'error' || state === 'missing'" :rows="6" @retry="load">
      <template v-if="data && ix && user">
        <!-- Вкус выбранного слушателя -->
        <header class="rc__group">
          <span class="rc__eyebrow">{{ t('user.eyebrow') }}</span>
          <h2 class="rc__group-title">
            <AdminSelect v-if="listeners.length > 1" v-model="selected" :options="listenerOptions" :label="t('who')" variant="title" />
            <template v-else>{{ user.name }}</template>
          </h2>
          <p class="ap-hint">{{ t('user.hint') }}</p>
        </header>

        <div class="ap-stats" style="--n: 4">
          <div class="ap-stat"><span class="ap-stat__value">{{ n(user.likes) }}</span><span class="ap-stat__label">{{ t('ustats.likes') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ n(user.plays) }}</span><span class="ap-stat__label">{{ t('ustats.plays') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ n(user.facets) }}</span><span class="ap-stat__label">{{ t('ustats.facets') }}</span></div>
          <div class="ap-stat">
            <span class="ap-stat__value">{{ n(poolTotals.ready) }}</span>
            <span class="ap-stat__label">{{ t('ustats.pools') }}</span>
            <span class="ap-stat__extra">{{ t('stats.of', { n: n(poolTotals.all) }) }}</span>
          </div>
        </div>

        <AdminSection :title="t('facets.title')" :hint="t('facets.hint')">
          <template #tools>
            <UiButton size="sm" icon="ph:sparkle" :loading="running === 'taste'" @click="run('taste')">{{ t('run.taste') }}</UiButton>
          </template>
          <p v-if="!facets.length" class="ap-hint">{{ t('facets.empty') }}</p>
          <div v-else class="ap-table rc__facets">
            <div v-for="f in facets" :key="f.id" class="ap-tr">
              <span class="ap-cell"><b>{{ f.label }}</b><small>{{ f.examples.slice(0, 3).map(x => `${x.title} — ${artistNames(x.artists)}`).join(' · ') }}</small></span>
              <span class="rc__weight"><i :style="{ width: `${f.weight * 100}%` }" /></span>
              <span class="ap-num">{{ Math.round(f.weight * 100) }}%</span>
              <span class="ap-num ap-muted">{{ formatCount(f.tracks, 'track', lang) }}</span>
            </div>
          </div>
        </AdminSection>

        <AdminSection :title="t('shown.title')" :hint="t('shown.hint')">
          <p v-if="!data.impressions.length" class="ap-hint">{{ t('shown.empty') }}</p>
          <div v-else class="ap-table rc__shown">
            <div v-for="i in data.impressions.slice(0, 15)" :key="i.id" class="ap-tr">
              <span class="ap-muted tnum ap-hide-phone">{{ formatDateTime(i.shownAt, lang) }}</span>
              <span class="ap-cell"><b>{{ i.track.title }}</b><small>{{ artistNames(i.track.artists) }}</small></span>
              <span class="ap-cell ap-hide-phone"><b class="rc__reason">{{ i.reason?.text ?? '—' }}</b><small>{{ t(`surface.${i.surface}`) }}</small></span>
              <span class="ap-num" :class="OUTCOME_TONE[i.outcome ?? '']">{{ outcome(i.outcome) }}</span>
            </div>
          </div>
        </AdminSection>

        <div class="ap-cols">
          <AdminSection :title="t('sessions.title')" :hint="t('sessions.hint')">
            <p v-if="!data.sessions.length" class="ap-hint">{{ t('sessions.empty') }}</p>
            <div v-else class="ap-table rc__sessions">
              <div v-for="s in data.sessions" :key="s.id" class="ap-tr">
                <span class="ap-cell"><b>{{ s.title }}</b><small>{{ summary(s.settings) }}</small></span>
                <span class="ap-cell rc__end"><b class="tnum">{{ formatCount(s.served, 'track', lang) }}</b><small class="tnum ap-hide-phone">{{ t('sessions.outcomes', { l: s.likes, c: s.completed, s: s.skips }) }}</small></span>
                <span class="ap-num ap-muted ap-hide-phone">{{ formatRelative(s.lastActiveAt, lang) }}</span>
              </div>
            </div>
          </AdminSection>

          <AdminSection :title="t('pools.title')" :hint="t('pools.hint')">
            <p v-if="!data.pools.length" class="ap-hint">{{ t('pools.empty') }}</p>
            <div v-else class="ap-table rc__pools">
              <div v-for="p in data.pools" :key="p.combo" class="ap-tr">
                <span class="ap-cell"><b>{{ summary(p.settings) }}</b><small class="ap-mono">{{ p.combo }}</small></span>
                <span class="rc__weight" :title="t('pools.ready', { r: p.ready, n: p.tracks })"><i :style="{ width: `${p.tracks ? (p.ready / p.tracks) * 100 : 0}%` }" /></span>
                <span class="ap-num tnum">{{ p.ready }}/{{ p.tracks }}</span>
              </div>
            </div>
          </AdminSection>
        </div>

        <AdminSection :title="t('mixes.title')" :hint="t('mixes.hint')">
          <p v-if="!data.mixes.length" class="ap-hint">{{ t('mixes.empty') }}</p>
          <div v-else class="ap-table rc__mixes">
            <div v-for="m in data.mixes" :key="m.id" class="ap-tr">
              <span class="ap-cell"><b>{{ m.title }}</b><small>{{ t(`mixes.kind.${m.generator}`) }}</small></span>
              <span class="ap-num tnum">{{ formatCount(m.tracks, 'track', lang) }}</span>
              <span class="ap-num ap-muted ap-hide-phone">{{ formatRelative(m.updatedAt, lang) }}</span>
            </div>
          </div>
        </AdminSection>

        <!-- Общий индекс каталога: один на всех слушателей -->
        <header class="rc__group rc__group--shared">
          <span class="rc__eyebrow">{{ t('shared.eyebrow') }}</span>
          <h2 class="rc__group-title">{{ t('shared.title') }}</h2>
          <p class="ap-hint">{{ t('shared.hint') }}</p>
        </header>

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
            </div>
          </AdminSection>
        </div>

        <AdminSection :title="t('nn.title')" :hint="t('nn.hint')">
          <template #tools>
            <UiButton size="sm" icon="ph:brain" :loading="running === 'model' || training?.running" :disabled="training?.running" @click="run('model')">{{ training?.running ? t('nn.training') : t('run.model') }}</UiButton>
          </template>
          <div v-if="training" class="rc__nn-status">
            <div class="rc__nn-line">
              <span class="rc__nn-label">{{ t('nn.data') }}</span>
              <span class="rc__weight rc__nn-meter"><i :style="{ width: `${dataShare * 100}%` }" /></span>
              <span class="tnum rc__nn-count">{{ training.rows >= training.minRows ? t('nn.enough', { n: n(training.rows) }) : t('nn.of', { n: n(training.rows), m: n(training.minRows) }) }}</span>
            </div>
            <p v-if="training.lastRunAt" class="ap-hint">
              <span :class="training.outcome === 'failed' ? 'ap-bad' : ''">{{ training.message ?? t(`nn.outcome.${training.outcome ?? 'none'}`) }}</span>
              · {{ formatRelative(training.lastRunAt, lang) }}
            </p>
            <p v-else class="ap-hint">{{ t('nn.never') }}</p>
          </div>
          <div class="ap-table rc__nn">
            <div class="ap-tr ap-tr--head">
              <span>{{ t('nn.version') }}</span>
              <span class="ap-num">{{ t('nn.auc') }}</span>
              <span class="ap-num ap-hide-phone">{{ t('nn.session') }}</span>
              <span class="ap-num ap-hide-phone">{{ t('nn.logloss') }}</span>
              <span class="ap-num">{{ t('nn.state.title') }}</span>
            </div>
            <div v-for="m in models" :key="m.id" class="ap-tr">
              <span class="ap-cell">
                <b class="ap-mono">{{ m.version }}</b>
                <small v-if="m.learned">{{ modelOrigin(m) }} · {{ formatRelative(m.createdAt, lang) }}</small>
                <small v-if="m.learned && m.note" class="rc__note">{{ m.note }}</small>
                <small v-if="!m.learned">{{ t('nn.rules') }}</small>
              </span>
              <span class="ap-cell rc__end">
                <b class="tnum">{{ metric(m.auc) }}</b>
                <small v-if="m.learned" class="tnum">{{ t('nn.vs', { v: metric(m.baselineAuc) }) }}</small>
                <small v-if="m.currentVersion" class="tnum">{{ t('nn.vsCurrent', { v: metric(m.currentAuc) }) }}</small>
              </span>
              <span class="ap-cell rc__end ap-hide-phone">
                <b class="tnum">{{ metric(m.sessionAuc) }}</b>
                <small v-if="m.learned && m.sessionAuc != null" class="tnum">{{ t('nn.vs', { v: metric(m.baselineSessionAuc) }) }}</small>
              </span>
              <span class="ap-cell rc__end ap-hide-phone">
                <b class="tnum">{{ metric(m.logloss) }}</b>
                <small v-if="m.learned" class="tnum">{{ t('nn.vs', { v: metric(m.baselineLogloss) }) }}</small>
              </span>
              <span class="ap-cell rc__end">
                <b :class="modelState(m).tone">{{ modelState(m).text }}</b>
                <small v-if="m.pBetter != null" class="tnum">{{ t('nn.confidence', { p: Math.round(m.pBetter * 100) }) }}</small>
              </span>
            </div>
          </div>
          <template v-if="tasks.length">
            <h3 class="rc__sub">{{ t('nn.devices') }}</h3>
            <p class="ap-hint">{{ t('nn.devicesHint') }}</p>
            <div class="ap-table rc__tasks">
              <div v-for="task in tasks" :key="task.id" class="ap-tr">
                <span class="ap-cell"><b>{{ task.deviceName }}</b><small>{{ task.userName ? t('nn.personal', { name: task.userName }) : t('nn.global') }} · {{ t('nn.rows', { n: n(task.rows) }) }}</small></span>
                <span class="ap-cell ap-hide-phone"><b class="ap-mono">{{ task.version ?? '—' }}</b><small>{{ task.message ?? '' }}</small></span>
                <span class="ap-cell rc__end"><b :class="TASK_TONE[task.status]">{{ t(`nn.task.${task.status}`) }}</b><small class="tnum">{{ formatRelative(task.finishedAt ?? task.leasedAt, lang) }}</small></span>
              </div>
            </div>
          </template>
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
.rc__group { display: flex; flex-direction: column; align-items: flex-start; gap: 4px; margin-bottom: -16px; }
.rc__group > * { max-width: 100%; }
.rc__group--shared { padding-top: 32px; border-top: 1px solid var(--hair); }
.rc__eyebrow { font-size: 11px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.rc__group-title { display: flex; min-width: 0; margin: 0 0 0 -8px; padding-left: 8px; overflow: hidden; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); text-overflow: ellipsis; white-space: nowrap; }
.rc__slider { display: flex; align-items: center; gap: 16px; width: 280px; max-width: 100%; }
.rc__slider > :first-child { flex: 1; min-width: 0; }
.rc__slider b { min-width: 72px; font-size: var(--fs-small); font-weight: 600; text-align: right; }
.rc__src { --cols: minmax(0, 1fr) 80px 96px 80px; }
.rc__src .ap-tr { min-height: 44px; }
.rc__run { display: flex; flex-wrap: wrap; gap: 8px; padding-top: 8px; }
.rc__facets { --cols: minmax(0, 2fr) minmax(80px, 1fr) 52px 84px; }
.rc__weight { display: block; height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-1); }
.rc__weight i { display: block; height: 100%; border-radius: inherit; background: var(--accent); }
.rc__shown { --cols: 112px minmax(0, 1.2fr) minmax(0, 1.4fr) 112px; }
.rc__reason { font-weight: 500 !important; }
.rc__sessions { --cols: minmax(0, 1fr) auto 88px; }
.rc__end { align-items: flex-end; text-align: right; }
.rc__pools { --cols: minmax(0, 1fr) 64px 48px; }
.rc__mixes { --cols: minmax(0, 1fr) 96px 104px; }
.rc__nn-status { display: flex; flex-direction: column; gap: 8px; padding-bottom: 12px; }
.rc__nn-line { display: flex; align-items: center; gap: 12px; }
.rc__nn-label { font-size: var(--fs-small); color: var(--text-2); }
.rc__nn-meter { flex: 1; max-width: 240px; }
.rc__nn-count { font-size: var(--fs-small); font-weight: 600; }
.rc__nn { --cols: minmax(0, 1fr) 104px 96px 96px 132px; }
.rc__nn .ap-tr { min-height: 52px; }
.rc__note { white-space: normal !important; }
.rc__sub { margin: 20px 0 0; font-size: var(--fs-body); font-weight: 650; }
.rc__tasks { --cols: minmax(0, 1fr) minmax(0, 1.4fr) 132px; }
.rc__err { max-width: 60%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
@media (max-width: 1099px) {
  .ap-stats[style*="--n: 5"] { --n: 3 !important; row-gap: 16px; }
  .ap-stats[style*="--n: 5"] .ap-stat:nth-child(4) { padding-left: 0; border-left: 0; }
}
@media (max-width: 699px) {
  .rc__group { margin-bottom: -8px; }
  .rc__group--shared { padding-top: 24px; }
  .ap-stats .ap-stat:nth-child(odd) { padding-left: 0; border-left: 0; }
  .ap-stats .ap-stat:nth-child(even), .ap-stats[style*="--n: 5"] .ap-stat:nth-child(4) { padding-left: 16px; border-left: 1px solid var(--hair); }
  .rc__slider { width: 100%; }
  .rc__facets { --cols: minmax(0, 1fr) 48px; }
  .rc__facets .ap-tr > :nth-child(2), .rc__facets .ap-tr > :nth-child(4) { display: none; }
  .rc__shown { --cols: minmax(0, 1fr) auto; }
  .rc__sessions { --cols: minmax(0, 1fr) auto; }
  .rc__mixes { --cols: minmax(0, 1fr) auto; }
  .rc__src { --cols: minmax(0, 1fr) 64px 72px 64px; }
  .rc__nn { --cols: minmax(0, 1fr) 96px 112px; column-gap: 12px; }
  .rc__nn .ap-tr > :last-child small { display: none; }
  .rc__tasks { --cols: minmax(0, 1fr) auto; }
  .rc__nn-meter { max-width: none; }
}
</style>
