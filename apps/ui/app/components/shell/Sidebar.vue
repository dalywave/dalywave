<script setup lang="ts">
// Боковая панель: знак, разделы, закреплённое, подписки и состояние загрузки подписок.
// rail — узкая полоса значков на планшете
withDefaults(defineProps<{ rail?: boolean }>(), {})

const route = useRoute()
const library = useLibraryStore()
const player = usePlayerStore()
const toast = useToast()
const lang = useLang()
const { t } = useI18n()

const NAV = [
  { to: '/', key: 'nav.home', icon: 'ph:house', iconOn: 'ph:house-fill', kbd: null },
  { to: '/search', key: 'nav.search', icon: 'ph:magnifying-glass', iconOn: 'ph:magnifying-glass-bold', kbd: ['⌘', 'K'] },
  { to: '/library', key: 'nav.library', icon: 'ph:books', iconOn: 'ph:books-fill', kbd: null },
] as const

const isActive = (to: string) => (to === '/' ? route.path === '/' : route.path.startsWith(to))

onMounted(() => {
  void library.loadOverview().catch(() => {})
  void library.loadPlaylists().catch(() => {})
})

const playlists = computed(() => library.playlists ?? library.overview?.pinnedPlaylists ?? [])
const followed = computed(() => library.overview?.followedArtists ?? [])
const loaded = computed(() => !!library.overview)

// Загрузка подписок: сколько треков артистов уже на сервере
const sync = computed(() => {
  const list = followed.value
  const total = list.reduce((s, f) => s + f.tracksTotal, 0)
  const done = list.reduce((s, f) => s + Math.min(f.tracksOnServer, f.tracksTotal), 0)
  const pending = list.filter(f => f.tracksOnServer < f.tracksTotal).map(f => f.artist.name)
  return { total, done, pending, complete: !pending.length }
})

function newPlaylist() {
  toast.show({ text: t('nav.newPlaylistSoon'), icon: 'ph:playlist' })
}
</script>

<template>
  <aside class="side" :class="{ 'side--rail': rail }" :aria-label="t('nav.aside')">
    <div class="side__brand"><BrandLogo :compact="rail" /></div>

    <nav class="side__nav" :aria-label="t('nav.main')">
      <UiTooltip v-for="n in NAV" :key="n.to" :text="rail ? t(n.key) : undefined" side="right" :kbd="n.kbd ? [...n.kbd] : undefined">
        <NuxtLink :to="n.to" class="nav" :class="{ 'is-active': isActive(n.to) }" :aria-current="isActive(n.to) ? 'page' : undefined">
          <Icon :name="isActive(n.to) ? n.iconOn : n.icon" class="nav__icon" />
          <span class="nav__label">{{ t(n.key) }}</span>
          <UiKbd v-if="n.kbd && !rail" :keys="[...n.kbd]" class="nav__kbd" />
        </NuxtLink>
      </UiTooltip>
    </nav>

    <div class="side__scroll no-scrollbar">
      <section class="sec">
        <header class="sec__head">
          <span class="sec__title">{{ t('nav.pinned') }}</span>
          <UiIconButton v-if="!rail" icon="ph:plus" :label="t('nav.newPlaylist')" size="xs" @click="newPlaylist" />
        </header>
        <UiTooltip :text="rail ? t('nav.liked') : undefined" side="right">
          <NuxtLink to="/liked" class="item" :class="{ 'is-active': route.path === '/liked', 'is-current': player.context?.type === 'library' }">
            <span class="item__art item__art--liked"><Icon name="ph:heart-fill" /></span>
            <span class="item__text">
              <span class="item__title">{{ t('nav.liked') }}</span>
              <span class="item__meta"><Icon name="ph:push-pin-fill" class="item__pin" />{{ formatCount(library.likedCount, 'track', lang) }}</span>
            </span>
          </NuxtLink>
        </UiTooltip>
        <template v-if="loaded || playlists.length">
          <UiTooltip v-for="p in playlists" :key="p.id" :text="rail ? p.title : undefined" side="right">
            <NuxtLink :to="`/playlist/${p.id}`" class="item" :class="{ 'is-active': route.path === `/playlist/${p.id}`, 'is-current': player.context?.type === 'playlist' && player.context.id === p.id }">
              <MusicPlaylistCover :id="p.id" :title="p.title" :covers="p.covers" :kind="p.kind" :show-title="false" class="item__art" />
              <span class="item__text">
                <span class="item__title">{{ p.title }}</span>
                <span class="item__meta"><Icon v-if="p.pinned" name="ph:push-pin-fill" class="item__pin" />{{ t('record.playlist') }} · {{ p.tracksCount }}</span>
              </span>
            </NuxtLink>
          </UiTooltip>
        </template>
        <template v-else>
          <div v-for="n in 3" :key="n" class="item item--skel"><UiSkeleton width="36px" height="36px" /><span class="item__text"><UiSkeleton variant="line" width="70%" /><UiSkeleton variant="line" width="40%" /></span></div>
        </template>
      </section>

      <section v-if="followed.length" class="sec">
        <header class="sec__head"><span class="sec__title">{{ t('nav.artists') }}</span></header>
        <UiTooltip v-for="f in followed" :key="f.artist.id" :text="rail ? f.artist.name : undefined" side="right">
          <NuxtLink :to="`/artist/${f.artist.id}`" class="item item--artist" :class="{ 'is-active': route.path === `/artist/${f.artist.id}` }">
            <MusicCoverArt :image="f.artist.image" size="small" round :alt="f.artist.name" class="item__art" fallback-icon="ph:user" />
            <span class="item__text"><span class="item__title">{{ f.artist.name }}</span></span>
            <UiTooltip v-if="f.tracksOnServer < f.tracksTotal && !rail" :text="t('nav.downloading', { done: f.tracksOnServer, total: f.tracksTotal })" side="right">
              <span class="item__dl" role="img" :aria-label="t('nav.downloading', { done: f.tracksOnServer, total: f.tracksTotal })" />
            </UiTooltip>
          </NuxtLink>
        </UiTooltip>
      </section>
    </div>

    <footer v-if="followed.length" class="side__foot">
      <UiTooltip :text="rail ? (sync.complete ? t('nav.syncDone') : t('nav.syncTitle')) : undefined" side="right">
        <NuxtLink to="/library?tab=artists" class="sync">
          <UiProgressRing :state="sync.complete ? 'done' : sync.done === 0 ? 'queued' : 'progress'" :value="sync.total ? sync.done / sync.total : 1" :size="28" />
          <span class="sync__text">
            <span class="sync__title">{{ sync.complete ? t('nav.syncDone') : t('nav.syncTitle') }}</span>
            <span class="sync__meta">{{ sync.complete ? formatCount(library.overview?.counts.readyTracks ?? sync.done, 'track', lang) : `${sync.done} ${t('nav.of')} ${sync.total} · ${sync.pending.slice(0, 2).join(', ')}` }}</span>
          </span>
        </NuxtLink>
      </UiTooltip>
    </footer>
  </aside>
