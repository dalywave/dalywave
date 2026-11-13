<script setup lang="ts">
import { ApiError } from '~/api/types'
import { ShellLoginError } from '~/bridge/tauri'

// Вход: через daly.rest (Google, Discord, Telegram, Twitch, ключ доступа — всё там). Одна главная
// кнопка; если вход сорвался — тот же экран говорит, что случилось и что делать. Аварийный вход
// паролем из .env — для владельца, когда daly.rest недоступен: только на сайте и неприметной ссылкой
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
const provider = computed(() => providers.value?.oidc?.title ?? 'daly.rest')
const signingIn = ref(false)
function reload() {
  window.location.reload()
}

// Ошибка возврата с daly.rest (сайт: ?error=…) или входа в приложении
const WEB_ERRORS = ['cancelled', 'expired', 'state', 'disabled', 'provider']
const APP_ERRORS = ['cancelled', 'timeout', 'expired', 'state', 'provider', 'disabled', 'invalid_request', 'invalid_grant', 'forbidden', 'too_many_requests', 'network', 'server', 'keychain']
const appError = ref<string | null>(null)
const failure = computed<string | null>(() => {
  if (appError.value) return appError.value
  const c = route.query.error
  if (typeof c !== 'string' || !c) return null
  return WEB_ERRORS.includes(c) ? c : 'unknown'
})

const heading = computed(() => {
  const f = failure.value
  if (f === 'cancelled') return t('cancelled.title')
  if (f === 'disabled') return t('disabled.title')
  if (f) return t('failed')
  if (serverDown.value) return t('down.title')
  return 'DalyWave'
})
const text = computed(() => {
  const f = failure.value
  if (signingIn.value) return t('app.wait', { title: provider.value })
  if (f) return t(`errors.${f}`)
  if (serverDown.value) return t('down.text')
  return t('lead')
})

// Приложение: вход в системном окне браузера, токен остаётся в связке ключей
const loginHref = computed(() => (bridge.kind === 'web' ? auth.loginUrl(next.value) : undefined))
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
    appError.value = APP_ERRORS.includes(code) ? code : 'unknown'
  }
  finally {
    signingIn.value = false
  }
}
function cancelAppLogin() {
  void bridge.auth.cancelLogin()
}

// ---------- Аварийный вход ----------
const canPassword = computed(() => !!providers.value?.password && bridge.auth.passwordAllowed)
const showPassword = ref(false)
const password = ref('')
const busy = ref(false)
const formError = ref<string | null>(null)
const input = ref<HTMLInputElement | null>(null)

watch(showPassword, (v) => {
  formError.value = null
  // Лист сначала выезжает: фокус — после, иначе клавиатура дёргает его на полпути
  if (v) setTimeout(() => input.value?.focus(), ui.isTouchUi ? 450 : 0)
})

