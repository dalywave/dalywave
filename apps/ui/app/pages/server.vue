<script setup lang="ts">
import type { AppServerError, AppServerInfo, AppServerState } from '~/bridge/types'

// «Адрес сервера» — только в приложении: DalyWave — клиент своего сервера (как Jellyfin). Первый
// запуск сборок для App Store начинается здесь; в настройках — «Сменить сервер». Адрес проверяется
// (`/api/config` и `/api/healthz`): найденный сервер показан названием и версией, подключение — второй
// кнопкой. Ссылка `dalywave://server?url=…` (QR-код в админке и на странице входа сайта) открывает
// этот же экран с подставленным адресом: сама она сервер не меняет.
definePageMeta({ layout: 'bare' })

const route = useRoute()
const bridge = useBridge()
const auth = useAuthStore()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const appServer = bridge.appServer
const state = ref<AppServerState | null>(null)
const address = ref('')
const checking = ref<string | null>(null)
const found = ref<AppServerInfo | null>(null)
const error = ref<AppServerError | 'unknown' | null>(null)
const connecting = ref(false)
const input = ref<HTMLInputElement | null>(null)

// Смена сервера из настроек: есть куда вернуться
const changing = computed(() => !!state.value?.chosen)
const recent = computed(() => (state.value?.recent ?? []).filter(s => s.url !== found.value?.url))

function host(url: string) {
  try {
    const u = new URL(url)
    return u.port ? `${u.hostname}:${u.port}` : u.hostname
  }
  catch {
    return url
  }
}

async function check(url: string) {
  if (!appServer || !url.trim()) return
  checking.value = url
  error.value = null
  found.value = null
  try {
    found.value = await appServer.check(url)
    address.value = host(found.value.url)
  }
  catch (e) {
    const code = (e as { code?: string }).code
    error.value = code === 'invalid' || code === 'unreachable' || code === 'not_dalywave' || code === 'down' ? code : 'unknown'
  }
  finally {
    checking.value = null
  }
}

async function connect() {
  const s = found.value
  if (!appServer || !s) return
  connecting.value = true
  try {
    // Смена сервера — выход из текущего: его сессия устройства отзывается
    if (changing.value && s.url !== state.value?.url && auth.status !== 'anonymous') await auth.logout().catch(() => {})
    await appServer.use(s.url, s.name)
    // Оболочка сообщит о смене — страница перезагрузится уже с новым сервером (на вход)
  }
  catch {
    error.value = 'unknown'
    connecting.value = false
  }
}

function edit() {
  found.value = null
  void nextTick(() => input.value?.focus())
}

async function forget(url: string) {
  await appServer?.forget(url).catch(() => {})
  if (state.value) state.value = { ...state.value, recent: state.value.recent.filter(s => s.url !== url) }
}

function back() {
  if (window.history.length > 1) window.history.back()
  else void navigateTo('/settings')
}

watch(address, () => {
  if (error.value) error.value = null
})

onMounted(async () => {
  if (!appServer) return
  state.value = await appServer.state().catch(() => null)
  const link = route.query.url
  if (typeof link === 'string' && link) {
    address.value = host(link)
    await check(link)
  }
})

// Новая ссылка, пока экран открыт (plugins/server-link.client.ts меняет ?url=)
watch(() => route.query.url, (url) => {
  if (typeof url !== 'string' || !url) return
  address.value = host(url)
  void check(url)
})

const heading = computed(() => (found.value ? found.value.name : changing.value ? t('change') : t('title')))
const text = computed(() => {
  if (found.value) return found.value.version ? t('foundVersion', { host: host(found.value.url), version: found.value.version }) : host(found.value.url)
  return changing.value ? t('changeText') : t('lead')
})
</script>