</template>

<style scoped>
.side {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  padding-top: var(--titlebar-inset, 0px);
  background: rgba(10, 11, 15, 0.62);
  -webkit-backdrop-filter: blur(30px) saturate(1.3);
  backdrop-filter: blur(30px) saturate(1.3);
  border-right: 1px solid var(--hair);
}
.side__brand { padding: calc(22px - min(var(--titlebar-inset, 0px), 18px)) 20px 18px; }
.side__nav { display: flex; flex-direction: column; gap: 2px; padding: 0 12px; }
.nav { display: flex; align-items: center; gap: 12px; height: 38px; padding: 0 12px; border-radius: var(--r-3); font-weight: 550; color: var(--text-2); transition: color var(--dur-1), background-color var(--dur-1); }
.nav__icon { flex: none; font-size: 20px; }
.nav:hover { color: var(--text-1); background: var(--fill-1); }
.nav.is-active { color: var(--text-1); background: var(--fill-2); }
.nav__kbd { margin-left: auto; opacity: 0; transition: opacity var(--dur-1); }
.nav:hover .nav__kbd { opacity: 1; }
.side__scroll { flex: 1; min-height: 0; overflow-y: auto; padding: 20px 12px 12px; }
.sec + .sec { margin-top: 20px; }
.sec__head { display: flex; align-items: center; justify-content: space-between; height: 28px; padding: 0 4px 0 12px; }
.sec__title { font-size: var(--fs-micro); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.item { display: flex; align-items: center; gap: 12px; min-width: 0; min-height: 48px; padding: 6px 8px; border-radius: var(--r-3); transition: background-color var(--dur-1); }
.item:hover, .item.is-active { background: var(--fill-1); }
.item__art { width: 36px; height: 36px; flex: none; border-radius: 8px; overflow: hidden; }
.item__art--liked { display: grid; place-items: center; background: radial-gradient(110% 110% at 25% 15%, var(--m3), var(--m2) 45%, var(--m1)); color: #fff; font-size: 16px; }
.item__text { display: flex; flex: 1; flex-direction: column; gap: 4px; min-width: 0; }
.item__title { overflow: hidden; font-weight: 550; text-overflow: ellipsis; white-space: nowrap; }
.item__text:has(.item__meta) { gap: 0; }
.item__meta { display: flex; align-items: center; gap: 4px; overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); white-space: nowrap; }
.item__pin { flex: none; font-size: 12px; color: var(--accent); }
.item.is-current .item__title { color: var(--accent); }
.item--artist { min-height: 40px; padding-top: 4px; padding-bottom: 4px; }
.item--artist .item__art { width: 28px; height: 28px; margin: 0 4px; border-radius: 50%; }
.item--skel { pointer-events: none; }
.item__dl { width: 6px; height: 6px; flex: none; margin-right: 6px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 8px var(--accent); }
.side__foot { padding: 12px; border-top: 1px solid var(--hair); }
.sync { display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px 10px; border-radius: var(--r-3); text-align: left; transition: background-color var(--dur-1); }
.sync:hover { background: var(--fill-1); }
.sync__text { display: flex; flex: 1; flex-direction: column; min-width: 0; }
.sync__title { font-size: var(--fs-small); font-weight: 600; white-space: nowrap; }
.sync__meta { overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }

/* Узкая полоса значков */
.side--rail .side__brand { display: flex; justify-content: center; padding: calc(22px - min(var(--titlebar-inset, 0px), 18px)) 0 18px; }
.side--rail .side__nav { padding: 0 10px; }
.side--rail .nav { justify-content: center; padding: 0; height: 44px; }
.side--rail .nav__label, .side--rail .nav__kbd { display: none; }
.side--rail .side__scroll { padding: 16px 10px 12px; }
.side--rail .sec__head { justify-content: center; padding: 0; }
.side--rail .sec__title { font-size: 9.5px; letter-spacing: 0.06em; }
.side--rail .item { justify-content: center; padding: 6px 0; }
.side--rail .item__text { display: none; }
.side--rail .item--artist .item__art { width: 32px; height: 32px; margin: 0; }
.side--rail .side__foot { padding: 10px; }
.side--rail .sync { justify-content: center; padding: 8px 0; }
.side--rail .sync__text { display: none; }
</style>
