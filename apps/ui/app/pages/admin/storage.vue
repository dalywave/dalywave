<script setup lang="ts">
import type { StorageOverview } from '~/api/types'
import type { BarDay, BarSeries } from '~/components/admin/Bars.vue'
import type { MeterPart } from '~/components/admin/Meter.vue'

// Хранилище: занятое место по видам, кодекам и источникам, прирост за месяц, самые «тяжёлые»
// артисты, вытесненные копии и оценка цены в Cloudflare R2
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const { data, state, load } = useLoad<StorageOverview>(() => bridge.admin.storage())
const COLORS: Record<string, string> = { audio: 'var(--accent)', covers: 'var(--chart-2)', photos: 'var(--chart-3)' }
const kinds = computed<MeterPart[]>(() => (data.value?.kinds ?? []).map(k => ({
  key: k.key,
  label: t(`kind.${k.key}`),
  value: k.bytes,
  display: formatBytes(k.bytes, lang.value),
  sub: formatCount(k.files, k.key === 'audio' ? 'file' : 'image', lang.value),
  color: COLORS[k.key] ?? 'var(--text-3)',
})))
const cost = computed(() => formatUsd(r2MonthlyUsd(data.value?.totalBytes ?? 0), lang.value))
const growth30 = computed(() => data.value?.growth.reduce((s, d) => s + d.bytes, 0) ?? 0)
const series = computed<BarSeries[]>(() => [{ key: 'bytes', label: t('growth.audio'), color: 'var(--accent)' }])
const days = computed<BarDay[]>(() => (data.value?.growth ?? []).map(d => ({ day: d.day, values: { bytes: d.bytes }, title: `${d.day}: ${formatBytes(d.bytes, lang.value)} · ${formatCount(d.files, 'file', lang.value)}` })))
const maxArtist = computed(() => Math.max(1, ...(data.value?.artists ?? []).map(a => a.bytes)))
const codecName = (c: string) => qualityLabel({ codec: c }) ?? c
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminState :loading="state === 'loading'" :error="state === 'error'" :rows="6" @retry="load">
      <template v-if="data">
        <div class="ap-stats" style="--n: 4">
          <div class="ap-stat"><span class="ap-stat__value">{{ formatBytes(data.totalBytes, lang) }}</span><span class="ap-stat__label">{{ t('stats.total') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ formatBytes(growth30, lang) }}</span><span class="ap-stat__label">{{ t('stats.growth') }}</span></div>
          <div class="ap-stat"><span class="ap-stat__value">{{ cost }}</span><span class="ap-stat__label">{{ t('stats.cost') }}</span><span class="ap-stat__extra">{{ t('stats.price') }}</span></div>
          <div class="ap-stat" :class="{ 'is-warn': data.replacedFiles }"><span class="ap-stat__value">{{ formatBytes(data.replacedBytes, lang) }}</span><span class="ap-stat__label">{{ t('stats.replaced') }}</span><span class="ap-stat__extra">{{ formatCount(data.replacedFiles, 'file', lang) }}</span></div>
        </div>

        <div class="ap-cols">
          <AdminSection :title="t('kinds')">
            <AdminMeter :parts="kinds" :label="t('kinds')" />
          </AdminSection>
          <AdminSection :title="t('growth.title')">
            <AdminBars :days="days" :series="series" :label="t('growth.title')" :height="110" />
          </AdminSection>

          <AdminSection :title="t('codecs')">
            <div v-if="data.codecs.length" class="ap-table st2">
              <div v-for="c in data.codecs" :key="c.key" class="ap-tr">
                <span>{{ codecName(c.key) }}</span>
                <span class="ap-num ap-muted">{{ formatCount(c.files, 'file', lang) }}</span>
                <span class="ap-num">{{ formatBytes(c.bytes, lang) }}</span>
              </div>
            </div>
            <p v-else class="ap-hint">{{ t('noAudio') }}</p>
          </AdminSection>
          <AdminSection :title="t('sources')">
            <div v-if="data.sources.length" class="ap-table st2">
              <div v-for="c in data.sources" :key="c.key" class="ap-tr">
                <span>{{ t(`source.${c.key}`) }}</span>
                <span class="ap-num ap-muted">{{ formatCount(c.files, 'file', lang) }}</span>
                <span class="ap-num">{{ formatBytes(c.bytes, lang) }}</span>
              </div>
            </div>
            <p v-else class="ap-hint">{{ t('noAudio') }}</p>
          </AdminSection>
        </div>

        <AdminSection :title="t('artists')" :hint="t('artistsHint')">
          <div v-if="data.artists.length" class="ap-table st3">
            <div v-for="a in data.artists" :key="a.artist.id" class="ap-tr">
              <NuxtLink :to="`/artist/${a.artist.id}`" class="st3__name">{{ a.artist.name }}</NuxtLink>
              <span class="st3__bar"><i :style="{ width: `${(a.bytes / maxArtist) * 100}%` }" /></span>
              <span class="ap-num ap-muted">{{ formatCount(a.files, 'file', lang) }}</span>
              <span class="ap-num">{{ formatBytes(a.bytes, lang) }}</span>
            </div>
          </div>
          <p v-else class="ap-hint">{{ t('noAudio') }}</p>
        </AdminSection>
      </template>
    </AdminState>
  </div>
