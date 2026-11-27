<script setup lang="ts">
import type { AdminAcquisition, Job } from '~/api/types'

// Задача очереди строкой таблицы: состояние, что, полоса, попытки, когда; повтор и отмена.
// У скачивания по нажатию открывается его история — кандидаты, оценки, проверка
const props = defineProps<{
  job?: Job
  lanes?: Record<string, string>
  names?: Record<string, string>
  /** Строка-шапка с названиями колонок */
  header?: boolean
}>()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const L = useAdminLabels()
const { t } = useScopedI18n('components.admin.JobRow')

const j = computed(() => props.job ?? ({ status: 'queued', kind: '', payload: {} } as unknown as Job))
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
  <div v-if="header" class="jr jr--head" aria-hidden="true">
    <div class="jr__row">
      <div class="jr__main">
        <span>{{ t('col.state') }}</span>
        <span>{{ t('col.job') }}</span>
        <span class="ap-hide-phone">{{ t('col.lane') }}</span>
        <span class="ap-num ap-hide-phone">{{ t('col.attempts') }}</span>
        <span class="ap-num ap-hide-phone">{{ t('col.updated') }}</span>
        <span />
      </div>
      <span class="jr__actions" />
    </div>
  </div>
  <div v-else-if="job" class="jr" :class="{ 'is-open': open }">
    <div class="jr__row">
      <component :is="acqId ? 'button' : 'div'" :type="acqId ? 'button' : undefined" class="jr__main" :class="{ 'is-btn': acqId }" :aria-expanded="acqId ? open : undefined" @click="acqId && toggle()">
        <AdminStatus :tone="JOB_TONE[j.status]" :pulse="j.status === 'running'">{{ L.jobStatus(j.status) }}</AdminStatus>
        <span class="ap-cell">
          <b>{{ acq?.track?.title ?? L.jobKind(j.kind) }}</b>
          <small v-if="j.lastError" class="jr__error" :title="j.lastError">{{ j.lastError }}</small>
          <small v-else>{{ acq?.track ? `${artistNames(acq.track.artists)} · ${L.jobKind(j.kind)}` : subject }}</small>
        </span>
        <span class="jr__lane ap-hide-phone">{{ lanes?.[j.lane] ?? j.lane }}</span>
        <span class="ap-num ap-muted tnum ap-hide-phone">{{ j.attempts }}/{{ j.maxAttempts }}</span>
        <span class="ap-num ap-muted tnum ap-hide-phone">{{ formatRelative(j.updatedAt, lang) }}</span>
        <Icon v-if="acqId" name="ph:caret-down" class="jr__caret" />
        <span v-else />
      </component>
      <div class="jr__actions">
        <UiButton v-if="j.status === 'failed' || j.status === 'cancelled'" size="sm" variant="ghost" icon="ph:arrow-clockwise" @click="retry">{{ t('retry') }}</UiButton>
        <UiIconButton v-else-if="j.status === 'queued' || j.status === 'running'" icon="ph:x" :label="t('cancelJob')" size="sm" @click="cancel" />
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
.jr--head { border-bottom: 1px solid var(--hair); }
.jr--head + .jr { border-top: 0; }
.jr__row { display: flex; align-items: center; gap: 8px; }
.jr__main { display: grid; flex: 1; grid-template-columns: 120px minmax(0, 1.6fr) minmax(0, 0.7fr) 64px 104px 16px; align-items: center; gap: 16px; min-width: 0; min-height: 56px; padding: 8px; margin: 0 -8px; border: 0; border-radius: var(--r-3); background: none; color: inherit; font: inherit; font-size: var(--fs-small); text-align: left; }
.jr--head .jr__main { min-height: 32px; padding-top: 0; padding-bottom: 6px; font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); }
.jr__main.is-btn { cursor: pointer; transition: background-color var(--dur-1); }
.jr__main.is-btn:hover { background: var(--fill-1); }
.jr__main:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.jr__lane { overflow: hidden; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.jr__error { color: var(--danger) !important; }
.jr__caret { font-size: 14px; color: var(--text-3); transition: transform var(--dur-2) var(--ease-out); }
.jr.is-open .jr__caret { transform: rotate(180deg); }
.jr__actions { display: flex; flex: none; justify-content: flex-end; width: 124px; }
.jr__detail { padding: 4px 0 24px; }
@media (max-width: 699px) {
  .jr__main { grid-template-columns: 96px minmax(0, 1fr) 16px; gap: 10px; }
  .jr__actions { width: auto; min-width: 32px; }
}
</style>
