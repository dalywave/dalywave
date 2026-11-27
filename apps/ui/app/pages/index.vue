<script setup lang="ts">
import type { AlbumSummary, ArtistSummary, Home, PlaylistSummary, Shelf, ShelfItem, Track } from '~/api/types'

// Главная: «Моя волна» и полки с сервера в порядке, который сервер выбрал по времени суток: продолжить,
// миксы дня, радар новинок, новинки подписок, открытия недели, недавно понравилось, забытые, ваши артисты.
// Планшет под пальцем (как Spotify на iPad): сверху аватар и приветствие, под ними плитки быстрого
// доступа 4×2 (любимые, волна, то, что слушали, миксы) — «Продолжить» уходит в них
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
const today = computed(() => new Intl.DateTimeFormat(intlLocale(lang.value, 'en-GB'), { weekday: 'long', day: 'numeric', month: 'long' }).format(new Date()))

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

// ---------- Плитки быстрого доступа (планшет) ----------
const library = useLibraryStore()
const { isContext, isPlayingContext } = usePlayActions()
type Quick = { key: string, title: string, to: string, item: ShelfItem, ctx: 'album' | 'artist' | 'playlist' | null }
const QUICK_MAX = 8
const quick = computed<Quick[]>(() => {
  const shelves = home.value?.shelves ?? []
  const pool = [
    ...shelves.filter(s => s.kind === 'continue').flatMap(s => s.items),
    ...shelves.filter(s => s.kind === 'daily_mixes').flatMap(s => s.items),
  ]
  const out: Quick[] = []
  const seen = new Set<string>()
  for (const it of pool) {
    const key = `${it.type}:${it.id}`
    if (seen.has(key) || out.length >= QUICK_MAX - 2) continue
    seen.add(key)
    if (it.type === 'album') out.push({ key, title: it.title, to: `/album/${it.id}`, item: it, ctx: 'album' })
    else if (it.type === 'artist') out.push({ key, title: it.name, to: `/artist/${it.id}`, item: it, ctx: 'artist' })
    else if (it.type === 'playlist') out.push({ key, title: it.title, to: `/playlist/${it.id}`, item: it, ctx: 'playlist' })
    else if (it.album) out.push({ key, title: it.title, to: `/album/${it.album.id}`, item: it, ctx: null })
  }
  return out
})
const waveOn = computed(() => player.waveActive && player.playing)
function onWave() {
  if (player.waveActive && player.current) return player.toggle()
  return player.startWave()
}
const pla = usePlaylistActions()
const quickMenu = ref<ReturnType<typeof pla.menuFor>>([])
function onQuickContext(target: EventTarget | null) {
  const key = (target as HTMLElement | null)?.closest<HTMLElement>('[data-quick]')?.dataset.quick
  if (key === 'liked') {
    quickMenu.value = pla.likedMenu()
    return
  }
  const q = quick.value.find(x => x.key === key)
  const p = q?.item.type === 'playlist' ? library.playlists?.find(x => x.id === q.item.id) : undefined
  quickMenu.value = p ? pla.menuFor(p) : []
}

function playLiked(list: Track[], i: number) {
  const tr = list[i]
  if (tr && player.current?.track.id === tr.id) return player.toggle()
  return player.playTracks(list, i, { type: 'library', id: null, title: t('recent') })
}
</script>

