<script setup lang="ts">
import type { WhyKind, WhyTrack } from '~/api/types'

// «Почему этот трек»: что на самом деле повлияло на выбор волны — понятными словами, без чисел.
// Администратору ниже — подробности, как в админке «Рекомендации». На телефоне — лист снизу,
// на компьютере — окно
const bridge = useBridge()
const auth = useAuthStore()
const { track, session, open } = useWhyTrack()
const { t } = useI18n({ useScope: 'local' })

const data = ref<WhyTrack | null>(null)
const failed = ref(false)
const loading = ref(false)
const showDetails = ref(false)

const ICONS: Record<WhyKind, string> = {
  artist: 'ph:user-sound',
  recent: 'ph:clock-counter-clockwise',
  facet: 'ph:circles-three',
  familiar: 'ph:heart',
  new: 'ph:compass',
  moment: 'ph:sun-horizon',
  seed: 'ph:radio',
  release: 'ph:bell-ringing',
  explore: 'ph:shuffle',
  learned: 'ph:check-circle',
}

watch(open, async (v) => {
  if (!v || !track.value) return
  data.value = null
  failed.value = false
  showDetails.value = false
  loading.value = true
  const id = track.value.id
  try {
    const res = await bridge.wave.why(id, session.value)
    if (track.value?.id === id) data.value = res
  }
  catch {
    failed.value = true
  }
  finally {
    loading.value = false
  }
})

const heading = computed(() => (track.value ? `${track.value.title} · ${track.value.artists.map(a => a.name).join(', ')}` : ''))

/** Подробности администратору — плоско: «why.tasteSim: 0.81» */
const details = computed(() => {
  const d = data.value?.details as Record<string, unknown> | null | undefined
  if (!d) return []
  const out: { key: string, value: string }[] = []
  const walk = (prefix: string, v: unknown) => {
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      for (const [k, x] of Object.entries(v)) walk(prefix ? `${prefix}.${k}` : k, x)
    }
    else if (v !== null && v !== undefined && v !== '') {
      out.push({ key: prefix, value: Array.isArray(v) ? v.join(', ') : String(v) })
    }
  }
  walk('', d)
  return out
})
</script>

<template>
  <UiModal v-model:open="open" :title="t('title')" :description="heading" :width="420">
    <div class="why">
      <div v-if="track" class="why__track">
        <MusicCoverArt class="why__cover" :image="track.album?.image ?? null" size="small" radius="8px" />
        <div class="why__names">
          <span class="why__title ellipsis">{{ track.title }}</span>
          <span class="why__artist ellipsis">{{ track.artists.map(a => a.name).join(', ') }}</span>
        </div>
      </div>

      <div v-if="loading" class="why__lines" aria-busy="true">
        <UiSkeleton v-for="i in 3" :key="i" variant="line" height="20px" class="why__skel" />
      </div>
      <p v-else-if="failed" class="why__empty">{{ t('failed') }}</p>
      <ul v-else-if="data" class="why__lines">
        <li v-for="(l, i) in data.lines" :key="i" class="why__line" :class="{ 'is-main': i === 0 }">
          <Icon :name="ICONS[l.kind] ?? 'ph:circle'" class="why__icon" aria-hidden="true" />
          <span>{{ l.text }}</span>
        </li>
      </ul>

      <template v-if="auth.isAdmin && details.length">
        <button type="button" class="why__more" :aria-expanded="showDetails" @click="showDetails = !showDetails">
          <span>{{ t('details') }}</span>
          <Icon :name="showDetails ? 'ph:caret-up' : 'ph:caret-down'" aria-hidden="true" />
        </button>
        <dl v-if="showDetails" class="why__details">
          <template v-for="d in details" :key="d.key">
            <dt>{{ d.key }}</dt>
            <dd>{{ d.value }}</dd>
          </template>
        </dl>
      </template>
    </div>
  </UiModal>
</template>

<style scoped>
.why { display: flex; flex-direction: column; gap: 14px; padding: 4px 20px 18px; }
.why__track { display: flex; align-items: center; gap: 12px; min-width: 0; }
.why__cover { flex: none; width: 44px; height: 44px; }
.why__names { display: flex; flex-direction: column; min-width: 0; }
.why__title { font-size: var(--fs-body); font-weight: 600; color: var(--text-1); }
.why__artist { font-size: var(--fs-small); color: var(--text-2); }
.why__lines { display: flex; flex-direction: column; gap: 2px; margin: 0; padding: 0; list-style: none; }
.why__line {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 0;
  border-bottom: 1px solid var(--hair);
  font-size: var(--fs-body);
  line-height: 1.4;
  color: var(--text-2);
}
.why__line:last-child { border-bottom: 0; }
.why__line.is-main { color: var(--text-1); }
.why__icon { flex: none; margin-top: 2px; font-size: 20px; color: var(--text-3); }
.why__line.is-main .why__icon { color: var(--accent); }
.why__skel { margin: 10px 0; }
.why__empty { margin: 0; padding: 8px 0; font-size: var(--fs-body); color: var(--text-2); }
.why__more {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  min-height: 32px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--text-3);
  font: inherit;
  font-size: var(--fs-small);
}
.why__more:hover { color: var(--text-2); }
.why__more:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: var(--r-2); }
.why__details {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 12px;
  margin: 0;
  padding: 10px 12px;
  border-radius: var(--r-3);
  background: var(--fill-1);
  font-size: var(--fs-small);
  font-variant-numeric: tabular-nums;
}
.why__details dt { color: var(--text-3); }
.why__details dd { margin: 0; color: var(--text-2); overflow-wrap: anywhere; }
@media (max-width: 699px) {
  .why { padding: 0 20px 12px; }
  .why__line { padding: 12px 0; min-height: 44px; }
  .why__more { min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Почему этот трек",
    "failed": "Не получилось узнать — попробуйте позже",
    "details": "Подробности"
  },
  "uk": {
    "title": "Чому цей трек",
    "failed": "Не вдалося дізнатися — спробуйте пізніше",
    "details": "Подробиці"
  },
  "en": {
    "title": "Why this track",
    "failed": "Couldn't find out — try again later",
    "details": "Details"
  }
}
</i18n>
