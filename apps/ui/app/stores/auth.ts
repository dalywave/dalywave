import { defineStore } from 'pinia'
import type { AuthProviders, Me, Permission, SignInResult } from '~/api/types'
import { ApiError } from '~/api/types'

// Подсказка «вход уже был»: без неё публичные экраны не спрашивают сервер о сессии впустую
const HINT_KEY = 'dw.session.hint'

/** Сервер не ответил, какие способы входа есть: показываем вход через первого провайдера */
export const FALLBACK_PROVIDERS: AuthProviders = {
  oidc: { id: 'dalyrest', title: 'daly.rest', icon: null },
  providers: [{ id: 'dalyrest', title: 'daly.rest', icon: null }],
  password: true,
  local: false,
  setup: false,
  demo: false,
}

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
  /** Общий аккаунт публичного демо: пароль, входы и удаление аккаунта закрыты */
  const isDemo = computed(() => !!me.value?.demo)

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
    providers.value ??= await bridge.auth.providers().catch(() => FALLBACK_PROVIDERS)
    return providers.value
  }

  async function passwordLogin(password: string) {
    const next = await bridge.auth.passwordLogin(password)
    apply(next)
    return next
  }

  /** Итог входа своим аккаунтом: сессия сайта — запоминаем; вход приложения — браузер уходит в приложение */
  function signedIn(result: SignInResult): 'site' | 'app' {
    if (result.redirect) {
      window.location.assign(result.redirect)
      return 'app'
    }
    apply(result.me)
    return 'site'
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

  /** Вход через daly.rest: сайт уходит на страницу провайдера, приложение открывает системный браузер.
   *  `demo` — приложение входит в общий аккаунт демо */
  async function login(next: string, opts?: { demo?: boolean }) {
    const result = await bridge.auth.login(next, opts)
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

  return { me, status, providers, user, roles, isAdmin, isDemo, can, load, loadProviders, passwordLogin, signedIn, login, logout, loginUrl }
})
