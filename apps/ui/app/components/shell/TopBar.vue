<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Верхняя строка: назад/вперёд, поиск (⌘K), профиль. Прозрачная над сценой, стекло при прокрутке
withDefaults(defineProps<{ scrolled?: boolean }>(), {})

const route = useRoute()
const router = useRouter()
const ui = useUiStore()
const auth = useAuthStore()
const bridge = useBridge()
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

// Друзья: что слушают — правая панель; точка — ждут ответа запросы в друзья
const friends = useFriendsStore()
const priv = usePrivateSession()
const friendsOpen = computed(() => ui.panelOpen && ui.panelTab === 'friends')

async function logout() {
  try {
    await auth.logout()
    await navigateTo('/login')
  }
  catch {
    toast.error(t('auth.logoutFailed'))
  }
}

// Установщики Mac и Windows — только на сайте: в приложениях есть автообновление
const appsOpen = ref(false)
const desktopApps: MenuEntry[] = bridge.kind === 'web' ? [{ label: t('nav.desktopApps'), icon: 'ph:desktop', onSelect: () => (appsOpen.value = true) }] : []

const accountMenu = computed<MenuEntry[]>(() => [
  { kind: 'heading', label: auth.user?.name ?? '', hint: auth.user?.email ?? undefined },
  priv.entry(),
  { label: t('friends.title'), icon: 'ph:users-three', onSelect: () => navigateTo('/friends') },
  { kind: 'separator' },
  { label: t('nav.history'), icon: 'ph:clock-counter-clockwise', onSelect: () => navigateTo('/history') },
  { label: t('nav.settings'), icon: 'ph:gear-six', onSelect: () => navigateTo('/settings') },
  ...desktopApps,
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
    <UiTooltip :text="t('friends.panel')" side="bottom">
      <button type="button" class="friends" :class="{ 'is-open': friendsOpen }" :aria-label="t('friends.panel')" :aria-pressed="friendsOpen" @click="ui.togglePanel('friends')">
        <Icon name="ph:users-three" class="friends__icon" />
        <span v-if="friends.incoming.length" class="friends__dot" aria-hidden="true" />
      </button>
    </UiTooltip>
    <UiMenu :items="accountMenu" align="end">
      <button type="button" class="avatar" :class="{ 'is-private': priv.on.value }" :aria-label="priv.on.value ? `${t('nav.profile')} · ${t('friends.private')}` : t('nav.profile')">
        <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="" class="avatar__img">
        <span v-else>{{ initial }}</span>
        <span v-if="priv.on.value" class="avatar__private" aria-hidden="true"><Icon name="ph:eye-slash-bold" /></span>
      </button>
    </UiMenu>
    <ShellDesktopApps v-if="desktopApps.length" v-model:open="appsOpen" />
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
  background: color-mix(in srgb, var(--ink-1) 72%, transparent);
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
  color: var(--text-1);
  transition: box-shadow var(--dur-2);
}
.avatar:hover, .avatar[data-state='open'] { box-shadow: inset 0 0 0 1px rgba(var(--hi-rgb), 0.3), 0 0 0 4px var(--fill-2); }
.avatar__img { width: 100%; height: 100%; object-fit: cover; }
/* Приватная сессия: значок на аватаре, чтобы не забыть, что друзья ничего не видят */
.avatar { position: relative; overflow: visible; }
.avatar__img { border-radius: 50%; }
.avatar__private { position: absolute; right: -4px; bottom: -4px; display: grid; place-items: center; width: 17px; height: 17px; border-radius: 50%; background: var(--text-1); box-shadow: 0 0 0 2px var(--ink-1); color: var(--on-light); font-size: 10px; }
/* Как колокольчик рядом: тот же круг и тот же вес значка */
.friends { position: relative; display: grid; place-items: center; width: 32px; height: 32px; flex: none; padding: 0; border: 0; border-radius: 50%; background: var(--fill-1); color: var(--text-2); transition: background-color var(--dur-1), color var(--dur-1); }
.friends:hover, .friends.is-open { background: var(--fill-2); color: var(--text-1); }
.friends:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.friends__icon { font-size: 18px; }
.friends__dot { position: absolute; top: 3px; right: 3px; width: 8px; height: 8px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 2px var(--ink-1); }
@media (max-width: 1023px) {
  .top { padding: 0 16px 0 20px; }
}
</style>
