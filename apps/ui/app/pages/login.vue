<script setup lang="ts">
import { ApiError } from '~/api/types'
import { ShellLoginError } from '~/bridge/tauri'

// Вход: через daly.rest (Google, Discord, Telegram, Twitch, ключ доступа — всё там).
// Аварийный вход паролем из .env — для владельца, когда daly.rest недоступен
definePageMeta({ layout: 'bare' })

const route = useRoute()
const auth = useAuthStore()
const ui = useUiStore()
const bridge = useBridge()
const { t } = useI18n({ useScope: 'local' })

useHead({ title: () => t('title') })

const next = computed(() => {
  const n = route.query.next
  return typeof n === 'string' && n.startsWith('/') && !n.startsWith('//') ? n : '/'
})
const errorCode = computed(() => (typeof route.query.error === 'string' ? route.query.error : null))
const KNOWN = ['cancelled', 'expired', 'state', 'disabled', 'provider'] as const
const errorText = computed(() => {
  const c = errorCode.value
  if (!c) return null
  return (KNOWN as readonly string[]).includes(c) ? t(`errors.${c}`) : t('errors.unknown')
})

const providers = ref<{ oidc: { title: string } | null, password: boolean } | null>(null)
const serverDown = ref(false)
onMounted(async () => {
  try {
    providers.value = await bridge.auth.providers()
  }
  catch {
    serverDown.value = true
    providers.value = { oidc: { title: 'daly.rest' }, password: true }
  }
})

const showEmergency = ref(false)
const password = ref('')
const busy = ref(false)
const formError = ref<string | null>(null)
const input = ref<HTMLInputElement | null>(null)
const loginHref = computed(() => (bridge.kind === 'web' ? auth.loginUrl(next.value) : undefined))
const signingIn = ref(false)

// Приложение: вход в системном браузере, токен остаётся в связке ключей
const APP_ERRORS = ['timeout', 'expired', 'state', 'provider', 'disabled', 'invalid_request', 'invalid_grant', 'forbidden', 'too_many_requests', 'network', 'server', 'keychain']
const appError = ref<string | null>(null)
async function appLogin() {
  if (bridge.kind === 'web') return
  signingIn.value = true
  appError.value = null
  try {
    const me = await auth.login(next.value)
    if (me) await navigateTo(auth.status === 'no-access' ? '/waiting' : next.value)
  }
  catch (e) {
    const code = e instanceof ShellLoginError ? e.code : 'unknown'
    appError.value = APP_ERRORS.includes(code) ? t(`app.${code}`) : t('errors.unknown')
  }
  finally {
    signingIn.value = false
  }
}

function cancelAppLogin() {
  void bridge.auth.cancelLogin()
}

watch(showEmergency, (v) => {
  if (v) void nextTick(() => input.value?.focus())
})

