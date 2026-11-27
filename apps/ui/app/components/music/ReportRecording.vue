<script setup lang="ts">
import type { WrongRecordingReason } from '~/api/types'

// «Неверная запись»: что не так с тем, что играет, и необязательный комментарий. Сервер ставит запись
// в «Спорные» и скачивает её заново без этого варианта. На телефоне — лист снизу, на компьютере — окно
const bridge = useBridge()
const toast = useToast()
const { track, open } = useReportRecording()
const { t } = useScopedI18n('components.music.ReportRecording')

const REASONS: WrongRecordingReason[] = ['wrong_song', 'wrong_version', 'bad_quality', 'cut']
const reason = ref<WrongRecordingReason | null>(null)
const comment = ref('')
const busy = ref(false)

watch(open, (v) => {
  if (!v) return
  reason.value = null
  comment.value = ''
})

const heading = computed(() => (track.value ? `${track.value.title} · ${track.value.artists.map(a => a.name).join(', ')}` : ''))

async function send() {
  const tr = track.value
  if (!tr || !reason.value || busy.value) return
  busy.value = true
  try {
    await bridge.library.reportRecording(tr.id, { reason: reason.value, comment: comment.value.trim() || null })
    open.value = false
    toast.show({ text: t('thanks'), icon: 'ph:check-circle' })
  }
  catch (e) {
    toast.error((e as { code?: string }).code === 'not_ready' ? t('notReady') : t('failed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <UiModal v-model:open="open" :title="t('title')" :description="heading" :width="400">
    <form class="rr" @submit.prevent="send">
      <p class="rr__what ellipsis">{{ heading }}</p>
      <div class="rr__list" role="radiogroup" :aria-label="t('title')">
        <button
          v-for="r in REASONS"
          :key="r"
          type="button"
          role="radio"
          class="rr__row"
          :class="{ 'is-on': reason === r }"
          :aria-checked="reason === r"
          @click="reason = r"
        >
          <span class="rr__label">{{ t(`reason.${r}`) }}</span>
          <Icon :name="reason === r ? 'ph:check-circle-fill' : 'ph:circle'" class="rr__mark" />
        </button>
      </div>
      <textarea v-model="comment" class="rr__comment" rows="2" maxlength="500" :placeholder="t('comment')" :aria-label="t('comment')" />
      <UiButton type="submit" variant="primary" size="lg" :loading="busy" :disabled="!reason" block>{{ t('send') }}</UiButton>
    </form>
  </UiModal>
</template>

<style scoped>
.rr { display: flex; flex-direction: column; gap: 12px; padding-bottom: 4px; }
.rr__what { margin: -4px 0 0; font-size: var(--fs-small); color: var(--text-2); }
.rr__list { display: flex; flex-direction: column; }
.rr__row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 48px;
  padding: 0 4px;
  border: 0;
  border-bottom: 1px solid var(--hair);
  background: none;
  color: var(--text-1);
  font: inherit;
  text-align: left;
  transition: color var(--dur-1);
}
.rr__row:last-child { border-bottom: 0; }
.rr__row:hover .rr__mark { color: var(--text-2); }
.rr__row:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; border-radius: var(--r-2); }
.rr__label { font-size: var(--fs-body); }
.rr__mark { flex: none; font-size: 22px; color: var(--text-3); transition: color var(--dur-1); }
.rr__row.is-on .rr__mark { color: var(--accent); }
.rr__comment {
  width: 100%;
  min-height: 64px;
  padding: 10px 12px;
  border: 0;
  border-radius: var(--r-3);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--hair);
  color: var(--text-1);
  font: inherit;
  font-size: var(--fs-body);
  line-height: 1.4;
  resize: none;
  outline: none;
}
.rr__comment::placeholder { color: var(--text-3); }
.rr__comment:focus { box-shadow: inset 0 0 0 1px var(--focus); }
@media (max-width: 699px) {
  .rr { padding: 0 20px 8px; }
  .rr__row { min-height: 52px; padding: 0; }
  .rr__comment { font-size: 16px; }
}
</style>
