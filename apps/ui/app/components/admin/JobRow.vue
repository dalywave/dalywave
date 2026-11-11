<script setup lang="ts">
import type { AdminAcquisition, Job } from '~/api/types'

// Задача очереди строкой: состояние, вид, полоса, попытки, ошибка; повтор и отмена.
// У скачивания по нажатию открывается его история — кандидаты, оценки, проверка
const props = defineProps<{
  job: Job
  lanes?: Record<string, string>
  names?: Record<string, string>
}>()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const L = useAdminLabels()
const { t } = useI18n({ useScope: 'local' })

const j = computed(() => props.job)
const p = computed(() => (j.value.payload ?? {}) as Record<string, unknown>)
const acqId = computed(() => (j.value.kind === 'acquire' && typeof p.value.acquisitionId === 'string' ? p.value.acquisitionId : null))
const subject = computed(() => {
  const v = p.value
  const owner = typeof v.owner === 'string' ? (v.owner === 'album' ? t('album') : t('artist')) : null
  const id = [v.id, v.trackId, v.artistId, v.recordingId].find(x => typeof x === 'string') as string | undefined
  return [owner, id ? `…${id.slice(-6)}` : null].filter(Boolean).join(' ')
})

const open = ref(false)
const acq = ref<AdminAcquisition | null>(null)
const acqFailed = ref(false)
async function toggle() {
  open.value = !open.value
  if (open.value && acqId.value && !acq.value) {
    try {
      acq.value = await bridge.admin.acquisition(acqId.value)
    }
    catch {
      acqFailed.value = true
    }
  }
}

async function retry() {
  try {
    await bridge.admin.retryJob(j.value.id)
    j.value.status = 'queued'
    toast.show({ text: t('retried'), icon: 'ph:arrow-clockwise' })
  }
  catch {
    toast.error(t('actionFailed'))
  }
}
async function cancel() {
  try {
    await bridge.admin.cancelJob(j.value.id)
    j.value.status = 'cancelled'
    toast.show({ text: t('cancelled'), icon: 'ph:x-circle' })
  }
  catch {
    toast.error(t('actionFailed'))
  }
}
</script>

<template>
  <div class="jr" :class="{ 'is-open': open }">
    <div class="jr__row">
      <UiBadge :variant="JOB_TONE[j.status]" class="jr__status">{{ L.jobStatus(j.status) }}</UiBadge>
      <component :is="acqId ? 'button' : 'div'" :type="acqId ? 'button' : undefined" class="jr__main" :class="{ 'is-btn': acqId }" :aria-expanded="acqId ? open : undefined" @click="acqId && toggle()">
        <span class="jr__title">
          <b>{{ acq?.track?.title ?? L.jobKind(j.kind) }}</b>
          <span class="ap-muted">{{ acq?.track ? artistNames(acq.track.artists) : subject }}</span>
        </span>
        <span class="jr__meta">
          <span>{{ lanes?.[j.lane] ?? j.lane }}</span>
          <span v-if="acq?.track">{{ L.jobKind(j.kind) }}</span>
          <span class="tnum">{{ t('attempts', { n: j.attempts, max: j.maxAttempts }) }}</span>
          <span>{{ formatRelative(j.updatedAt, lang) }}</span>
        </span>
        <span v-if="j.lastError" class="jr__error" :title="j.lastError">{{ j.lastError }}</span>
        <Icon v-if="acqId" name="ph:caret-down" class="jr__caret" />
      </component>
      <div class="jr__actions">
        <UiButton v-if="j.status === 'failed' || j.status === 'cancelled'" size="sm" icon="ph:arrow-clockwise" @click="retry">{{ t('retry') }}</UiButton>
        <UiButton v-else-if="j.status === 'queued' || j.status === 'running'" size="sm" variant="ghost" icon="ph:x" @click="cancel">{{ t('cancel') }}</UiButton>
      </div>
    </div>
    <div v-if="open" class="jr__detail">
      <AdminAcquisition v-if="acq" :acq="acq" :names="names" />
      <p v-else-if="acqFailed" class="ap-hint">{{ t('noHistory') }}</p>
      <UiSkeleton v-else variant="line" height="16px" width="60%" />
    </div>
  </div>
</template>

<style scoped>
.jr { border-top: 1px solid var(--hair); }
.jr:first-child { border-top: 0; }
.jr__row { display: flex; align-items: center; gap: 14px; min-height: 56px; padding: 6px 0; }
.jr__status { flex: none; min-width: 88px; justify-content: center; }
.jr__main { position: relative; display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; padding: 6px 28px 6px 8px; margin: 0 0 0 -8px; border: 0; border-radius: var(--r-3); background: none; color: inherit; font: inherit; text-align: left; }
.jr__main.is-btn { cursor: pointer; transition: background-color var(--dur-1); }
.jr__main.is-btn:hover { background: var(--fill-1); }
.jr__main:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.jr__title { display: flex; align-items: baseline; gap: 8px; min-width: 0; overflow: hidden; white-space: nowrap; }
.jr__title b { overflow: hidden; font-weight: 600; text-overflow: ellipsis; }
.jr__title span { overflow: hidden; font-size: var(--fs-small); text-overflow: ellipsis; }
.jr__meta { display: flex; flex-wrap: wrap; gap: 2px 12px; font-size: var(--fs-caption); color: var(--text-3); }
.jr__error { overflow: hidden; font-size: var(--fs-caption); color: var(--danger); text-overflow: ellipsis; white-space: nowrap; }
.jr__caret { position: absolute; top: 50%; right: 8px; font-size: 14px; color: var(--text-3); transform: translateY(-50%); transition: transform var(--dur-2) var(--ease-out); }
.jr.is-open .jr__caret { transform: translateY(-50%) rotate(180deg); }
.jr__actions { flex: none; }
.jr__detail { padding: 4px 0 24px; }
@media (max-width: 699px) {
  .jr__row { display: grid; grid-template-columns: minmax(0, 1fr) auto; row-gap: 4px; column-gap: 8px; }
  .jr__status { grid-row: 1; grid-column: 1; justify-self: start; min-width: 0; }
  .jr__actions { grid-row: 1; grid-column: 2; }
  .jr__main { grid-column: 1 / -1; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "album": "альбом",
    "artist": "артист",
    "attempts": "попытка {n} из {max}",
    "retry": "Повторить",
    "cancel": "Отменить",
    "retried": "Задача снова в очереди",
    "cancelled": "Задача отменена",
    "actionFailed": "Не получилось — задача уже изменилась, обновите список",
    "noHistory": "Истории скачивания нет"
  },
  "en": {
    "album": "album",
    "artist": "artist",
    "attempts": "attempt {n} of {max}",
    "retry": "Retry",
    "cancel": "Cancel",
    "retried": "Job queued again",
    "cancelled": "Job cancelled",
    "actionFailed": "Didn't work — the job already changed, refresh the list",
    "noHistory": "No download history"
  }
}
</i18n>
