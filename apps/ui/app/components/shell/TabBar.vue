<script setup lang="ts">
// Нижние вкладки телефона. dock — плавающая стеклянная панель планшета (рядом — карточка плеера)
withDefaults(defineProps<{ dock?: boolean }>(), {})
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
  <nav class="tabbar" :class="{ 'tabbar--dock': dock }" :aria-label="t('nav.main')">
    <NuxtLink v-for="tab in TABS" :key="tab.to" :to="tab.to" class="tabbar__item" :class="{ 'is-active': isActive(tab.to) }" :aria-current="isActive(tab.to) ? 'page' : undefined">
      <Icon :name="isActive(tab.to) ? tab.iconOn : tab.icon" class="tabbar__icon" />
      <span>{{ t(tab.key) }}</span>
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
  background: rgba(8, 9, 12, 0.84);
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
  background: rgba(22, 24, 31, 0.72);
  -webkit-backdrop-filter: blur(30px) saturate(1.6);
  backdrop-filter: blur(30px) saturate(1.6);
  box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.07), 0 18px 36px -14px rgba(0, 0, 0, 0.9);
}
.tabbar--dock .tabbar__item { gap: 3px; padding-top: 0; font-size: 11.5px; }
.tabbar--dock .tabbar__icon { font-size: 25px; }
</style>