async function submit() {
  if (!password.value || busy.value) return
  busy.value = true
  formError.value = null
  try {
    await auth.passwordLogin(password.value)
    password.value = ''
    await navigateTo(auth.status === 'no-access' ? '/waiting' : next.value)
  }
  catch (e) {
    if (e instanceof ApiError && e.status === 429) formError.value = t('form.rateLimited')
    else if (e instanceof ApiError && (e.status === 401 || e.status === 403)) formError.value = t('form.wrong')
    else formError.value = e instanceof ApiError ? e.message : t('form.failed')
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="login">
    <div class="login__wave"><MusicWaveCanvas :layout="ui.isPhone ? 'full-phone' : 'full'" /></div>
    <main class="login__card">
      <img src="/favicon.svg" alt="" class="login__icon" width="72" height="72">
      <h1 class="login__title">DalyWave</h1>
      <p class="login__lead">{{ t('lead') }}</p>

      <div v-if="appError || errorText" class="login__error" role="alert">
        <Icon name="ph:warning-circle-fill" />
        <span>{{ appError ?? errorText }}</span>
      </div>
      <div v-else-if="serverDown" class="login__error" role="alert">
        <Icon name="ph:wifi-slash" />
        <span>{{ t('serverDown') }}</span>
      </div>

      <div v-if="signingIn && bridge.kind === 'tauri'" class="login__browser" role="status">
        <Icon name="ph:browser" class="login__browser-icon" />
        <div class="login__browser-title">{{ t('app.waitTitle') }}</div>
        <p class="login__browser-text">{{ t('app.waitText') }}</p>
        <UiButton variant="secondary" icon="ph:x" @click="cancelAppLogin">{{ t('app.cancel') }}</UiButton>
      </div>
      <template v-else>
        <UiButton v-if="providers?.oidc !== null" variant="primary" size="lg" block :href="loginHref" icon="ph:sign-in" class="login__main" @click="appLogin">
          {{ t('oidc', { title: providers?.oidc?.title ?? 'daly.rest' }) }}
        </UiButton>
      </template>

      <template v-if="providers?.password && bridge.auth.passwordAllowed">
        <div class="login__sep"><span>{{ t('or') }}</span></div>
        <UiButton v-if="!showEmergency" variant="ghost" icon="ph:key" @click="showEmergency = true">{{ t('emergency') }}</UiButton>
        <form v-else class="login__form" @submit.prevent="submit">
          <label class="login__label" for="pw">{{ t('form.label') }}</label>
          <div class="login__row">
            <input
              id="pw"
              ref="input"
              v-model="password"
              class="login__input"
              type="password"
              autocomplete="current-password"
              :aria-invalid="!!formError || undefined"
              :aria-describedby="formError ? 'pw-error' : undefined"
              :placeholder="t('form.placeholder')"
            >
            <UiButton type="submit" variant="secondary" :loading="busy" :disabled="!password">{{ t('form.submit') }}</UiButton>
          </div>
          <p v-if="formError" id="pw-error" class="login__form-error" role="alert">{{ formError }}</p>
        </form>
      </template>
    </main>
  </div>
</template>

<style scoped>
.login { position: relative; display: grid; place-items: center; min-height: 100vh; min-height: 100dvh; padding: 32px 16px; overflow: hidden; background: #020509; }
.login__wave { position: absolute; inset: 0; }
.login__wave::after { content: ''; position: absolute; inset: 0; pointer-events: none; background: radial-gradient(60% 55% at 50% 50%, rgba(2, 5, 9, 0.78), rgba(2, 5, 9, 0.35) 70%, rgba(2, 5, 9, 0.6)); }
.login__card {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  width: min(420px, 100%);
  padding: 40px 32px 28px;
  border-radius: var(--r-6);
  background: rgba(10, 12, 16, 0.6);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
  backdrop-filter: blur(30px) saturate(1.4);
  box-shadow: inset 0 0 0 1px var(--line), var(--shadow-pop);
  text-align: center;
}
.login__icon { width: 72px; height: 72px; }
.login__title { margin: 18px 0 0; font-size: 32px; font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.1; }
.login__lead { max-width: 300px; margin: 8px 0 24px; color: var(--text-2); text-wrap: balance; }
.login__error { display: flex; align-items: flex-start; gap: 10px; width: 100%; margin-bottom: 16px; padding: 10px 12px; border-radius: var(--r-3); background: color-mix(in oklab, var(--danger) 12%, transparent); box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--danger) 30%, transparent); font-size: var(--fs-small); text-align: left; }
.login__error :deep(.iconify) { flex: none; margin-top: 1px; font-size: 18px; color: var(--danger); }
.login__main { width: 100%; }
.login__sep { display: flex; align-items: center; gap: 12px; width: 100%; margin: 22px 0 10px; font-size: var(--fs-caption); color: var(--text-4); }
.login__sep::before, .login__sep::after { content: ''; flex: 1; height: 1px; background: var(--hair); }
.login__browser { display: flex; flex-direction: column; align-items: center; gap: 8px; width: 100%; padding: 18px 16px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); animation: fade-in var(--dur-2); }
.login__browser-icon { font-size: 28px; color: var(--accent); }
.login__browser-title { font-weight: 650; }
.login__browser-text { margin: 0 0 6px; font-size: var(--fs-small); color: var(--text-2); text-wrap: balance; }
.login__form { display: flex; flex-direction: column; gap: 8px; width: 100%; text-align: left; animation: fade-in var(--dur-2); }
.login__label { font-size: var(--fs-small); font-weight: 600; }
.login__row { display: flex; gap: 8px; }
.login__input {
  flex: 1;
  min-width: 0;
  height: 40px;
  padding: 0 14px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
  transition: box-shadow var(--dur-1), background-color var(--dur-1);
}
.login__input::placeholder { color: var(--text-4); }
.login__input:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.login__input:focus-visible { outline: none; background: var(--fill-2); box-shadow: inset 0 0 0 1.5px var(--focus); }
.login__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.login__form-error { margin: 0; font-size: var(--fs-caption); color: var(--danger); }
@media (max-width: 699px) {
  .login { align-items: end; padding: 16px 12px calc(16px + var(--safe-bottom)); }
  .login__card { padding: 32px 20px 20px; border-radius: 28px; }
  .login__input { height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Вход",
    "lead": "Ваша музыка — и волна, которая знает, что вам нравится",
    "oidc": "Войти через {title}",
    "or": "или",
    "emergency": "Аварийный вход",
    "serverDown": "Сервер не отвечает. Обновите страницу чуть позже",
    "errors": {
      "cancelled": "Вход отменён. Можно попробовать ещё раз",
      "expired": "Ссылка для входа устарела — начните заново",
      "state": "Не получилось проверить вход. Начните заново в этой же вкладке",
      "disabled": "Аккаунт отключён. Напишите администратору",
      "provider": "daly.rest сейчас не отвечает. Попробуйте через минуту",
      "unknown": "Не получилось войти. Попробуйте ещё раз"
    },
    "app": {
      "waitTitle": "Завершите вход в браузере",
      "waitText": "Мы открыли daly.rest в браузере. Войдите там — приложение продолжит само",
      "cancel": "Отмена",
      "timeout": "Вход не завершили за 10 минут — начните заново",
      "expired": "Вход устарел — начните заново",
      "state": "Вход начали в другом браузере или окне — начните заново",
      "provider": "daly.rest сейчас не отвечает. Попробуйте через минуту",
      "disabled": "Аккаунт отключён. Напишите администратору",
      "invalid_request": "Сервер не принял вход — начните заново",
      "invalid_grant": "Код входа уже использован или устарел — начните заново",
      "forbidden": "Сервер не разрешил вход этому приложению",
      "too_many_requests": "Слишком много попыток. Подождите минуту",
      "network": "Нет связи с сервером. Проверьте сеть",
      "server": "Сервер ответил неожиданно. Попробуйте позже",
      "keychain": "Связка ключей не сохранила вход. Проверьте доступ приложения к ней"
    },
    "form": {
      "label": "Пароль владельца",
      "placeholder": "Пароль",
      "submit": "Войти",
      "wrong": "Пароль не подошёл",
      "rateLimited": "Слишком много попыток. Подождите минуту",
      "failed": "Не получилось войти. Проверьте связь с сервером"
    }
  },
  "en": {
    "title": "Sign in",
    "lead": "Your music — and a wave that knows what you like",
    "oidc": "Sign in with {title}",
    "or": "or",
    "emergency": "Emergency sign-in",
    "serverDown": "The server isn't responding. Reload in a moment",
    "errors": {
      "cancelled": "Sign-in was cancelled. You can try again",
      "expired": "The sign-in link expired — start over",
      "state": "Couldn't verify the sign-in. Start over in this tab",
      "disabled": "This account is disabled. Contact the administrator",
      "provider": "daly.rest isn't responding. Try again in a minute",
      "unknown": "Couldn't sign in. Try again"
    },
    "app": {
      "waitTitle": "Finish signing in in your browser",
      "waitText": "We opened daly.rest in your browser. Sign in there — the app continues by itself",
      "cancel": "Cancel",
      "timeout": "Sign-in wasn't finished within 10 minutes — start over",
      "expired": "The sign-in expired — start over",
      "state": "Sign-in started in another browser or window — start over",
      "provider": "daly.rest isn't responding. Try again in a minute",
      "disabled": "This account is disabled. Contact the administrator",
      "invalid_request": "The server rejected the sign-in — start over",
      "invalid_grant": "The sign-in code was used or expired — start over",
      "forbidden": "The server didn't allow this app to sign in",
      "too_many_requests": "Too many attempts. Wait a minute",
      "network": "No connection to the server. Check your network",
      "server": "The server responded unexpectedly. Try later",
      "keychain": "The keychain didn't save the sign-in. Check the app's access to it"
    },
    "form": {
      "label": "Owner password",
      "placeholder": "Password",
      "submit": "Sign in",
      "wrong": "Wrong password",
      "rateLimited": "Too many attempts. Wait a minute",
      "failed": "Couldn't sign in. Check the connection to the server"
    }
  }
}
</i18n>
