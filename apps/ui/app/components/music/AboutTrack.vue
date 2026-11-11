<script setup lang="ts">
import type { Track } from '~/api/types'

// «О треке»: откуда запись и в каком она состоянии на сервере
const props = defineProps<{ track: Track | null }>()
const player = usePlayerStore()
const { t } = useI18n()

const rows = computed(() => {
  const tr = props.track
  if (!tr) return []
  const out: { label: string, value: string, to?: string }[] = []
  if (tr.album) out.push({ label: t('record.album'), value: tr.album.title, to: `/album/${tr.album.id}` })
  out.push({ label: t('about.artists'), value: tr.artists.map(a => a.name).join(', ') })
  if (tr.trackPosition) out.push({ label: t('about.position'), value: tr.discNumber > 1 ? `${tr.discNumber}–${tr.trackPosition}` : String(tr.trackPosition) })
  out.push({ label: t('about.duration'), value: formatDuration(tr.durationMs) })
  out.push({ label: t('about.status'), value: t(`track.status.${tr.audioStatus}`) })
  if (player.current?.track.id === tr.id && player.source) out.push({ label: t('about.file'), value: player.source.preview ? t('player.preview') : t('player.quality') })
  if (player.current?.track.id === tr.id && player.source?.bpm) out.push({ label: t('about.tempo'), value: `${Math.round(player.source.bpm)} BPM` })
  if (tr.isrc) out.push({ label: 'ISRC', value: tr.isrc })
  if (tr.deezerId) out.push({ label: 'Deezer', value: String(tr.deezerId) })
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
        <div class="about__artist">{{ track.artists.map(a => a.name).join(', ') }}</div>
      </div>
    </div>
    <dl class="about__list">
      <template v-for="r in rows" :key="r.label">
        <dt>{{ r.label }}</dt>
        <dd><NuxtLink v-if="r.to" :to="r.to">{{ r.value }}</NuxtLink><template v-else>{{ r.value }}</template></dd>
      </template>
    </dl>
  </div>
</template>

<style scoped>
.about { display: flex; flex-direction: column; gap: 20px; padding: 12px 8px; }
.about__head { display: flex; align-items: center; gap: 14px; }
.about__art { width: 72px; border-radius: 10px; }
.about__titles { min-width: 0; }
.about__title { display: flex; align-items: center; gap: 6px; font-size: var(--fs-h4); font-weight: 650; }
.about__artist { color: var(--text-2); }
.about__list { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 10px 16px; margin: 0; font-size: var(--fs-small); }
.about__list dt { color: var(--text-3); }
.about__list dd { margin: 0; overflow-wrap: anywhere; }
.about__list a { color: var(--text-1); text-decoration: underline; text-decoration-color: var(--text-4); text-underline-offset: 3px; }
</style>