async function submit() {
  if (!password.value || busy.value) return
  busy.value = true
  formError.value = null
  try {
    await auth.passwordLogin(password.value)
    password.value = ''
    showPassword.value = false
    await navigateTo(auth.status === 'no-access' ? '/waiting' : next.value)
  }
  catch (e) {
    if (e instanceof ApiError && e.status === 429) formError.value = t('form.rateLimited')
    else if (e instanceof ApiError && (e.status === 401 || e.status === 403)) formError.value = t('form.wrong')
    else formError.value = t('form.failed')
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <AuthStage :title="heading" :text="text">
    <template v-if="serverDown && !failure">
      <UiButton variant="primary" size="lg" block @click="reload">{{ t('down.reload') }}</UiButton>
    </template>
    <template v-else-if="providers?.oidc !== null">
      <UiButton variant="primary" size="lg" block :href="loginHref" :loading="signingIn" @click="appLogin">
        {{ failure ? t('retry') : t('oidc', { title: provider }) }}
      </UiButton>
      <UiButton v-if="signingIn && bridge.kind === 'tauri'" variant="ghost" size="lg" block @click="cancelAppLogin">{{ t('app.cancel') }}</UiButton>
    </template>

    <template v-if="canPassword && !signingIn" #foot>
      <!-- Компьютер: поле прямо здесь; под пальцем — нижний лист -->
      <form v-if="showPassword && !ui.isTouchUi" class="pw pw--inline" @submit.prevent="submit">
        <div class="pw__row">
          <input
            ref="input"
            v-model="password"
            class="pw__input"
            type="password"
            autocomplete="current-password"
            :aria-label="t('form.title')"
            :aria-invalid="!!formError || undefined"
            :aria-describedby="formError ? 'pw-error' : undefined"
            :placeholder="t('form.placeholder')"
            @keydown.esc="showPassword = false"
          >
          <UiButton type="submit" variant="secondary" :loading="busy" :disabled="!password">{{ t('form.submit') }}</UiButton>
        </div>
        <p v-if="formError" id="pw-error" class="pw__error" role="alert">{{ formError }}</p>
      </form>
      <button v-else type="button" class="pw-link" @click="showPassword = true">{{ t('password') }}</button>
    </template>

    <!-- Лист аварийного входа под пальцем (уходит в портал — место в разметке не важно) -->
    <template v-if="canPassword && ui.isTouchUi" #extra>
    <UiSheet v-model:open="showPassword" :title="t('form.title')">
      <form class="pw pw--sheet" @submit.prevent="submit">
        <input
          ref="input"
          v-model="password"
          class="pw__input"
          type="password"
          autocomplete="current-password"
          :aria-label="t('form.title')"
          :aria-invalid="!!formError || undefined"
          :aria-describedby="formError ? 'pw-error-sheet' : undefined"
          :placeholder="t('form.placeholder')"
        >
        <p v-if="formError" id="pw-error-sheet" class="pw__error" role="alert">{{ formError }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="busy" :disabled="!password">{{ t('form.submit') }}</UiButton>
      </form>
    </UiSheet>
    </template>
  </AuthStage>
</template>

<style scoped>
/* Ссылка на аварийный вход: неприметная, но с полноценной зоной касания */
.pw-link { min-height: 44px; padding: 0 12px; border: 0; background: none; color: var(--text-4); font: inherit; font-size: var(--fs-small); cursor: pointer; transition: color var(--dur-1); }
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

<i18n lang="json">
{
  "ru": {
    "title": "Вход",
    "lead": "Ваша музыка — и волна, которая знает, что вам нравится",
    "oidc": "Войти через {title}",
    "retry": "Попробовать снова",
    "password": "Вход по паролю",
    "failed": "Не получилось войти",
    "cancelled": { "title": "Вход отменён" },
    "disabled": { "title": "Доступ закрыт" },
    "down": {
      "title": "Сервер не отвечает",
      "text": "Скорее всего, это ненадолго. Попробуйте через минуту",
      "reload": "Обновить"
    },
    "errors": {
      "cancelled": "Ничего страшного — можно попробовать ещё раз",
      "expired": "Ссылка для входа устарела. Начните заново",
      "state": "Вход начали в другой вкладке или браузере. Начните заново здесь",
      "disabled": "Аккаунт отключён. Напишите администратору",
      "provider": "daly.rest сейчас не отвечает. Попробуйте через минуту",
      "timeout": "Вход не завершили за 10 минут. Начните заново",
      "invalid_request": "Сервер не принял вход. Начните заново",
      "invalid_grant": "Код входа уже использован или устарел. Начните заново",
      "forbidden": "Сервер не разрешил вход этому приложению. Напишите администратору",
      "too_many_requests": "Слишком много попыток. Подождите минуту",
      "network": "Нет связи с сервером. Проверьте интернет и попробуйте снова",
      "server": "Сервер ответил неожиданно. Попробуйте чуть позже",
      "keychain": "Связка ключей не сохранила вход. Проверьте доступ приложения к ней",
      "unknown": "Что-то пошло не так. Попробуйте ещё раз"
    },
    "app": {
      "wait": "Завершите вход на {title} в открывшемся окне",
      "cancel": "Отменить"
    },
    "form": {
      "title": "Вход по паролю",
      "placeholder": "Пароль владельца",
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
    "retry": "Try again",
    "password": "Sign in with password",
    "failed": "Couldn't sign in",
    "cancelled": { "title": "Sign-in cancelled" },
    "disabled": { "title": "Access closed" },
    "down": {
      "title": "The server isn't responding",
      "text": "It's probably brief. Try again in a minute",
      "reload": "Reload"
    },
    "errors": {
      "cancelled": "No problem — you can try again",
      "expired": "The sign-in link expired. Start over",
      "state": "Sign-in started in another tab or browser. Start over here",
      "disabled": "This account is disabled. Contact the administrator",
      "provider": "daly.rest isn't responding. Try again in a minute",
      "timeout": "Sign-in wasn't finished within 10 minutes. Start over",
      "invalid_request": "The server rejected the sign-in. Start over",
      "invalid_grant": "The sign-in code was used or expired. Start over",
      "forbidden": "The server didn't allow this app to sign in. Contact the administrator",
      "too_many_requests": "Too many attempts. Wait a minute",
      "network": "No connection to the server. Check the internet and try again",
      "server": "The server responded unexpectedly. Try a bit later",
      "keychain": "The keychain didn't save the sign-in. Check the app's access to it",
      "unknown": "Something went wrong. Try again"
    },
    "app": {
      "wait": "Finish signing in to {title} in the window that opened",
      "cancel": "Cancel"
    },
    "form": {
      "title": "Sign in with password",
      "placeholder": "Owner password",
      "submit": "Sign in",
      "wrong": "Wrong password",
      "rateLimited": "Too many attempts. Wait a minute",
      "failed": "Couldn't sign in. Check the connection to the server"
    }
  }
}
</i18n>
