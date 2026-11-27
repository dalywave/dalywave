<script setup lang="ts">
import type { TasteFacet } from '~/api/types'

// Грань вкуса: обложки примеров, доля, главные артисты, когда слушаешь чаще; «Слушать» — волна от
// самого характерного трека грани, «Меньше такого» — приглушить (повтор — сильнее), «Вернуть» — отменить
const props = defineProps<{
  facet: TasteFacet
  color: string
  busy?: boolean
}>()
const emit = defineEmits<{ less: [], undo: [] }>()

const player = usePlayerStore()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const covers = computed(() => {
  const seen = new Set<string>()
  const out = []
  for (const tr of props.facet.examples) {
    const img = tr.album?.image
    if (!img || seen.has(img.small)) continue
    seen.add(img.small)
    out.push(img)
  }
  return out
})
const percent = computed(() => Math.max(1, Math.round(props.facet.share * 100)))
const damped = computed(() => props.facet.damped > 0.05)
// Название грани обычно уже из имён артистов — под ним только те, кого в названии нет
const artistLine = computed(() => {
  const label = props.facet.label.toLowerCase()
  const rest = props.facet.artists.filter(a => !label.includes(a.name.toLowerCase())).map(a => a.name)
  if (rest.length === props.facet.artists.length) return rest.join(', ')
  return rest.length ? t('also', { names: rest.join(', ') }) : ''
})
const meta = computed(() => [
  props.facet.peak ? t(`peak.${props.facet.peak}`) : null,
  formatCount(props.facet.tracks, 'track', lang.value),
].filter(Boolean).join(' · '))

function listen() {
  const seed = props.facet.examples[0]
  if (!seed) return
  void player.startWave(undefined, { seed: { type: 'track', id: seed.id }, title: props.facet.label })
}
</script>

<template>
  <article class="fc" :class="{ 'is-damped': damped }" :style="{ '--fc': color }">
    <div class="fc__art">
      <MusicCoverArt v-if="covers.length >= 4" :mosaic="covers" size="small" radius="var(--r-3)" />
      <MusicCoverArt v-else :image="covers[0] ?? null" size="medium" radius="var(--r-3)" />
    </div>
    <div class="fc__body">
      <div class="fc__top">
        <span class="fc__share tnum"><i />{{ percent }}&#8239;%</span>
        <span class="fc__meta">{{ meta }}</span>
      </div>
      <h3 class="fc__title">{{ facet.label }}</h3>
      <div v-if="facet.artists.length" class="fc__artists">
        <span class="fc__faces" aria-hidden="true">
          <MusicCoverArt v-for="a in facet.artists" :key="a.id" :image="a.image" size="small" round class="fc__face" fallback-icon="ph:user" />
        </span>
        <span v-if="artistLine" class="fc__names ellipsis">{{ artistLine }}</span>
      </div>
      <div class="fc__actions">
        <MusicPlayButton v-if="ui.isPhone" :playing="false" :size="44" :disabled="!facet.examples.length" :label="`${t('listen')}: ${facet.label}`" @click="listen" />
        <UiButton v-else size="sm" variant="primary" icon="ph:play-fill" :disabled="!facet.examples.length" @click="listen">{{ t('listen') }}</UiButton>
        <UiButton v-if="damped" size="sm" variant="ghost" icon="ph:arrow-counter-clockwise" :loading="busy" @click="emit('undo')">{{ t('undo') }}</UiButton>
        <UiButton v-else size="sm" variant="ghost" icon="ph:minus-circle" :loading="busy" @click="emit('less')">{{ t('lessBtn') }}</UiButton>
      </div>
    </div>
    <span class="fc__bar" aria-hidden="true"><i :style="{ width: `${percent}%` }" /></span>
  </article>
</template>

<style scoped>
.fc {
  position: relative;
  display: grid;
  grid-template-columns: 132px minmax(0, 1fr);
  gap: 20px;
  min-width: 0;
  padding: 16px 20px 20px 16px;
  overflow: hidden;
  border-radius: var(--r-5);
  background: var(--ink-2);
  box-shadow: inset 0 0 0 1px var(--hair);
  transition: filter var(--dur-3) var(--ease-out), opacity var(--dur-3) var(--ease-out);
}
.fc.is-damped .fc__art, .fc.is-damped .fc__title, .fc.is-damped .fc__artists { opacity: 0.55; filter: saturate(0.4); }
.fc__art { position: relative; width: 132px; aspect-ratio: 1; box-shadow: var(--shadow-1); border-radius: var(--r-3); }
.fc__body { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.fc__top { display: flex; align-items: center; gap: 10px; min-width: 0; }
.fc__share { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-small); font-weight: 700; color: var(--text-1); }
.fc__share i { width: 8px; height: 8px; border-radius: 50%; background: var(--fc); }
.fc__meta { overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
.fc__title { display: -webkit-box; margin: 0; overflow: hidden; font-size: var(--fs-h4); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.25; -webkit-line-clamp: 2; -webkit-box-orient: vertical; }
.fc__artists { display: flex; align-items: center; gap: 8px; min-width: 0; font-size: var(--fs-small); color: var(--text-2); }
.fc__faces { display: inline-flex; flex: none; }
.fc__face { width: 22px; height: 22px; box-shadow: 0 0 0 2px var(--ink-2); }
.fc__face + .fc__face { margin-left: -6px; }
.fc__actions { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; margin-top: auto; padding-top: 8px; }
.fc__bar { position: absolute; left: 0; right: 0; bottom: 0; height: 3px; background: var(--fill-1); }
.fc__bar i { display: block; height: 100%; border-radius: 0 3px 3px 0; background: var(--fc); }

@media (max-width: 699px) {
  .fc { grid-template-columns: 96px minmax(0, 1fr); gap: 14px; padding: 12px 14px 16px 12px; border-radius: var(--r-4); }
  .fc__art { width: 96px; }
  .fc__actions :deep(.btn) { min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "listen": "Слушать",
    "lessBtn": "Меньше такого",
    "undo": "Вернуть",
    "also": "ещё {names}",
    "peak": {
      "morning": "чаще утром",
      "day": "чаще днём",
      "evening": "чаще вечером",
      "night": "чаще ночью"
    }
  },
  "uk": {
    "listen": "Слухати",
    "lessBtn": "Менше такого",
    "undo": "Повернути",
    "also": "ще {names}",
    "peak": {
      "morning": "частіше вранці",
      "day": "частіше вдень",
      "evening": "частіше ввечері",
      "night": "частіше вночі"
    }
  },
  "en": {
    "listen": "Play",
    "lessBtn": "Less like this",
    "undo": "Restore",
    "also": "also {names}",
    "peak": {
      "morning": "mostly mornings",
      "day": "mostly afternoons",
      "evening": "mostly evenings",
      "night": "mostly at night"
    }
  }
}
</i18n>
