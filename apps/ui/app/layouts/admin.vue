<script setup lang="ts">
// Админка: своя оболочка с разделами; музыка, если играла, продолжает играть
const route = useRoute()
const ui = useUiStore()
const { t } = useI18n()

const SECTIONS = [
  { hash: '#overview', key: 'admin.nav.overview', icon: 'ph:gauge' },
  { hash: '#lanes', key: 'admin.nav.lanes', icon: 'ph:stack' },
  { hash: '#jobs', key: 'admin.nav.jobs', icon: 'ph:list-checks' },
  { hash: '#access', key: 'admin.nav.access', icon: 'ph:users-three' },
  { hash: '#sessions', key: 'admin.nav.sessions', icon: 'ph:devices' },
  { hash: '#audit', key: 'admin.nav.audit', icon: 'ph:scroll' },
] as const

const active = computed(() => route.hash || '#overview')
</script>

<template>
  <div class="adm">
    <aside v-if="!ui.isPhone" class="adm__side" :aria-label="t('admin.title')">
      <div class="adm__brand">
        <BrandLogo />
        <UiBadge variant="meta">{{ t('admin.title') }}</UiBadge>
      </div>
      <nav class="adm__nav" :aria-label="t('admin.title')">
        <NuxtLink v-for="s in SECTIONS" :key="s.hash" :to="{ path: '/admin', hash: s.hash }" class="adm__link" :class="{ 'is-active': active === s.hash }">
          <Icon :name="s.icon" />
          <span>{{ t(s.key) }}</span>
        </NuxtLink>
      </nav>
      <div class="adm__foot">
        <UiButton to="/" variant="ghost" icon="ph:arrow-left" block>{{ t('admin.back') }}</UiButton>
      </div>
    </aside>
    <main class="adm__main thin-scrollbar">
      <header v-if="ui.isPhone" class="adm__phone">
        <UiIconButton to="/" icon="ph:arrow-left" :label="t('admin.back')" size="lg" :tooltip="false" />
        <span class="adm__phone-title">{{ t('admin.title') }}</span>
      </header>
      <slot />
    </main>
  </div>
</template>

<style scoped>
.adm { position: relative; z-index: 1; display: grid; grid-template-columns: 240px minmax(0, 1fr); height: 100vh; height: 100dvh; }
.adm__side { display: flex; flex-direction: column; gap: 20px; padding: 22px 12px 12px; background: rgba(10, 11, 15, 0.62); -webkit-backdrop-filter: blur(30px); backdrop-filter: blur(30px); border-right: 1px solid var(--hair); }
.adm__brand { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 0 8px; }
.adm__nav { display: flex; flex: 1; flex-direction: column; gap: 2px; }
.adm__link { display: flex; align-items: center; gap: 12px; height: 38px; padding: 0 12px; border-radius: var(--r-3); font-weight: 550; color: var(--text-2); transition: color var(--dur-1), background-color var(--dur-1); }
.adm__link :deep(svg) { font-size: 20px; }
.adm__link:hover { color: var(--text-1); background: var(--fill-1); }
.adm__link.is-active { color: var(--text-1); background: var(--fill-2); }
.adm__main { min-width: 0; overflow-y: auto; scroll-behavior: smooth; scroll-padding-top: 16px; }
.adm__phone { position: sticky; top: 0; z-index: 5; display: flex; align-items: center; gap: 8px; padding: calc(var(--safe-top) + 4px) 8px 4px; background: rgba(8, 9, 12, 0.82); -webkit-backdrop-filter: blur(24px); backdrop-filter: blur(24px); border-bottom: 1px solid var(--hair); }
.adm__phone-title { font-size: var(--fs-h4); font-weight: 700; }
@media (max-width: 699px) {
  .adm { display: block; height: auto; }
  .adm__main { overflow: visible; }
}
</style>
