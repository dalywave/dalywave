<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Верхняя строка: назад/вперёд, поиск (⌘K), профиль. Прозрачная над сценой, стекло при прокрутке
withDefaults(defineProps<{ scrolled?: boolean }>(), {})

const route = useRoute()
const router = useRouter()
const ui = useUiStore()
const auth = useAuthStore()
const toast = useToast()
const { t } = useI18n()

const search = ref<{ focus: () => void } | null>(null)
const q = ref(typeof route.query.q === 'string' ? route.query.q : '')
watch(() => route.query.q, (v) => {
  if (route.path === '/search') q.value = typeof v === 'string' ? v : ''
})
watch(() => ui.searchFocus, () => search.value?.focus())

let timer: ReturnType<typeof setTimeout> | undefined
function onInput(v: string) {
  clearTimeout(timer)
  timer = setTimeout(() => {
    const query = v.trim() ? { q: v.trim() } : {}
    // Фильтры поиска (?type, ?scope) переживают новый запрос
    if (route.path === '/search') void router.replace({ path: '/search', query: { ...route.query, q: v.trim() || undefined } })
    else if (v.trim()) void router.push({ path: '/search', query })
  }, 180)
}

function onSubmit(v: string) {
  clearTimeout(timer)
  void router.push({ path: '/search', query: v.trim() ? { q: v.trim() } : {} })
}

const canBack = computed(() => {
  void route.fullPath
  return import.meta.client && !!(window.history.state as { back?: string | null } | null)?.back
})
const canForward = computed(() => {
  void route.fullPath
  return import.meta.client && !!(window.history.state as { forward?: string | null } | null)?.forward
})

const initial = computed(() => (auth.user?.name ?? '?').trim().charAt(0).toUpperCase())

async function logout() {
  try {
    await auth.logout()
    await navigateTo('/login')
  }
  catch {
    toast.error(t('auth.logoutFailed'))
  }
}

const accountMenu = computed<MenuEntry[]>(() => [
  { kind: 'heading', label: auth.user?.name ?? '', hint: auth.user?.email ?? undefined },
  { label: t('nav.history'), icon: 'ph:clock-counter-clockwise', onSelect: () => navigateTo('/history') },
  { label: t('nav.settings'), icon: 'ph:gear-six', onSelect: () => navigateTo('/settings') },
  ...(auth.isAdmin ? [{ label: t('nav.admin'), icon: 'ph:shield-check', onSelect: () => navigateTo('/admin') }] : []),
  { kind: 'separator' },
  { label: t('nav.logout'), icon: 'ph:sign-out', danger: true, onSelect: logout },
])
</script>

<template>
  <header class="top" :class="{ 'is-scrolled': scrolled }" data-tauri-drag-region="deep">
    <div class="top__nav">
      <UiIconButton icon="ph:caret-left" :label="t('nav.back')" size="sm" :disabled="!canBack" side="bottom" @click="router.back()" />
      <UiIconButton icon="ph:caret-right" :label="t('nav.forward')" size="sm" :disabled="!canForward" side="bottom" @click="router.forward()" />
    </div>
    <UiSearchField ref="search" v-model="q" class="top__search" shortcut links :placeholder="t('search.placeholder')" @update:model-value="onInput" @submit="onSubmit" />
    <RecognizeButton size="sm" class="top__mic" />
    <div class="top__spacer" />
    <NotifyBell />
    <UiMenu :items="accountMenu" align="end">
      <button type="button" class="avatar" :aria-label="t('nav.profile')">
        <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="" class="avatar__img">
        <span v-else>{{ initial }}</span>
      </button>
    </UiMenu>
  </header>
</template>

<style scoped>
.top {
  position: sticky;
  top: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  gap: 12px;
  height: var(--topbar-h);
  margin-bottom: calc(var(--topbar-h) * -1);
  padding: 0 24px 0 32px;
  transition: background-color var(--dur-3), box-shadow var(--dur-3);
}
.top.is-scrolled {
  background: rgba(8, 9, 12, 0.72);
  -webkit-backdrop-filter: blur(24px) saturate(1.3);
  backdrop-filter: blur(24px) saturate(1.3);
  box-shadow: 0 1px 0 var(--hair);
}
.top__nav { display: flex; gap: 4px; }
.top__nav :deep(.ib) { background: var(--fill-1); }
.top__nav :deep(.ib:hover) { background: var(--fill-2); }
.top__nav :deep(.ib:disabled) { background: var(--fill-1); opacity: 0.6; }
.top__search { width: min(380px, 100%); flex: 0 1 380px; }
.top__mic { flex: none; background: var(--fill-1); }
.top__mic:hover { background: var(--fill-2); }
.top__spacer { flex: 1; align-self: stretch; }
.avatar {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  flex: none;
  padding: 0;
  overflow: hidden;
  border: 0;
  border-radius: 50%;
  background: var(--fill-3);
  box-shadow: inset 0 0 0 1px var(--line);
  font-size: var(--fs-small);
  font-weight: 700;
  color: #fff;
  transition: box-shadow var(--dur-2);
}
.avatar:hover, .avatar[data-state='open'] { box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.3), 0 0 0 4px var(--fill-2); }
.avatar__img { width: 100%; height: 100%; object-fit: cover; }
@media (max-width: 1023px) {
  .top { padding: 0 16px 0 20px; }
}
</style>