<template>
  <AuthStage :title="heading" :text="text">
    <template v-if="found">
      <UiButton variant="primary" size="lg" block :loading="connecting" @click="connect">{{ t('connect') }}</UiButton>
      <UiButton variant="ghost" size="lg" block :disabled="connecting" @click="edit">{{ t('other') }}</UiButton>
    </template>

    <template v-else>
      <form class="sv" novalidate @submit.prevent="check(address)">
        <input
          ref="input"
          v-model="address"
          class="sv__input"
          type="url"
          inputmode="url"
          autocomplete="url"
          autocapitalize="none"
          autocorrect="off"
          spellcheck="false"
          enterkeyhint="go"
          :aria-label="t('address')"
          :aria-invalid="!!error || undefined"
          :aria-describedby="error ? 'sv-error' : undefined"
          :placeholder="t('placeholder')"
        >
        <p v-if="error" id="sv-error" class="sv__error" role="alert">{{ t(`errors.${error}`) }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="checking === address" :disabled="!address.trim() || !!checking">{{ t('continue') }}</UiButton>
      </form>

      <ul v-if="recent.length" class="rc" :aria-label="t('recent')">
        <li v-for="s in recent" :key="s.url" class="rc__item">
          <button type="button" class="rc__pick" :disabled="!!checking" @click="check(s.url)">
            <span class="rc__name ellipsis">{{ s.name || host(s.url) }}</span>
            <span v-if="s.name" class="rc__host ellipsis">{{ host(s.url) }}</span>
          </button>
          <UiIconButton icon="ph:x" size="md" :label="t('forget', { name: s.name || host(s.url) })" class="rc__forget" @click="forget(s.url)" />
        </li>
      </ul>

      <UiButton v-if="state?.demo && state.url !== state.demo" variant="ghost" size="lg" block :loading="checking === state.demo" :disabled="!!checking" @click="check(state.demo)">
        {{ t('demo') }}
      </UiButton>
      <UiButton v-if="changing" variant="ghost" size="lg" block @click="back">{{ t('cancel') }}</UiButton>
    </template>

    <template #foot>
      <NuxtLink to="/privacy" class="sv__foot">{{ t('privacy') }}</NuxtLink>
    </template>
  </AuthStage>
</template>

<style scoped>
.sv { display: flex; flex-direction: column; gap: 10px; width: 100%; text-align: left; }
.sv__input {
  width: 100%;
  height: 48px;
  padding: 0 18px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
  font-size: 16px;
  transition: box-shadow var(--dur-1), background-color var(--dur-1);
}
.sv__input::placeholder { color: var(--text-4); }
.sv__input:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.sv__input:focus-visible { outline: none; background: var(--fill-2); box-shadow: inset 0 0 0 1.5px var(--focus); }
.sv__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.sv__error { margin: -2px 0 2px; padding: 0 18px; font-size: var(--fs-caption); line-height: 1.4; color: var(--danger); }
.sv :deep(.btn) { margin-top: 6px; }

/* Недавние: строки-пилюли, нажимается вся строка; крестик убирает из списка */
.rc { display: flex; flex-direction: column; gap: 6px; width: 100%; margin: 8px 0 0; padding: 0; list-style: none; text-align: left; }
.rc__item { display: flex; align-items: center; gap: 4px; min-height: 52px; padding-right: 6px; border-radius: var(--r-3); background: var(--fill-1); }
.rc__pick { display: flex; flex: 1; flex-direction: column; justify-content: center; min-width: 0; min-height: 52px; padding: 6px 18px; border: 0; border-radius: inherit; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
.rc__pick:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.rc__pick:disabled { cursor: default; opacity: 0.6; }
.rc__name { font-size: 15px; font-weight: 600; color: var(--text-1); }
.rc__host { font-size: var(--fs-caption); color: var(--text-3); }
.rc__forget { flex: none; color: var(--text-3); }

.sv__foot { display: inline-flex; align-items: center; min-height: 44px; padding: 0 12px; color: var(--text-4); font-size: var(--fs-small); text-decoration: none; transition: color var(--dur-1); }
.sv__foot:hover { color: var(--text-2); }
.sv__foot:focus-visible { outline: none; border-radius: var(--r-2); box-shadow: 0 0 0 2px var(--focus); }

@media (pointer: fine) {
  .sv__input { height: 44px; font-size: 15px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Адрес сервера",
    "lead": "DalyWave играет музыку с вашего сервера. Введите его адрес — или загляните в демо с открытой музыкой",
    "change": "Другой сервер",
    "changeText": "На текущем сервере вы выйдете из аккаунта",
    "address": "Адрес сервера",
    "placeholder": "music.example.com",
    "continue": "Продолжить",
    "connect": "Подключиться",
    "other": "Другой адрес",
    "foundVersion": "{host} · версия {version}",
    "recent": "Недавние серверы",
    "forget": "Убрать «{name}» из недавних",
    "demo": "Попробовать демо",
    "cancel": "Отмена",
    "privacy": "Конфиденциальность",
    "errors": {
      "invalid": "Это не похоже на адрес. Например: music.example.com",
      "unreachable": "Сервер не отвечает. Проверьте адрес и интернет",
      "not_dalywave": "По этому адресу не DalyWave",
      "down": "Сервер на месте, но сейчас не готов. Попробуйте через пару минут",
      "unknown": "Что-то пошло не так. Попробуйте ещё раз"
    }
  },
  "uk": {
    "title": "Адреса сервера",
    "lead": "DalyWave грає музику з вашого сервера. Введіть його адресу — або зазирніть у демо з відкритою музикою",
    "change": "Інший сервер",
    "changeText": "На поточному сервері ви вийдете з облікового запису",
    "address": "Адреса сервера",
    "placeholder": "music.example.com",
    "continue": "Продовжити",
    "connect": "Підключитися",
    "other": "Інша адреса",
    "foundVersion": "{host} · версія {version}",
    "recent": "Нещодавні сервери",
    "forget": "Прибрати «{name}» з нещодавніх",
    "demo": "Спробувати демо",
    "cancel": "Скасувати",
    "privacy": "Конфіденційність",
    "errors": {
      "invalid": "Це не схоже на адресу. Наприклад: music.example.com",
      "unreachable": "Сервер не відповідає. Перевірте адресу та інтернет",
      "not_dalywave": "За цією адресою не DalyWave",
      "down": "Сервер на місці, але зараз не готовий. Спробуйте за кілька хвилин",
      "unknown": "Щось пішло не так. Спробуйте ще раз"
    }
  },
  "en": {
    "title": "Server address",
    "lead": "DalyWave plays music from your own server. Enter its address — or look around the demo with open music",
    "change": "Another server",
    "changeText": "You'll be signed out of the current server",
    "address": "Server address",
    "placeholder": "music.example.com",
    "continue": "Continue",
    "connect": "Connect",
    "other": "Another address",
    "foundVersion": "{host} · version {version}",
    "recent": "Recent servers",
    "forget": "Remove “{name}” from recent",
    "demo": "Try the demo",
    "cancel": "Cancel",
    "privacy": "Privacy",
    "errors": {
      "invalid": "That doesn't look like an address. For example: music.example.com",
      "unreachable": "The server isn't responding. Check the address and the internet",
      "not_dalywave": "There's no DalyWave at this address",
      "down": "The server is there but not ready. Try again in a couple of minutes",
      "unknown": "Something went wrong. Try again"
    }
  }
}
</i18n>
