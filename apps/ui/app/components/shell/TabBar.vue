<script setup lang="ts">
// Нижние вкладки телефона. dock — плавающая стеклянная панель планшета (рядом — карточка плеера).
// Сложенный складной iPhone (строка состояния — колонкой справа): вкладки столбиком значков внизу
// этой колонки, как у приложений Apple на нём
withDefaults(defineProps<{ dock?: boolean }>(), {})
const ui = useUiStore()
const route = useRoute()
const { t } = useI18n()

const TABS = [
  { to: '/', key: 'nav.home', icon: 'ph:house', iconOn: 'ph:house-fill' },
  { to: '/search', key: 'nav.search', icon: 'ph:magnifying-glass', iconOn: 'ph:magnifying-glass-bold' },
  { to: '/library', key: 'nav.library', icon: 'ph:books', iconOn: 'ph:books-fill' },
] as const

const isActive = (to: string) => (to === '/' ? route.path === '/' : route.path.startsWith(to) || (to === '/library' && (route.path === '/liked' || route.path.startsWith('/playlist'))))
</script>

<template>
  <nav class="tabbar" :class="{ 'tabbar--dock': dock, 'tabbar--side': !dock && ui.isSideBar }" :aria-label="t('nav.main')">
    <NuxtLink v-for="tab in TABS" :key="tab.to" :to="tab.to" class="tabbar__item" :class="{ 'is-active': isActive(tab.to) }" :aria-current="isActive(tab.to) ? 'page' : undefined" :aria-label="!dock && ui.isSideBar ? t(tab.key) : undefined">
      <Icon :name="isActive(tab.to) ? tab.iconOn : tab.icon" class="tabbar__icon" />
      <span class="tabbar__label">{{ t(tab.key) }}</span>
    </NuxtLink>
  </nav>
</template>

<style scoped>
.tabbar {
  position: fixed;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 35;
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  height: calc(var(--tabbar-h) + var(--safe-bottom));
  padding-bottom: var(--safe-bottom);
  background: color-mix(in srgb, var(--ink-1) 84%, transparent);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
  backdrop-filter: blur(30px) saturate(1.4);
  border-top: 1px solid var(--hair);
}
.tabbar__item { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; padding-top: 4px; font-size: 10.5px; font-weight: 600; color: var(--text-3); }
.tabbar__icon { font-size: 24px; }
.tabbar__item.is-active { color: var(--text-1); }
.tabbar__item:active { opacity: 0.7; }

/* Планшет: плавающая скруглённая панель над содержимым */
.tabbar--dock {
  position: relative;
  left: auto;
  right: auto;
  bottom: auto;
  z-index: auto;
  height: var(--dock-h);
  padding: 0 8px;
  border: 0;
  border-radius: 22px;
  background: var(--float-bar);
  -webkit-backdrop-filter: blur(30px) saturate(1.6);
  backdrop-filter: blur(30px) saturate(1.6);
  box-shadow: 0 0 0 1px var(--float-ring), 0 18px 36px -14px var(--float-drop);
}
.tabbar--dock .tabbar__item { gap: 3px; padding-top: 0; font-size: 11.5px; }
.tabbar--dock .tabbar__icon { font-size: 25px; }

/* Сложенный складной iPhone: столбик значков внизу колонки строки состояния справа, по её центру */
.tabbar--side {
  left: auto;
  right: calc((var(--safe-right) - 56px) / 2);
  bottom: calc(var(--safe-bottom) + 12px);
  grid-template-columns: none;
  grid-auto-rows: 56px;
  width: 56px;
  height: auto;
  padding: 4px 0;
  border: 0;
  border-radius: 28px;
  background: var(--float-bar);
  box-shadow: 0 0 0 1px var(--float-ring), 0 14px 30px -14px var(--float-drop);
}
.tabbar--side .tabbar__item { position: relative; padding-top: 0; }
.tabbar--side .tabbar__item.is-active::before { content: ''; position: absolute; inset: 4px; z-index: -1; border-radius: 50%; background: var(--fill-2); }
.tabbar--side .tabbar__item { isolation: isolate; }
.tabbar--side .tabbar__label { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
</style>
