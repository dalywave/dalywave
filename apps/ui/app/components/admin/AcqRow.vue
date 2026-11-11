<script setup lang="ts">
import type { AcquisitionStatus, AdminAcquisition } from '~/api/types'

// Скачивание одной записи строкой: трек, шаг (ищет → качает → проверяет → обрабатывает), зачем,
// кто качает. По нажатию раскрывается история: кандидаты с оценками и попытки
const props = defineProps<{
  acq: AdminAcquisition
  names?: Record<string, string>
}>()
const emit = defineEmits<{ changed: [] }>()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const L = useAdminLabels()
const { t } = useI18n({ useScope: 'local' })

const a = computed(() => props.acq.acquisition)
const STEPS: AcquisitionStatus[] = ['matching', 'downloading', 'verifying', 'processing']
const step = computed(() => STEPS.indexOf(a.value.status))
const active = computed(() => ACQ_ACTIVE.includes(a.value.status))
const tone = computed(() => (a.value.status === 'ready' ? 'ap-ok' : a.value.status === 'disputed' ? 'ap-warn' : ['failed', 'unavailable'].includes(a.value.status) ? 'ap-bad' : active.value && a.value.status !== 'queued' ? 'ap-on' : ''))
const title = computed(() => props.acq.track?.title ?? t('noTrack'))
const artists = computed(() => (props.acq.track ? artistNames(props.acq.track.artists) : ''))
const open = ref(false)
const busy = ref(false)

async function cancel() {
  if (!props.acq.jobId) return
  busy.value = true
  try {
    await bridge.admin.cancelJob(props.acq.jobId)
    toast.show({ text: t('cancelled', { title: title.value }), icon: 'ph:x-circle' })
    emit('changed')
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="ar" :class="{ 'is-open': open }">
    <div class="ar__row">
      <button type="button" class="ar__main" :aria-expanded="open" @click="open = !open">
        <span class="ap-cell">
          <b>{{ title }}</b>
          <small>{{ artists }}</small>
        </span>
        <span class="ar__state">
          <span class="ar__status" :class="tone">
            <span v-if="active" class="ap-dot" :class="{ 'is-on': a.status !== 'queued' }" />
            {{ L.acqStatus(a.status) }}
            <template v-if="a.status === 'queued' && a.queuePosition != null"> · {{ t('position', { n: a.queuePosition + 1 }) }}</template>
          </span>
          <span v-if="active && a.status !== 'queued'" class="ar__steps" aria-hidden="true">
            <i v-for="(s, i) in STEPS" :key="s" :class="{ 'is-done': i < step, 'is-now': i === step }" />
          </span>
          <small v-else-if="a.error && !active" class="ar__error" :title="a.error">{{ a.error }}</small>
        </span>
        <span class="ar__meta ap-hide-phone">
          <span>{{ L.reason(a.reason) }}</span>
          <span v-if="acq.worker">{{ L.executor(acq.worker, names) }}</span>
        </span>
        <span class="ar__time ap-muted tnum ap-hide-phone">{{ formatRelative(a.updatedAt, lang) }}</span>
        <Icon name="ph:caret-down" class="ar__caret" />
      </button>
      <UiIconButton v-if="active && acq.jobId" icon="ph:x" :label="t('cancel')" size="sm" :disabled="busy" @click="cancel" />
    </div>
    <div v-if="open" class="ar__detail">
      <AdminAcquisition :acq="acq" :names="names" />
    </div>
  </div>
</template>

<style scoped>
.ar { border-top: 1px solid var(--hair); }
.ar:first-child { border-top: 0; }
.ar__row { display: flex; align-items: center; gap: 8px; }
.ar__main { display: grid; flex: 1; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) minmax(0, 0.8fr) 84px 16px; align-items: center; gap: 16px; min-width: 0; min-height: 56px; padding: 8px 8px; margin: 0 -8px; border: 0; border-radius: var(--r-3); background: none; color: inherit; font: inherit; font-size: var(--fs-small); text-align: left; cursor: pointer; transition: background-color var(--dur-1); }
.ar__main:hover { background: var(--fill-1); }
.ar__main:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.ar__state { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.ar__status { display: inline-flex; align-items: center; gap: 8px; font-weight: 600; white-space: nowrap; }
.ar__steps { display: flex; gap: 3px; width: 120px; }
.ar__steps i { flex: 1; height: 3px; border-radius: 2px; background: var(--fill-3); }
.ar__steps i.is-done { background: var(--accent); }
.ar__steps i.is-now { background: var(--accent); animation: ap-pulse 1.4s var(--ease-in-out) infinite; }
.ar__error { overflow: hidden; color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-caption); }
.ar__meta { display: flex; flex-direction: column; gap: 2px; min-width: 0; font-size: var(--fs-caption); color: var(--text-2); }
.ar__meta span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ar__time { font-size: var(--fs-caption); text-align: right; }
.ar__caret { font-size: 14px; color: var(--text-3); transition: transform var(--dur-2) var(--ease-out); }
.ar.is-open .ar__caret { transform: rotate(180deg); }
.ar__detail { padding: 8px 0 24px; }
@media (prefers-reduced-motion: reduce) { .ar__steps i.is-now { animation: none; } }
@media (max-width: 699px) {
  .ar__main { grid-template-columns: minmax(0, 1fr) minmax(0, 0.9fr) 16px; gap: 10px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "noTrack": "Трек удалён из каталога",
    "position": "{n}-я",
    "cancel": "Отменить скачивание",
    "cancelled": "«{title}» больше не качается",
    "failed": "Не получилось — скачивание уже изменилось"
  },
  "en": {
    "noTrack": "Track removed from the catalog",
    "position": "#{n}",
    "cancel": "Cancel download",
    "cancelled": "“{title}” is no longer downloading",
    "failed": "Didn't work — the download already changed"
  }
}
</i18n>