<template>
  <div class="home">
    <!-- Планшет: аватар слева вверху, как у Spotify; под ним — плитки быстрого доступа -->
    <header v-if="ui.isTouch" class="thead">
      <NuxtLink to="/settings" class="phead__avatar thead__avatar" :aria-label="t('settings')">
        <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="" class="phead__avatar-img">
        <template v-else>{{ (auth.user?.name ?? '?').charAt(0).toUpperCase() }}</template>
      </NuxtLink>
      <h1 class="thead__hello">{{ greeting }}</h1>
      <NotifyBell size="lg" class="thead__bell" />
    </header>
    <UiContextMenu v-if="ui.isTouch" :items="quickMenu" @open="onQuickContext">
      <nav class="quick" :aria-label="t('quick')">
        <MusicQuickTile data-quick="liked" :title="tg('nav.liked')" to="/liked" :current="player.context?.type === 'library'" :playing="player.playing">
          <template #art><span class="quick__liked"><Icon name="ph:heart-fill" /></span></template>
        </MusicQuickTile>
        <MusicQuickTile :title="tg('wave.title')" :current="player.waveActive" :playing="waveOn" @click="onWave">
          <template #art><span class="quick__wave"><BrandGlyph class="quick__glyph" /></span></template>
        </MusicQuickTile>
        <MusicQuickTile
          v-for="q in quick"
          :key="q.key"
          :data-quick="q.key"
          :title="q.title"
          :to="q.to"
          :current="q.ctx ? isContext(q.ctx, q.item.id) : player.current?.track.id === q.item.id"
          :playing="q.ctx ? isPlayingContext(q.ctx, q.item.id) : player.playing"
        >
          <template #art>
            <MusicPlaylistCover v-if="q.item.type === 'playlist'" :id="q.item.id" :title="q.item.title" :covers="q.item.covers" :kind="q.item.kind" :show-title="false" />
            <MusicCoverArt v-else-if="q.item.type === 'album' || q.item.type === 'artist'" :image="q.item.image" size="small" :alt="q.title" />
            <MusicCoverArt v-else :image="q.item.album?.image ?? null" size="small" :alt="q.title" />
          </template>
        </MusicQuickTile>
      </nav>
    </UiContextMenu>

    <header v-if="ui.isPhone" class="phead">
      <div>
        <div class="phead__date">{{ today.charAt(0).toUpperCase() + today.slice(1) }}</div>
        <h1 class="phead__hello">{{ greeting }}</h1>
      </div>
      <div class="phead__tools">
        <NotifyBell size="lg" />
        <NuxtLink to="/settings" class="phead__avatar" :aria-label="t('settings')">
          <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="" class="phead__avatar-img">
          <template v-else>{{ (auth.user?.name ?? '?').charAt(0).toUpperCase() }}</template>
        </NuxtLink>
      </div>
    </header>

    <!-- Итоги года открылись, а слушатель их ещё не видел — «Твои итоги готовы» первым делом -->
    <RewindCard launch />

    <MusicWaveHero :greeting="ui.isTouchUi ? undefined : greeting" />

    <!-- «Открытия»: лента клипов нового для слушателя -->
    <MusicDiscoverTile />

    <!-- Итоги года — плиткой, пока событие открыто и истории уже посмотрены -->
    <RewindCard home />

    <!-- Телефон и iPad: что слушают друзья (на компьютере — правая панель) -->
    <FriendsShelf v-if="ui.isTouchUi" />

    <template v-if="home">
      <template v-for="s in home.shelves" :key="s.id">
        <MusicShelf v-if="s.kind === 'continue' && !ui.isTouch" :title="shelfTitle(s)" :cols="3" :rows="s.items.length > 3 ? 2 : 1" phone-width="82%">
          <MusicResumeCard v-for="it in s.items" :key="`${it.type}:${it.id}`" :item="it" />
        </MusicShelf>

        <MusicShelf v-else-if="s.kind === 'daily_mixes'" :title="shelfTitle(s)" :cols="5">
          <MusicDailyMixCard v-for="(p, i) in playlists(s)" :key="p.id" :playlist="p" :index="i + 1" />
        </MusicShelf>

        <MusicShelf v-else-if="TRACK_SHELVES.has(s.kind)" :title="shelfTitle(s)" :hint="t(`hint.${s.kind}`)" :cols="6" phone-width="136px">
          <template #actions>
            <UiButton variant="ghost" size="sm" icon="ph:play-fill" @click="playShelf(s, 0)">{{ t('playAll') }}</UiButton>
          </template>
          <MusicTrackCard v-for="(tr, i) in tracks(s)" :key="tr.id" :track="tr" hook :origin="() => ({ tracks: tracks(s), index: i, context: { type: 'mix', id: s.id, title: shelfTitle(s) } })" @play="playShelf(s, i)" />
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

        <!-- Своя музыка: что недавно появилось в папках на сервере -->
        <MusicShelf v-else-if="s.kind === 'recently_added'" :title="shelfTitle(s)" :cols="6" phone-width="136px">
          <MusicAlbumCard v-for="a in albums(s)" :key="a.id" :album="a" :meta="a.releaseDate ? `${a.artist.name} · ${a.releaseDate.slice(0, 4)}` : a.artist.name" />
        </MusicShelf>

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
.phead__tools { display: flex; flex: none; align-items: center; gap: 4px; }
.thead__bell { margin-left: auto; }
.thead { display: flex; align-items: center; gap: 14px; margin-bottom: -24px; }
.thead__avatar { width: 40px; height: 40px; }
.thead__hello { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; }
/* Плитки быстрого доступа: четыре в ряд, две строки — как у Spotify на iPad */
.quick { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }
.quick__liked, .quick__wave { display: grid; place-items: center; width: 100%; height: 100%; color: #fff; }
/* Единственная плитка с градиентом по смыслу — «Любимые», как в боковой панели; волна — знак бренда */
.quick__liked { background: radial-gradient(110% 110% at 25% 15%, var(--m3), var(--m2) 45%, var(--m1)); font-size: 22px; }
.quick__wave { background: var(--brand-gradient); }
.quick__glyph { width: 28px; height: 28px; }
@container main (max-width: 760px) {
  .quick { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
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
    "quick": "Быстрый доступ",
    "failed": "Главная не загрузилась",
    "failedText": "Проверьте соединение и попробуйте ещё раз",
    "emptyTitle": "Здесь появятся ваши подборки",
    "emptyText": "Слушайте волну и отмечайте любимое — миксы и новинки соберутся сами",
    "find": "Найти музыку",
    "import": "Перенести медиатеку",
    "retry": "Повторить"
  },
  "uk": {
    "title": "Головна",
    "greet": {
      "morning": "Доброго ранку, {name}",
      "day": "Добрий день, {name}",
      "evening": "Добрий вечір, {name}",
      "night": "Доброї ночі, {name}"
    },
    "greetShort": {
      "morning": "Доброго ранку",
      "day": "Добрий день",
      "evening": "Добрий вечір",
      "night": "Доброї ночі"
    },
    "recent": "Нещодавно вподобане",
    "playAll": "Слухати",
    "hint": {
      "discover_weekly": "Нове поруч із вашим смаком",
      "release_radar": "Свіжі релізи ваших виконавців",
      "forgotten_favorites": "Улюблене, яке давно не грало"
    },
    "allLiked": "Усе вподобане",
    "settings": "Налаштування",
    "quick": "Швидкий доступ",
    "failed": "Головна не завантажилася",
    "failedText": "Перевірте з’єднання і спробуйте ще раз",
    "emptyTitle": "Тут з’являться ваші добірки",
    "emptyText": "Слухайте хвилю та позначайте улюблене — мікси й новинки зберуться самі",
    "find": "Знайти музику",
    "import": "Перенести медіатеку",
    "retry": "Повторити"
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
    "quick": "Quick access",
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
