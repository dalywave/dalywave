<script setup lang="ts">
import type { AlbumSummary, ArtistSummary, Home, PlaylistSummary, Shelf, ShelfItem, Track } from '~/api/types'

// Главная: «Моя волна» и полки с сервера в порядке, который сервер выбрал по времени суток: продолжить,
// миксы дня, радар новинок, новинки подписок, открытия недели, недавно понравилось, забытые, ваши артисты
const bridge = useBridge()
const auth = useAuthStore()
const ui = useUiStore()
const player = usePlayerStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg, te } = useGlobalT()
useHead({ title: () => t('title') })

const home = ref<Home | null>(null)
const failed = ref(false)

async function load() {
  failed.value = false
  try {
    home.value = await bridge.home()
  }
  catch {
    failed.value = true
  }
}
onMounted(load)

const firstName = computed(() => auth.user?.name?.split(' ')[0] ?? '')
const greeting = computed(() => {
  const h = new Date().getHours()
  const key = h < 5 ? 'night' : h < 12 ? 'morning' : h < 17 ? 'day' : h < 23 ? 'evening' : 'night'
  return firstName.value ? t(`greet.${key}`, { name: firstName.value }) : t(`greetShort.${key}`)
})
const today = computed(() => new Intl.DateTimeFormat(lang.value === 'en' ? 'en-GB' : 'ru-RU', { weekday: 'long', day: 'numeric', month: 'long' }).format(new Date()))

const isNew = (a: AlbumSummary) => !!a.releaseDate && Date.now() - new Date(a.releaseDate).getTime() < 14 * 86_400_000
const albums = (s: Shelf) => s.items.filter((i): i is ShelfItem & { type: 'album' } => i.type === 'album') as AlbumSummary[]
const artists = (s: Shelf) => s.items.filter((i): i is ShelfItem & { type: 'artist' } => i.type === 'artist') as ArtistSummary[]
const tracks = (s: Shelf) => s.items.filter((i): i is ShelfItem & { type: 'track' } => i.type === 'track') as Track[]
const playlists = (s: Shelf) => s.items.filter((i): i is ShelfItem & { type: 'playlist' } => i.type === 'playlist') as PlaylistSummary[]
const TRACK_SHELVES = new Set(['discover_weekly', 'forgotten_favorites'])

/** Полка треков: играть с выбранного, контекст — «микс» с названием полки */
function playShelf(s: Shelf, i: number) {
  const list = tracks(s)
  const tr = list[i]
  if (tr && player.current?.track.id === tr.id) return player.toggle()
  return player.playTracks(list, i, { type: 'mix', id: s.id, title: shelfTitle(s) })
}

/** Заголовки полок сервер присылает по-русски; переводим по id, неизвестные оставляем как есть */
function shelfTitle(s: Shelf) {
  const key = `home.shelf.${s.id}`
  return te(key) ? tg(key) : s.title
}

// «Недавно понравилось» — превью, а не весь список: на телефоне пять строк, на большом экране две колонки по пять
const likesShown = computed(() => (ui.isPhone ? 5 : 10))

function playLiked(list: Track[], i: number) {
  const tr = list[i]
  if (tr && player.current?.track.id === tr.id) return player.toggle()
  return player.playTracks(list, i, { type: 'library', id: null, title: t('recent') })
}
</script>

<template>
  <div class="home">
    <header v-if="ui.isPhone" class="phead">
      <div>
        <div class="phead__date">{{ today.charAt(0).toUpperCase() + today.slice(1) }}</div>
        <h1 class="phead__hello">{{ greeting }}</h1>
      </div>
      <NuxtLink to="/settings" class="phead__avatar" :aria-label="t('settings')">
        <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="" class="phead__avatar-img">
        <template v-else>{{ (auth.user?.name ?? '?').charAt(0).toUpperCase() }}</template>
      </NuxtLink>
    </header>

    <MusicWaveHero :greeting="ui.isPhone ? undefined : greeting" />

    <!-- Итоги года — в декабре -->
    <RewindCard season-only />

    <template v-if="home">
      <template v-for="s in home.shelves" :key="s.id">
        <MusicShelf v-if="s.kind === 'continue'" :title="shelfTitle(s)" :cols="3" :rows="s.items.length > 3 ? 2 : 1" phone-width="82%">
          <MusicResumeCard v-for="it in s.items" :key="`${it.type}:${it.id}`" :item="it" />
        </MusicShelf>

        <MusicShelf v-else-if="s.kind === 'daily_mixes'" :title="shelfTitle(s)" :cols="5">
          <MusicDailyMixCard v-for="(p, i) in playlists(s)" :key="p.id" :playlist="p" :index="i + 1" />
        </MusicShelf>

        <MusicShelf v-else-if="TRACK_SHELVES.has(s.kind)" :title="shelfTitle(s)" :hint="t(`hint.${s.kind}`)" :cols="6" phone-width="136px">
          <template #actions>
            <UiButton variant="ghost" size="sm" icon="ph:play-fill" @click="playShelf(s, 0)">{{ t('playAll') }}</UiButton>
          </template>
          <MusicTrackCard v-for="(tr, i) in tracks(s)" :key="tr.id" :track="tr" @play="playShelf(s, i)" />
        </MusicShelf>

        <!-- Радар: по карточке на релиз — обложка, название, артист и вид релиза -->
        <MusicShelf v-else-if="s.kind === 'release_radar'" :title="shelfTitle(s)" :hint="t('hint.release_radar')" :cols="6" phone-width="136px">
          <MusicAlbumCard v-for="a in albums(s)" :key="a.id" :album="a" :meta="`${a.artist.name} · ${tg(`record.${a.recordType}`)}`" />
        </MusicShelf>

        <MusicShelf v-else-if="s.kind === 'new_releases'" :title="shelfTitle(s)" :cols="5">
          <MusicAlbumCard
            v-for="a in albums(s)"
            :key="a.id"
            :album="a"
            :badge="isNew(a) ? 'new' : null"
            :meta="`${a.artist.name} · ${a.releaseDate ? formatShortDate(a.releaseDate, lang) : tg(`record.${a.recordType}`)}`"
          />
        </MusicShelf>

        <section v-else-if="s.kind === 'recent_likes'" class="likes" :aria-label="shelfTitle(s)">
          <header class="likes__head">
            <h2 class="likes__title">{{ shelfTitle(s) }}</h2>
            <UiButton to="/liked" variant="ghost" size="sm" icon-right="ph:caret-right">{{ t('allLiked') }}</UiButton>
          </header>
          <div class="likes__grid">
            <MusicTrackRow v-for="(tr, i) in tracks(s).slice(0, likesShown)" :key="tr.id" :track="tr" variant="compact" @play="playLiked(tracks(s), i)" />
          </div>
        </section>

        <MusicShelf v-else-if="s.kind === 'top_artists'" :title="shelfTitle(s)" :cols="6" phone-width="120px">
          <MusicArtistCard v-for="a in artists(s)" :key="a.id" :artist="a" />
        </MusicShelf>
      </template>
      <!-- Первый вход: полкам пока не из чего собраться -->
      <UiEmptyState v-if="!home.shelves.length" icon="ph:music-notes-simple" :title="t('emptyTitle')" :text="t('emptyText')">
        <UiButton to="/search" icon="ph:magnifying-glass">{{ t('find') }}</UiButton>
        <UiButton to="/import" variant="ghost">{{ t('import') }}</UiButton>
      </UiEmptyState>
      <ShellDevMockNote :sections="['home']" what="полки главной" class="home__note" />
    </template>

    <UiEmptyState v-else-if="failed" icon="ph:cloud-warning" :title="t('failed')" :text="t('failedText')" tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="load">{{ t('retry') }}</UiButton>
    </UiEmptyState>

    <div v-else class="home__skel" aria-hidden="true">
      <UiSkeleton variant="line" width="180px" height="22px" />
      <div class="home__skel-row">
        <UiSkeleton v-for="n in 5" :key="n" height="200px" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.home { display: flex; flex-direction: column; gap: 48px; }