</template>

<style scoped>
.st2 { --cols: minmax(0, 1fr) auto 88px; }
.st2 .ap-tr { min-height: 44px; }
.st3 { --cols: minmax(140px, 1fr) minmax(80px, 1.4fr) 96px 88px; }
.st3 .ap-tr { min-height: 44px; }
.st3__name { overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.st3__name:hover { text-decoration: underline; text-underline-offset: 3px; }
.st3__bar { display: block; height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-1); }
.st3__bar i { display: block; height: 100%; border-radius: inherit; background: var(--accent); }
@media (max-width: 1099px) {
  .ap-stats { --n: 2 !important; row-gap: 16px; }
  .ap-stat:nth-child(3) { padding-left: 0; border-left: 0; }
}
@media (max-width: 699px) {
  .st3 { --cols: minmax(0, 1fr) 72px; }
  .st3 .ap-tr > :nth-child(2), .st3 .ap-tr > :nth-child(3) { display: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Хранилище",
    "stats": {
      "total": "занято всего",
      "growth": "прибавилось за 30 дней",
      "cost": "в месяц в R2",
      "price": "$0,015 за ГБ сверх 10 ГБ",
      "replaced": "вытесненные копии"
    },
    "kinds": "По видам",
    "kind": { "audio": "Звук", "covers": "Обложки", "photos": "Фото артистов" },
    "growth": { "title": "Прирост звука", "audio": "Звук за день" },
    "codecs": "Звук по форматам",
    "sources": "Звук по источникам",
    "noAudio": "Звука на сервере пока нет",
    "artists": "Самые «тяжёлые» артисты",
    "artistsHint": "Все файлы записей артиста, включая вытесненные",
    "source": { "youtube_music": "YouTube Music", "youtube": "YouTube", "soundcloud": "SoundCloud", "bandcamp": "Bandcamp", "upload": "Загружено", "other": "Другое" }
  },
  "uk": {
    "title": "Сховище",
    "stats": {
      "total": "зайнято загалом",
      "growth": "додалося за 30 днів",
      "cost": "на місяць у R2",
      "price": "$0,015 за ГБ понад 10 ГБ",
      "replaced": "витіснені копії"
    },
    "kinds": "За видами",
    "kind": { "audio": "Звук", "covers": "Обкладинки", "photos": "Фото виконавців" },
    "growth": { "title": "Приріст звуку", "audio": "Звук за день" },
    "codecs": "Звук за форматами",
    "sources": "Звук за джерелами",
    "noAudio": "Звуку на сервері поки немає",
    "artists": "Найбільш «важкі» виконавці",
    "artistsHint": "Усі файли записів виконавця, включно з витісненими",
    "source": { "youtube_music": "YouTube Music", "youtube": "YouTube", "soundcloud": "SoundCloud", "bandcamp": "Bandcamp", "upload": "Завантажено", "other": "Інше" }
  },
  "en": {
    "title": "Storage",
    "stats": {
      "total": "used in total",
      "growth": "added in 30 days",
      "cost": "per month in R2",
      "price": "$0.015 per GB above 10 GB",
      "replaced": "replaced copies"
    },
    "kinds": "By kind",
    "kind": { "audio": "Audio", "covers": "Covers", "photos": "Artist photos" },
    "growth": { "title": "Audio growth", "audio": "Audio per day" },
    "codecs": "Audio by format",
    "sources": "Audio by source",
    "noAudio": "No audio on the server yet",
    "artists": "Heaviest artists",
    "artistsHint": "All files of the artist's recordings, replaced ones included",
    "source": { "youtube_music": "YouTube Music", "youtube": "YouTube", "soundcloud": "SoundCloud", "bandcamp": "Bandcamp", "upload": "Uploaded", "other": "Other" }
  }
}
</i18n>
