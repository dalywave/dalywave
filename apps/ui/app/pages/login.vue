<script setup lang="ts">
import type { AuthProviders } from '~/api/types'
import { ApiError } from '~/api/types'
import { ShellLoginError } from '~/bridge/tauri'
import { FALLBACK_PROVIDERS } from '~/stores/auth'

// Вход. Что на экране — решает сервер (`/api/auth/providers`): кнопки провайдеров OIDC (daly.rest,
// Authentik, Google…), форма логина и пароля своих аккаунтов, мастер первого запуска. Один провайдер
// без своих аккаунтов — одна главная кнопка. Если вход сорвался — тот же экран говорит,
// что случилось. Аварийный вход паролем из .env — только на сайте и неприметной ссылкой.
// Публичное демо (`demo`): над всем — главная кнопка «Войти в демо» без пароля, ниже — свой аккаунт.
// Приложение входит через системный браузер: там открывается этот же экран с ?app=1
definePageMeta({ layout: 'bare' })

const route = useRoute()
const auth = useAuthStore()
const ui = useUiStore()
const bridge = useBridge()
const { t } = useScopedI18n('pages.login')

useHead({ title: () => t('title') })

const next = computed(() => {
  const n = route.query.next
  return typeof n === 'string' && n.startsWith('/') && !n.startsWith('//') ? n : '/'
})
// Вход приложения в системном браузере: код уходит приложению, сессия сайта не нужна
const appMode = computed(() => route.query.app !== undefined && bridge.kind === 'web')
const inApp = computed(() => bridge.kind !== 'web')

const providers = ref<AuthProviders | null>(null)
const serverDown = ref(false)
async function loadProviders() {
  try {
    providers.value = await bridge.auth.providers()
  }
  catch {
    serverDown.value = true
    providers.value = FALLBACK_PROVIDERS
  }
}
onMounted(loadProviders)

const list = computed(() => providers.value?.providers ?? [])
const local = computed(() => !!providers.value?.local)
const setup = computed(() => local.value && !!providers.value?.setup)
// Пока администратора нет, демо-аккаунта тоже нет: сначала мастер первого запуска
const demo = computed(() => !!providers.value?.demo && !setup.value)
// Один провайдер и без своих аккаунтов — одна главная кнопка
const single = computed(() => !local.value && list.value.length === 1 ? list.value[0]! : null)
const signingIn = ref(false)
const handedOff = ref(false)
function reload() {
  window.location.reload()
}

// Ошибка возврата от провайдера (сайт: ?error=…) или входа в приложении
const WEB_ERRORS = ['cancelled', 'expired', 'state', 'disabled', 'provider', 'not_registered']
const APP_ERRORS = ['cancelled', 'timeout', 'expired', 'state', 'provider', 'disabled', 'not_registered', 'invalid_request', 'invalid_grant', 'forbidden', 'too_many_requests', 'network', 'server', 'keychain', 'demo_unavailable']
const appError = ref<string | null>(null)
const failure = computed<string | null>(() => {
  if (appError.value) return appError.value
  const c = route.query.error
  if (typeof c !== 'string' || !c) return null
  return WEB_ERRORS.includes(c) ? c : 'unknown'
})

const heading = computed(() => {
  if (handedOff.value) return t('handoff.title')
  const f = failure.value
  if (f === 'cancelled') return t('cancelled.title')
  if (f === 'disabled') return t('disabled.title')
  if (f === 'not_registered') return t('notRegistered.title')
  if (f) return t('failed')
  if (serverDown.value) return t('down.title')
  if (setup.value) return t('setup.title')
  return 'DalyWave'
})
const text = computed(() => {
  if (handedOff.value) return t('handoff.text')
  const f = failure.value
  if (signingIn.value && !demoPending.value) return single.value ? t('app.wait', { title: single.value.title }) : t('app.waitAny')
  if (f) return t(`errors.${f}`)
  if (serverDown.value) return t('down.text')
  if (setup.value) return t('setup.text')
  if (appMode.value) return t('appLead')
  return t('lead')
})

