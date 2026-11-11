import { defineStore } from 'pinia'
import type { AuthProviders, Me, Permission } from '~/api/types'
import { ApiError } from '~/api/types'

// Подсказка «вход уже был»: без неё публичные экраны не спрашивают сервер о сессии впустую
const HINT_KEY = 'dw.session.hint'

export type AuthStatus = 'unknown' | 'anonymous' | 'ready' | 'no-access' | 'offline'

export const useAuthStore = defineStore('auth', () => {
  const bridge = useBridge()
  const me = ref<Me | null>(null)
  const status = ref<AuthStatus>('unknown')
  const providers = ref<AuthProviders | null>(null)
  let loading: Promise<void> | null = null

  const user = computed(() => me.value?.user ?? null)
  const roles = computed(() => me.value?.roles ?? [])
  const isAdmin = computed(() => me.value?.permissions.includes('admin') ?? false)

  function can(p: Permission): boolean {
    return me.value?.permissions.includes(p) ?? false
  }

  function hint(on: boolean) {
    try {
      if (on) localStorage.setItem(HINT_KEY, '1')
      else localStorage.removeItem(HINT_KEY)
    }
    catch {
      // Без хранилища просто спросим сервер ещё раз
    }
  }

  function hasHint(): boolean {
    try {
      return localStorage.getItem(HINT_KEY) === '1'
    }
    catch {
      return true
    }
  }

  function apply(next: Me | null) {
    me.value = next
    if (!next) status.value = 'anonymous'
    else status.value = next.roles.length ? 'ready' : 'no-access'
    hint(!!next)
  }

  /** Узнать, кто вошёл. На публичных экранах без подсказки сервер не трогаем */
  function load(opts: { force?: boolean, quiet?: boolean } = {}): Promise<void> {
    if (status.value !== 'unknown' && !opts.force) return Promise.resolve()
    if (opts.quiet && !hasHint()) {
      status.value = 'anonymous'
      return Promise.resolve()
    }
    loading ??= bridge.auth.me()
      .then(apply)
      .catch(() => {
        // Нет связи: если вход уже был, показываем интерфейс, данные подтянутся позже
        status.value = hasHint() ? 'offline' : 'anonymous'
      })
      .finally(() => {
        loading = null
      })
    return loading
  }

  async function loadProviders() {
    providers.value ??= await bridge.auth.providers().catch(() => ({ oidc: { title: 'daly.rest' }, password: true }))
    return providers.value
  }

  async function passwordLogin(password: string) {
    const next = await bridge.auth.passwordLogin(password)
    apply(next)
    return next
  }

  async function logout() {
    try {
      await bridge.auth.logout()
    }
    catch (e) {
      if (!(e instanceof ApiError && e.status === 401)) throw e
    }
    apply(null)
  }

  function loginUrl(next: string) {
    return bridge.auth.loginUrl(next)
  }

  /** Вход через daly.rest: сайт уходит на страницу провайдера, приложение открывает системный браузер */
  async function login(next: string) {
    const result = await bridge.auth.login(next)
    if (result) apply(result)
    return result
  }

  // Сессия кончилась не по нашей кнопке (401, выход в другом окне приложения) — на экран входа
  if (import.meta.client) {
    bridge.auth.onSignedOut(() => {
      if (status.value === 'anonymous') return
      apply(null)
      const route = useRouter().currentRoute.value
      if (route.path !== '/login') void navigateTo({ path: '/login', query: route.fullPath !== '/' ? { next: route.fullPath } : {} })
    })
  }

  return { me, status, providers, user, roles, isAdmin, can, load, loadProviders, passwordLogin, login, logout, loginUrl }
})
