<script setup lang="ts">
import type { AdminImport, AdminImportList } from '~/api/types'

// Импорт: история импортов всех людей — что, чьё, на каком шаге, сколько сопоставлено и что осталось
// без пары. Идущий импорт обновляется сам
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const { data, state, load } = useLoad<AdminImportList>(() => bridge.admin.imports())
const running = computed(() => data.value?.items.some(i => i.status === 'running') ?? false)
useIntervalFn(() => {
  if (running.value) void load()
}, 3000)
const pct = (done: number, total: number) => (total ? Math.round((done / total) * 100) : 0)

const STAT_RU: Record<string, [string, string, string]> = {
  likes: ['лайк', 'лайка', 'лайков'],
  playlists: ['плейлист', 'плейлиста', 'плейлистов'],
  albums: ['альбом', 'альбома', 'альбомов'],
  artists: ['подписка', 'подписки', 'подписок'],
  plays: ['прослушивание', 'прослушивания', 'прослушиваний'],
  files: ['файл', 'файла', 'файлов'],
  upgraded: ['замена', 'замены', 'замен'],
}

/** «1 240 лайков · 38 плейлистов · 52 000 прослушиваний» — только ненулевое */
function statsLine(i: AdminImport): string {
  const s = i.stats
  const parts: [number, string][] = [[s.likes, 'likes'], [s.playlists, 'playlists'], [s.albums, 'albums'], [s.artists, 'artists'], [s.plays, 'plays'], [s.files, 'files'], [s.upgraded, 'upgraded']]
  return parts
    .filter(([n]) => n > 0)
    .map(([n, k]) => `${formatNumber(n, lang.value)}\u00a0${lang.value === 'en' ? t(`stat.${k}`) : plural(n, STAT_RU[k] ?? ['', '', ''])}`)
    .join(' · ')
}
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminSection :title="t('history')">
      <AdminState
        :loading="state === 'loading'"
        :error="state === 'error'"
        :empty="!!data && !data.items.length"
        empty-icon="ph:tray-arrow-down"
        :empty-title="t('empty')"
        :empty-text="t('emptyText')"
        @retry="load"
      >
        <div v-if="data" class="ap-table im">
          <div class="ap-tr ap-tr--head">
            <span>{{ t('col.what') }}</span>
            <span>{{ t('col.state') }}</span>
            <span class="ap-num">{{ t('col.matched') }}</span>
            <span class="ap-num">{{ t('col.unmatched') }}</span>
            <span class="ap-num ap-hide-phone">{{ t('col.when') }}</span>
          </div>
          <div v-for="i in data.items" :key="i.id" class="ap-tr">
            <span class="ap-cell"><b>{{ i.title }}</b><small>{{ t(`kind.${i.kind}`) }} · {{ i.userName }}<template v-if="statsLine(i)"> · {{ statsLine(i) }}</template></small></span>
            <span class="ap-cell">
              <AdminStatus :tone="i.status === 'done' ? 'ok' : i.status === 'failed' ? 'bad' : i.status === 'cancelled' ? 'idle' : 'on'" :pulse="i.status === 'running'">{{ i.status === 'running' ? t(`phase.${i.phase}`) : t(`status.${i.status}`) }}</AdminStatus>
              <span v-if="i.status === 'running'" class="im__bar"><i :style="{ width: `${pct(i.processed, i.total)}%` }" /></span>
              <small v-else-if="i.error" class="ap-bad">{{ i.error }}</small>
            </span>
            <span class="ap-num">{{ formatNumber(i.matched, lang) }} / {{ formatNumber(i.total, lang) }}</span>
            <span class="ap-num" :class="{ 'ap-warn': i.unmatched }">{{ i.unmatched ? formatNumber(i.unmatched, lang) : '—' }}</span>
            <span class="ap-num ap-muted ap-hide-phone">{{ formatRelative(i.createdAt, lang) }}</span>
          </div>
        </div>
      </AdminState>
    </AdminSection>
  </div>
</template>

<style scoped>
.im { --cols: minmax(0, 2fr) minmax(110px, 1fr) 110px 110px 120px; }
.im__bar { display: block; width: 100%; max-width: 160px; height: 4px; overflow: hidden; border-radius: 2px; background: var(--fill-2); }
.im__bar i { display: block; height: 100%; background: var(--accent); }
@media (max-width: 699px) {
  .im { --cols: minmax(0, 1fr) 84px 64px; }
  .im .ap-tr > :nth-child(2) { display: none; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Импорт",
    "history": "История",
    "empty": "Импортов пока не было",
    "emptyText": "Здесь появятся переносы архива Spotify, списков и своих файлов — с ходом и позициями, которые не нашлись",
    "col": { "what": "Что", "state": "Состояние", "matched": "Найдено", "unmatched": "Без пары", "when": "Когда" },
    "kind": { "text": "Список текстом", "csv": "Таблица CSV", "m3u": "Плейлист M3U", "files": "Файлы", "spotify_archive": "Архив Spotify", "yandex": "Яндекс Музыка", "youtube_playlist": "Плейлист YouTube Music", "youtube_takeout": "Архив YouTube Music" },
    "status": { "running": "Идёт", "done": "Готово", "failed": "Ошибка", "cancelled": "Остановлен" },
    "phase": { "uploading": "Загрузка файлов", "matching": "Поиск в каталоге", "applying": "Раскладка", "done": "Готово" }
  },
  "en": {
    "title": "Import",
    "history": "History",
    "empty": "No imports yet",
    "emptyText": "Transfers of the Spotify archive, lists and your own files will show up here — with progress and items that weren't found",
    "col": { "what": "What", "state": "State", "matched": "Found", "unmatched": "Unmatched", "when": "When" },
    "kind": { "text": "Text list", "csv": "CSV table", "m3u": "M3U playlist", "files": "Files", "spotify_archive": "Spotify archive", "yandex": "Yandex Music", "youtube_playlist": "YouTube Music playlist", "youtube_takeout": "YouTube Music archive" },
    "status": { "running": "Running", "done": "Done", "failed": "Failed", "cancelled": "Stopped" },
    "phase": { "uploading": "Uploading files", "matching": "Catalog search", "applying": "Applying", "done": "Done" },
    "stat": { "likes": "likes", "playlists": "playlists", "albums": "albums", "artists": "follows", "plays": "plays", "files": "files", "upgraded": "upgraded" }
  }
}
</i18n>