// ---------- Приложение: вход в системном окне браузера, токен остаётся в связке ключей ----------
// Демо — тот же вход, только сервер сразу возвращает код общего аккаунта, без экрана входа
const demoPending = ref(false)
async function appLogin(opts: { demo?: boolean } = {}) {
  signingIn.value = true
  demoPending.value = !!opts.demo
  appError.value = null
  try {
    const me = await auth.login(next.value, opts)
    if (me) await navigateTo(auth.status === 'no-access' ? '/waiting' : next.value)
  }
  catch (e) {
    const code = e instanceof ShellLoginError ? e.code : 'unknown'
    appError.value = APP_ERRORS.includes(code) ? code : 'unknown'
  }
  finally {
    signingIn.value = false
    demoPending.value = false
  }
}
function cancelAppLogin() {
  void bridge.auth.cancelLogin()
}

// ---------- Свой аккаунт: вход и мастер первого запуска ----------
const busy = ref(false)
const localError = ref<string | null>(null)

function describe(e: unknown): string {
  if (!(e instanceof ApiError)) return t('form.failed')
  if (e.status === 429) return t('form.rateLimited')
  if (e.status === 401) return t('form.wrong')
  if (e.code === 'weak_password') return t('form.weak')
  if (e.code === 'bad_username') return t('form.badUsername')
  if (e.code === 'app_expired') return t('form.appExpired')
  if (e.status === 403) return t('form.disabled')
  return t('form.failed')
}

async function finish(result: Parameters<typeof auth.signedIn>[0]) {
  if (auth.signedIn(result) === 'app') {
    handedOff.value = true
    return
  }
  await navigateTo(auth.status === 'no-access' ? '/waiting' : next.value)
}

async function localLogin(v: { username: string, password: string }) {
  busy.value = true
  localError.value = null
  try {
    await finish(await bridge.auth.localLogin(v.username, v.password, appMode.value))
  }
  catch (e) {
    localError.value = describe(e)
  }
  finally {
    busy.value = false
  }
}

// ---------- Публичное демо: общий аккаунт без пароля ----------
const demoBusy = ref(false)
const demoError = ref<string | null>(null)

async function demoLogin() {
  demoBusy.value = true
  demoError.value = null
  try {
    await finish(await bridge.auth.demoLogin(appMode.value))
  }
  catch (e) {
    if (e instanceof ApiError && e.status === 429) demoError.value = t('form.rateLimited')
    else if (e instanceof ApiError && (e.status === 404 || e.status === 403)) demoError.value = t('demo.unavailable')
    else demoError.value = describe(e)
  }
  finally {
    demoBusy.value = false
  }
}

async function createAdmin(v: { name: string, username: string, password: string }) {
  busy.value = true
  localError.value = null
  try {
    await finish(await bridge.auth.setup({ name: v.name || null, username: v.username, password: v.password, app: appMode.value }))
  }
  catch (e) {
    // Администратора уже создали (в другой вкладке): экран входа как обычно
    if (e instanceof ApiError && e.code === 'already_set_up') await loadProviders()
    else localError.value = describe(e)
  }
  finally {
    busy.value = false
  }
}

// ---------- Сервер приложения и приложение на телефоне ----------
const serverHost = computed(() => {
  if (!bridge.appServer || typeof location === 'undefined') return ''
  try {
    return new URL(bridge.resolveUrl('/'), location.href).host
  }
  catch {
    return ''
  }
})
// Сайт (не вход приложения в браузере): ссылка dalywave://server на этот сервер
const phoneLink = computed(() => (bridge.kind === 'web' && !appMode.value && typeof location !== 'undefined'
  ? `dalywave://server?url=${encodeURIComponent(location.origin)}`
  : null))
const qrOpen = ref(false)

// ---------- Аварийный вход ----------
const canPassword = computed(() => !!providers.value?.password && bridge.auth.passwordAllowed && !appMode.value && !handedOff.value)
const showPassword = ref(false)
const password = ref('')
const pwBusy = ref(false)
const formError = ref<string | null>(null)
const input = ref<HTMLInputElement | null>(null)

watch(showPassword, (v) => {
  formError.value = null
  // Лист сначала выезжает: фокус — после, иначе клавиатура дёргает его на полпути
  if (v) setTimeout(() => input.value?.focus(), ui.isTouchUi ? 450 : 0)
})

