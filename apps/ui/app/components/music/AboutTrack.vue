<script setup lang="ts">
import type { Track } from '~/api/types'

// «О треке»: альбом, место в нём, длительность, темп и качество звука
const props = defineProps<{ track: Track | null }>()
const player = usePlayerStore()
const { t } = useI18n()

const rows = computed(() => {
  const tr = props.track
  if (!tr) return []
  const now = player.current?.track.id === tr.id ? player.source : null
  const out: { label: string, value: string, to?: string }[] = []
  if (tr.album) out.push({ label: t('record.album'), value: tr.album.title, to: `/album/${tr.album.id}` })
  if (tr.trackPosition) out.push({ label: t('about.position'), value: tr.discNumber > 1 ? `${tr.discNumber}–${tr.trackPosition}` : String(tr.trackPosition) })
  out.push({ label: t('about.duration'), value: formatDuration(tr.durationMs) })
  if (now?.bpm) out.push({ label: t('about.tempo'), value: `${Math.round(now.bpm)} BPM` })
  // Кодек и битрейт — только здесь (и в админке): в плеере их нет
  const q = now && (now.preview ? t('player.preview') : qualityLabel(now))
  if (q) out.push({ label: t('about.quality'), value: q })
  return out
})
</script>

<template>
  <UiEmptyState v-if="!track" icon="ph:info" :title="t('player.nothing')" :text="t('player.nothingHint')" compact />
  <div v-else class="about">
    <div class="about__head">
      <MusicCoverArt :image="track.album?.image" size="medium" :alt="track.album?.title ?? ''" class="about__art" />
      <div class="about__titles">
        <div class="about__title">{{ track.title }}<UiBadge v-if="track.explicit" variant="explicit" /></div>
        <div class="about__artist">
          <template v-for="(a, i) in track.artists" :key="a.id"><span v-if="i">, </span><NuxtLink :to="`/artist/${a.id}`">{{ a.name }}</NuxtLink></template>
        </div>
      </div>
    </div>
    <dl class="about__list">
      <div v-for="r in rows" :key="r.label" class="about__row">
        <dt>{{ r.label }}</dt>
        <dd class="ellipsis"><NuxtLink v-if="r.to" :to="r.to">{{ r.value }}</NuxtLink><template v-else>{{ r.value }}</template></dd>
      </div>
    </dl>
  </div>
</template>

<style scoped>
.about { display: flex; flex-direction: column; gap: 12px; padding: 12px 8px; }
.about__head { display: flex; align-items: center; gap: 14px; }
.about__art { width: 72px; border-radius: 10px; }
.about__titles { min-width: 0; }
.about__title { display: flex; align-items: center; gap: 6px; font-size: var(--fs-h4); font-weight: 650; line-height: 1.25; }
.about__artist { color: var(--text-2); }
.about__artist a:hover { color: var(--text-1); }
.about__list { margin: 0; }
.about__row { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; min-height: 44px; padding: 12px 0; box-shadow: inset 0 -0.5px 0 var(--hair); }
.about__row:last-child { box-shadow: none; }
.about__list dt { flex: none; color: var(--text-2); }
.about__list dd { min-width: 0; margin: 0; text-align: right; font-variant-numeric: tabular-nums; }
.about__list a:hover { text-decoration: underline; text-decoration-color: var(--text-4); text-underline-offset: 3px; }
</style>
