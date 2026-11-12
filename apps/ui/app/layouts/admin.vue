<script setup lang="ts">
// Админка: своя оболочка с разделами-страницами. На компьютере — колонка разделов слева,
// на телефоне — строка вкладок с прокруткой под заголовком. Музыка, если играла, продолжает играть
const route = useRoute()
const ui = useUiStore()
const { t } = useI18n()
const { counts, refresh } = useAdminCounts()

interface NavItem { to: string, key: string, icon: string, count?: 'disputed' | 'waiting' }
const GROUPS: { key: string | null, items: NavItem[] }[] = [
  { key: null, items: [{ to: '/admin', key: 'overview', icon: 'ph:gauge' }] },
  { key: 'groupWork', items: [
    { to: '/admin/downloads', key: 'downloads', icon: 'ph:download-simple' },
    { to: '/admin/disputed', key: 'disputed', icon: 'ph:scales', count: 'disputed' },
    { to: '/admin/sources', key: 'sources', icon: 'ph:plugs-connected' },
  ] },
  { key: 'groupMusic', items: [
    { to: '/admin/catalog', key: 'catalog', icon: 'ph:vinyl-record' },
    { to: '/admin/storage', key: 'storage', icon: 'ph:hard-drives' },
    { to: '/admin/reco', key: 'reco', icon: 'ph:sparkle' },
    { to: '/admin/import', key: 'import', icon: 'ph:tray-arrow-down' },
  ] },
  { key: 'groupSystem', items: [
    { to: '/admin/access', key: 'access', icon: 'ph:users-three', count: 'waiting' },
    { to: '/admin/settings', key: 'settings', icon: 'ph:sliders-horizontal' },
    { to: '/admin/log', key: 'log', icon: 'ph:scroll' },
  ] },
]
const ITEMS = GROUPS.flatMap(g => g.items)

const current = computed(() => {
  const path = route.path.replace(/\/+$/, '') || '/admin'
  return ITEMS.find(i => i.to === path)?.to ?? '/admin'
})
const countOf = (i: NavItem) => (i.count ? counts.value[i.count] : 0)

onMounted(() => void refresh())
// Счётчики живут вместе с данными: спорные появляются по событиям скачивания
const refreshSoon = useDebounced(() => void refresh(), 1500)
useAdminLive({ 'acquisition.updated': refreshSoon }, refreshSoon)

// Новая страница — наверх; вкладка выбранного раздела на телефоне — в видимую часть строки
const main = ref<HTMLElement | null>(null)
const tabs = ref<HTMLElement | null>(null)
function revealTab() {
  const el = tabs.value?.querySelector<HTMLElement>('.is-active')
  el?.scrollIntoView({ block: 'nearest', inline: 'center', behavior: 'instant' as ScrollBehavior })
}
watch(() => route.path, () => {
  main.value?.scrollTo({ top: 0 })
  if (ui.isPhone) window.scrollTo({ top: 0 })
  void nextTick(revealTab)
})
onMounted(() => void nextTick(revealTab))
</script>

<template>
  <div class="adm">
    <aside v-if="!ui.isPhone" class="adm__side thin-scrollbar" :aria-label="t('admin.title')">
      <div class="adm__brand">
        <BrandLogo />
        <span class="adm__brand-sub">{{ t('admin.title') }}</span>
      </div>
      <nav class="adm__nav" :aria-label="t('admin.sections')">
        <div v-for="g in GROUPS" :key="g.key ?? 'top'" class="adm__group">
          <span v-if="g.key" class="adm__group-title">{{ t(`admin.nav.${g.key}`) }}</span>
          <NuxtLink
            v-for="s in g.items"
            :key="s.to"
            :to="s.to"
            class="adm__link"
            :class="{ 'is-active': current === s.to }"
            :aria-current="current === s.to ? 'page' : undefined"
          >
            <Icon :name="s.icon" />
            <span class="adm__label">{{ t(`admin.nav.${s.key}`) }}</span>
            <span v-if="countOf(s)" class="adm__count tnum">{{ countOf(s) }}</span>
          </NuxtLink>
        </div>
      </nav>
      <div class="adm__foot">
        <UiButton to="/" variant="ghost" icon="ph:arrow-left" block>{{ t('admin.back') }}</UiButton>
      </div>
    </aside>
    <main ref="main" class="adm__main thin-scrollbar">
      <header v-if="ui.isPhone" class="adm__phone">
        <div class="adm__phone-bar">
          <UiIconButton to="/" icon="ph:arrow-left" :label="t('admin.back')" size="lg" :tooltip="false" />
          <span class="adm__phone-title">{{ t('admin.title') }}</span>
        </div>
        <nav ref="tabs" class="adm__tabs" :aria-label="t('admin.sections')">
          <NuxtLink
            v-for="s in ITEMS"
            :key="s.to"
            :to="s.to"
            class="adm__tab"
            :class="{ 'is-active': current === s.to }"
            :aria-current="current === s.to ? 'page' : undefined"
          >
            {{ t(`admin.nav.${s.key}`) }}<span v-if="countOf(s)" class="adm__count tnum">{{ countOf(s) }}</span>
          </NuxtLink>
        </nav>
      </header>
      <slot />
    </main>
  </div>