.phead { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 8px 16px 0; }
.phead__date { font-size: var(--fs-small); font-weight: 550; color: var(--text-3); }
.phead__hello { margin: 0; font-size: 28px; font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; }
.phead__avatar { display: grid; place-items: center; width: 44px; height: 44px; flex: none; overflow: hidden; border-radius: 50%; background: var(--fill-3); box-shadow: inset 0 0 0 1px var(--hair); font-weight: 700; }
.phead__avatar-img { width: 100%; height: 100%; object-fit: cover; }
.likes { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
.likes__head { display: flex; align-items: flex-end; justify-content: space-between; gap: 16px; }
.likes__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.2; }
.likes__grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); column-gap: 24px; }
.home__note { align-self: center; }
.home__skel { display: flex; flex-direction: column; gap: 16px; }
.home__skel-row { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 24px; }
@container main (max-width: 760px) {
  .likes__grid { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 699px) {
  .likes__grid { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 699px) {
  .home { gap: 36px; }
  .home > :deep(.hero) { margin-top: -16px; }
  .likes { padding: 0 8px; }
  .likes__head { padding: 0 8px; }
  .home__skel { padding: 0 16px; }
  .home__skel-row { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Главная",
    "greet": {
      "morning": "Доброе утро, {name}",
      "day": "Добрый день, {name}",
      "evening": "Добрый вечер, {name}",
      "night": "Доброй ночи, {name}"
    },
    "greetShort": {
      "morning": "Доброе утро",
      "day": "Добрый день",
      "evening": "Добрый вечер",
      "night": "Доброй ночи"
    },
    "recent": "Недавно понравилось",
    "playAll": "Слушать",
    "hint": {
      "discover_weekly": "Новое рядом с вашим вкусом",
      "release_radar": "Свежие релизы ваших артистов",
      "forgotten_favorites": "Любимое, которое давно не играло"
    },
    "allLiked": "Все любимые",
    "settings": "Настройки",
    "failed": "Главная не загрузилась",
    "failedText": "Проверьте соединение и попробуйте ещё раз",
    "emptyTitle": "Здесь появятся ваши подборки",
    "emptyText": "Слушайте волну и отмечайте любимое — миксы и новинки соберутся сами",
    "find": "Найти музыку",
    "import": "Перенести медиатеку",
    "retry": "Повторить"
  },
  "en": {
    "title": "Home",
    "greet": {
      "morning": "Good morning, {name}",
      "day": "Good afternoon, {name}",
      "evening": "Good evening, {name}",
      "night": "Good night, {name}"
    },
    "greetShort": {
      "morning": "Good morning",
      "day": "Good afternoon",
      "evening": "Good evening",
      "night": "Good night"
    },
    "recent": "Recently liked",
    "playAll": "Play",
    "hint": {
      "discover_weekly": "New music close to your taste",
      "release_radar": "Fresh releases from your artists",
      "forgotten_favorites": "Favorites you haven't played in a while"
    },
    "allLiked": "All liked",
    "settings": "Settings",
    "failed": "Home didn't load",
    "failedText": "Check your connection and try again",
    "emptyTitle": "Your picks will show up here",
    "emptyText": "Play the wave and like what you love — mixes and new releases will follow",
    "find": "Find music",
    "import": "Bring your library",
    "retry": "Retry"
  }
}
</i18n>