async function submit() {
  if (!password.value || pwBusy.value) return
  pwBusy.value = true
  formError.value = null
  try {
    await auth.passwordLogin(password.value)
    password.value = ''
    showPassword.value = false
    await navigateTo(auth.status === 'no-access' ? '/waiting' : next.value)
  }
  catch (e) {
    if (e instanceof ApiError && e.status === 429) formError.value = t('form.rateLimited')
    else if (e instanceof ApiError && (e.status === 401 || e.status === 403)) formError.value = t('emergency.wrong')
    else formError.value = t('form.failed')
  }
  finally {
    pwBusy.value = false
  }
}
</script>

<template>
  <AuthStage :title="heading" :text="text">
    <template v-if="handedOff" />
    <template v-else-if="serverDown && !failure">
      <UiButton variant="primary" size="lg" block @click="reload">{{ t('down.reload') }}</UiButton>
    </template>

    <!-- Приложение: одна кнопка — системный браузер откроет этот же экран со всеми способами.
         На демо над ней «Войти в демо»: браузер сразу возвращает код общего аккаунта -->
    <template v-else-if="inApp && providers">
      <template v-if="demo">
        <UiButton variant="primary" size="lg" block :loading="signingIn && demoPending" :disabled="signingIn && !demoPending" @click="appLogin({ demo: true })">{{ t('demo.button') }}</UiButton>
        <p class="demo-note">{{ t('demo.note') }}</p>
        <AuthOr class="demo-or" :label="t('demo.or')" />
      </template>
      <UiButton :variant="demo ? 'secondary' : 'primary'" size="lg" block :loading="signingIn && !demoPending" :disabled="signingIn && demoPending" @click="appLogin()">
        {{ failure ? t('retry') : single ? t('oidc', { title: single.title }) : t('signIn') }}
      </UiButton>
      <UiButton v-if="signingIn && bridge.kind === 'tauri'" variant="ghost" size="lg" block @click="cancelAppLogin">{{ t('app.cancel') }}</UiButton>
    </template>

    <template v-else-if="providers">
      <template v-if="demo">
        <UiButton variant="primary" size="lg" block :loading="demoBusy" :disabled="busy" @click="demoLogin">{{ t('demo.button') }}</UiButton>
        <p v-if="demoError" class="demo-note demo-note--error" role="alert">{{ demoError }}</p>
        <p v-else class="demo-note">{{ t('demo.note') }}</p>
        <AuthOr v-if="local || list.length" class="demo-or" :label="t('demo.or')" />
      </template>
      <AuthCredentials
        v-if="setup"
        mode="create"
        :submit="t('setup.submit')"
        :busy="busy"
        :error="localError"
        @submit="createAdmin"
      />
      <AuthCredentials
        v-else-if="local"
        mode="login"
        :submit="t('form.submit')"
        :busy="busy"
        :error="localError"
        @submit="localLogin"
      />
      <AuthOr v-if="local && list.length" />
      <AuthProviderList
        v-if="list.length"
        :providers="list"
        :primary="!!single"
        :next="next"
        :app="appMode"
        :label="single && failure ? t('retry') : undefined"
      />
    </template>

    <template v-if="!signingIn && !handedOff" #foot>
      <div class="lf">
      <!-- Компьютер: поле прямо здесь; под пальцем — нижний лист -->
      <form v-if="canPassword && showPassword && !ui.isTouchUi" class="pw pw--inline" @submit.prevent="submit">
        <div class="pw__row">
          <input
            ref="input"
            v-model="password"
            class="pw__input"
            type="password"
            autocomplete="current-password"
            :aria-label="t('emergency.title')"
            :aria-invalid="!!formError || undefined"
            :aria-describedby="formError ? 'pw-error' : undefined"
            :placeholder="t('emergency.placeholder')"
            @keydown.esc="showPassword = false"
          >
          <UiButton type="submit" variant="secondary" :loading="pwBusy" :disabled="!password">{{ t('form.submit') }}</UiButton>
        </div>
        <p v-if="formError" id="pw-error" class="pw__error" role="alert">{{ formError }}</p>
      </form>
      <div class="lf__links">
        <button v-if="canPassword && !showPassword" type="button" class="pw-link" @click="showPassword = true">{{ local ? t('emergency.link') : t('password') }}</button>
        <!-- Приложение: к какому серверу подключено и как сменить -->
        <NuxtLink v-if="bridge.appServer" to="/server" class="pw-link">{{ t('changeServer', { host: serverHost }) }}</NuxtLink>
        <!-- Сайт: приложение на телефоне — QR-код с адресом этого сервера; на самом телефоне — ссылка -->
        <a v-if="phoneLink && ui.isPhone" :href="phoneLink" class="pw-link">{{ t('openApp') }}</a>
        <button v-else-if="phoneLink" type="button" class="pw-link" @click="qrOpen = true">{{ t('phone') }}</button>
        <NuxtLink to="/privacy" class="pw-link">{{ t('privacy') }}</NuxtLink>
      </div>
      <!-- Окно уходит в портал: место в разметке не важно -->
      <UiModal v-if="phoneLink" v-model:open="qrOpen" :title="t('phone')" :width="360">
        <div class="lq"><AuthAppQr compact /></div>
      </UiModal>
      </div>
    </template>

    <!-- Лист аварийного входа под пальцем (уходит в портал — место в разметке не важно) -->
    <template v-if="canPassword && ui.isTouchUi" #extra>
    <UiSheet v-model:open="showPassword" :title="t('emergency.title')">
      <form class="pw pw--sheet" @submit.prevent="submit">
        <input
          ref="input"
          v-model="password"
          class="pw__input"
          type="password"
          autocomplete="current-password"
          :aria-label="t('emergency.title')"
          :aria-invalid="!!formError || undefined"
          :aria-describedby="formError ? 'pw-error-sheet' : undefined"
          :placeholder="t('emergency.placeholder')"
        >
        <p v-if="formError" id="pw-error-sheet" class="pw__error" role="alert">{{ formError }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="pwBusy" :disabled="!password">{{ t('form.submit') }}</UiButton>
      </form>
    </UiSheet>
    </template>
  </AuthStage>
</template>

<style scoped>
/* Подпись под кнопкой демо: то же место — для ошибки входа в демо. Под разделителем — воздух:
   дальше другой способ входа, а не продолжение кнопки */
.demo-note { margin: 0 0 6px; font-size: var(--fs-small); line-height: 1.4; color: var(--text-3); text-align: center; }
.demo-note--error { color: var(--danger); }
p.demo-or { margin: 6px 0 20px; }
/* Ссылка на аварийный вход: неприметная, но с полноценной зоной касания */
.lf { display: flex; flex-direction: column; align-items: center; width: 100%; }
.lf__links { display: flex; flex-wrap: wrap; justify-content: center; }
.lq { padding: 4px 20px 20px; }
.pw-link { display: inline-flex; align-items: center; text-decoration: none; min-height: 44px; padding: 0 12px; border: 0; background: none; color: var(--text-4); font: inherit; font-size: var(--fs-small); cursor: pointer; transition: color var(--dur-1); }
.pw-link:hover { color: var(--text-2); }
.pw-link:focus-visible { outline: none; border-radius: var(--r-2); box-shadow: 0 0 0 2px var(--focus); }
.pw { display: flex; flex-direction: column; gap: 8px; width: 100%; text-align: left; animation: fade-in var(--dur-2); }
.pw__row { display: flex; gap: 8px; }
.pw__input {
  flex: 1;
  min-width: 0;
  height: 40px;
  padding: 0 16px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
  transition: box-shadow var(--dur-1), background-color var(--dur-1);
}
.pw__input::placeholder { color: var(--text-4); }
.pw__input:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.pw__input:focus-visible { outline: none; background: var(--fill-2); box-shadow: inset 0 0 0 1.5px var(--focus); }
.pw__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.pw__error { margin: 0; padding: 0 16px; font-size: var(--fs-caption); color: var(--danger); }
.pw--inline { margin-top: 4px; }
.pw--sheet { gap: 12px; padding: 4px 20px 16px; }
.pw--sheet .pw__input { flex: none; height: 48px; font-size: 16px; }
</style>