</template>

<style scoped>
.adm { position: relative; z-index: 1; display: grid; background: var(--ink-1); grid-template-columns: 232px minmax(0, 1fr); height: 100vh; height: 100dvh; }
.adm__side { display: flex; flex-direction: column; gap: 16px; overflow-y: auto; padding: 22px 12px 12px; border-right: 1px solid var(--hair); }
.adm__brand { display: flex; align-items: center; gap: 10px; padding: 0 8px 4px; }
.adm__brand-sub { font-size: var(--fs-small); font-weight: 600; color: var(--text-3); }
.adm__nav { display: flex; flex: 1; flex-direction: column; gap: 16px; }
.adm__group { display: flex; flex-direction: column; gap: 2px; }
.adm__group-title { padding: 0 12px 6px; font-size: 11px; font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.adm__link { display: flex; align-items: center; gap: 12px; height: 36px; padding: 0 10px 0 12px; border-radius: var(--r-3); font-weight: 550; color: var(--text-2); transition: color var(--dur-1), background-color var(--dur-1); }
.adm__link :deep(.iconify) { flex: none; font-size: 19px; }
.adm__label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.adm__link:hover { color: var(--text-1); background: var(--fill-1); }
.adm__link:active { background: var(--fill-2); }
.adm__link:focus-visible, .adm__tab:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.adm__link.is-active { color: var(--text-1); background: var(--fill-2); }
.adm__count { display: inline-grid; place-items: center; min-width: 20px; height: 20px; padding: 0 6px; border-radius: var(--r-pill); background: color-mix(in oklab, var(--warning) 22%, transparent); font-size: 11px; font-weight: 700; color: var(--warning); }
.adm__main { min-width: 0; overflow-y: auto; }
.adm__phone { position: sticky; top: 0; z-index: 5; background: var(--ink-1); border-bottom: 1px solid var(--hair); }
.adm__phone-bar { display: flex; align-items: center; gap: 8px; padding: calc(var(--safe-top) + 4px) 8px 0; }
.adm__phone-title { font-size: var(--fs-h4); font-weight: 700; }
.adm__tabs { display: flex; gap: 4px; overflow-x: auto; padding: 4px 12px 8px; scrollbar-width: none; scroll-padding-inline: 12px; }
.adm__tabs::-webkit-scrollbar { display: none; }
.adm__tab { position: relative; display: inline-flex; flex: none; align-items: center; gap: 6px; height: 36px; padding: 0 14px; border-radius: var(--r-pill); font-size: var(--fs-small); font-weight: 600; color: var(--text-2); white-space: nowrap; transition: color var(--dur-1), background-color var(--dur-1); -webkit-tap-highlight-color: transparent; }
.adm__tab::before { content: ''; position: absolute; inset: -4px -2px; }
.adm__tab:active { background: var(--fill-2); }
.adm__tab.is-active { color: var(--ink-1, #08090c); background: var(--text-1); }
.adm__tab.is-active .adm__count { background: rgba(0, 0, 0, 0.12); color: inherit; }
@media (max-width: 699px) {
  .adm { display: block; height: auto; }
  .adm__main { overflow: visible; }
}
</style>

<!-- Общие классы страниц админки: заголовок, разделы, простые таблицы с волосками, «подпись — значение» -->
<style>
.ap { display: flex; flex-direction: column; gap: 40px; max-width: 1180px; padding: 32px 40px 96px; }
.ap-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px 16px; }
.ap-title { margin: 0; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.1; }
.ap-head__side { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.ap-sec { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
.ap-sec__head { display: flex; flex-wrap: wrap; align-items: flex-end; justify-content: space-between; gap: 8px 16px; }
.ap-sec__titles { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.ap-h2 { display: flex; align-items: center; gap: 10px; margin: 0; font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; }
.ap-hint { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.ap-tools { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }

/* Таблица: строки-сетки с волосками; колонки задаются --cols у .ap-table */
.ap-table { display: flex; flex-direction: column; min-width: 0; }
.ap-tr { display: grid; grid-template-columns: var(--cols); align-items: center; gap: 16px; min-height: 52px; padding: 8px 0; border-top: 1px solid var(--hair); font-size: var(--fs-small); }
.ap-tr:first-child { border-top: 0; }
.ap-tr--head { min-height: 32px; padding: 0 0 6px; border-bottom: 1px solid var(--hair); font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); }
.ap-tr--head + .ap-tr { border-top: 0; }
.ap-tr--btn { width: 100%; border-left: 0; border-right: 0; border-bottom: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; transition: background-color var(--dur-1); }
.ap-tr--btn:hover { background: var(--fill-1); }
.ap-tr--btn:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.ap-cell { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.ap-cell b { overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.ap-cell small, .ap-sub { overflow: hidden; font-size: var(--fs-caption); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
.ap-num { font-variant-numeric: tabular-nums; text-align: right; justify-self: end; }
.ap-end { display: flex; justify-content: flex-end; gap: 6px; }
.ap-mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11.5px; color: var(--text-3); }
.ap-muted { color: var(--text-3); }
.ap-on { color: var(--accent); font-weight: 600; }
.ap-ok { color: var(--success); }
.ap-warn { color: var(--warning); }
.ap-bad { color: var(--danger); }
.ap-more { display: flex; justify-content: center; padding-top: 8px; border-top: 1px solid var(--hair); }

/* «Подпись — значение» с волосками */
.ap-kv { display: flex; flex-direction: column; margin: 0; }
.ap-kv > div { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; min-height: 44px; padding: 10px 0; border-top: 1px solid var(--hair); font-size: var(--fs-small); }
.ap-kv > div:first-child { border-top: 0; }
.ap-kv dt { min-width: 0; color: var(--text-2); }
.ap-kv dd { margin: 0; font-weight: 600; font-variant-numeric: tabular-nums; text-align: right; }

/* Крупные цифры в строку: без карточек, разделены волосками */
.ap-stats { display: grid; grid-template-columns: repeat(var(--n, 6), minmax(0, 1fr)); }
.ap-stat { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding: 4px 16px; border-left: 1px solid var(--hair); }
.ap-stat:first-child { padding-left: 0; border-left: 0; }
.ap-stat__value { font-size: var(--fs-h2); font-weight: 700; letter-spacing: var(--tr-title); line-height: 1.15; font-variant-numeric: tabular-nums; }
.ap-stat__label { overflow: hidden; font-size: var(--fs-small); color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.ap-stat__extra { font-size: var(--fs-caption); color: var(--text-3); }
.ap-stat.is-bad .ap-stat__value { color: var(--danger); }
.ap-stat.is-warn .ap-stat__value { color: var(--warning); }

/* Две колонки разделов на широком экране */
.ap-cols { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 40px 48px; }

/* Точка состояния: плоская, без свечения — цвет и есть сигнал */
.ap-dot { display: inline-block; width: 8px; height: 8px; flex: none; border-radius: 50%; background: var(--text-4); }
.ap-dot.is-ok { background: var(--success); }
.ap-dot.is-warn { background: var(--warning); }
.ap-dot.is-bad { background: var(--danger); }
.ap-dot.is-on { background: var(--accent); }
.ap-dot.is-pulse { animation: ap-pulse 1.6s var(--ease-in-out) infinite; }
@keyframes ap-pulse { 50% { opacity: 0.35; } }
@media (prefers-reduced-motion: reduce) { .ap-dot.is-pulse { animation: none; } }

/* Строка фильтров над таблицей страницы */
.ap-filters { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-bottom: -16px; }

/* Подпись группы строк внутри таблицы (день в журнале) */
.ap-group { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; padding: 20px 0 6px; border-bottom: 1px solid var(--hair); font-size: var(--fs-caption); font-weight: 650; color: var(--text-2); }
.ap-group:first-child { padding-top: 0; }
.ap-group + .ap-tr { border-top: 0; }

/* Поле ввода админки */
.ap-input { height: 36px; min-width: 0; padding: 0 12px; border: 0; border-radius: var(--r-3); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; font-size: var(--fs-small); transition: box-shadow var(--dur-1); }
.ap-input::placeholder { color: var(--text-3); }
.ap-input:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.ap-input:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }

@media (max-width: 1099px) {
  .ap-cols { grid-template-columns: minmax(0, 1fr); }
}
/* Под палец: маленькие кнопки в строках таблиц ловят касание на 44 pt, не меняя вида */
@media (pointer: coarse) {
  .ap .ib--sm::before, .ap .btn--sm::before { content: ''; position: absolute; inset: -6px; }
  .ap .btn--sm { position: relative; }
}
@media (max-width: 699px) {
  .ap { gap: 32px; padding: 20px 16px 72px; }
  .ap-title { font-size: 28px; }
  .ap-stats { grid-template-columns: repeat(2, minmax(0, 1fr)); row-gap: 16px; }
  .ap-stat:nth-child(odd) { padding-left: 0; border-left: 0; }
  .ap-stat__label { white-space: normal; }
  .ap-tr { gap: 10px; }
  .ap-hide-phone { display: none !important; }
  /* Фильтры раздела на телефоне — строкой с прокруткой, а не шире экрана */
  .ap-tools { flex-wrap: nowrap; max-width: 100%; overflow-x: auto; scrollbar-width: none; }
  .ap-tools::-webkit-scrollbar { display: none; }
  .ap-tools > * { flex: none; }
  .ap-filters { margin-bottom: -8px; }
  .ap-filters > .asl { flex: 1 1 140px; }
}
</style>
