<script setup lang="ts">
import type { ResetInfo } from '~/api/types'
import { ApiError } from '~/api/types'

// Одноразовая ссылка от администратора: задать новый пароль (и логин, если его ещё нет).
// Прежние сессии человека сервер закрывает, а этот браузер сразу входит
definePageMeta({ layout: 'bare' })

const route = useRoute()
const auth = useAuthStore()
const bridge = useBridge()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const token = computed(() => String(route.params.token ?? ''))
const state = ref<'loading' | 'ready' | 'invalid' | 'down'>('loading')
const info = ref<ResetInfo | null>(null)

onMounted(async () => {
  try {
    info.value = await bridge.auth.resetInfo(token.value)
    state.value = 'ready'
  }
  catch (e) {
    state.value = e instanceof ApiError && e.status === 404 ? 'invalid' : 'down'
  }
})

const busy = ref(false)
const error = ref<string | null>(null)

async function save(v: { username: string, password: string }) {
  busy.value = true
  error.value = null
  try {
    const username = info.value?.username ? null : v.username
    auth.signedIn(await bridge.auth.resetPassword(token.value, { password: v.password, username }))
    await navigateTo(auth.status === 'no-access' ? '/waiting' : '/')
  }
  catch (e) {
    if (e instanceof ApiError && e.code === 'reset_invalid') state.value = 'invalid'
    else if (e instanceof ApiError && e.code === 'username_taken') error.value = t('taken')
    else if (e instanceof ApiError && e.code === 'weak_password') error.value = t('weak')
    else if (e instanceof ApiError && e.code === 'bad_username') error.value = t('badUsername')
    else if (e instanceof ApiError && e.status === 403) error.value = t('disabled')
    else error.value = t('failed')
  }
  finally {
    busy.value = false
  }
}

const heading = computed(() => (state.value === 'invalid' ? t('invalid.title') : state.value === 'down' ? t('down.title') : t('heading')))
const text = computed(() => {
  if (state.value === 'invalid') return t('invalid.text')
  if (state.value === 'down') return t('down.text')
  if (!info.value) return ''
  return info.value.username ? t('lead', { name: info.value.name }) : t('leadNew', { name: info.value.name })
})
</script>

<template>
  <AuthStage :title="heading" :text="text">
    <AuthCredentials
      v-if="state === 'ready' && info"
      mode="reset"
      :username="info.username"
      :submit="t('submit')"
      :busy="busy"
      :error="error"
      @submit="save"
    />
    <UiButton v-else-if="state === 'invalid'" variant="secondary" size="lg" block to="/login">{{ t('toLogin') }}</UiButton>
  </AuthStage>
</template>

<i18n lang="json">
{
  "ru": {
    "title": "Новый пароль",
    "heading": "Новый пароль",
    "lead": "{name}, придумайте пароль — старый больше не подойдёт",
    "leadNew": "{name}, придумайте логин и пароль для входа",
    "submit": "Сохранить и войти",
    "taken": "Такой логин уже есть",
    "weak": "Пароль — не короче 10 символов",
    "badUsername": "Логин — 3–32 символа: буквы, цифры, точка, _ и -",
    "disabled": "Аккаунт отключён. Напишите администратору",
    "failed": "Не получилось. Проверьте связь с сервером",
    "toLogin": "К входу",
    "invalid": {
      "title": "Ссылка не действует",
      "text": "Её уже использовали или срок вышел. Попросите новую у администратора"
    },
    "down": {
      "title": "Сервер не отвечает",
      "text": "Скорее всего, это ненадолго. Попробуйте через минуту"
    }
  },
  "uk": {
    "title": "Новий пароль",
    "heading": "Новий пароль",
    "lead": "{name}, придумайте пароль — старий більше не підійде",
    "leadNew": "{name}, придумайте логін і пароль для входу",
    "submit": "Зберегти й увійти",
    "taken": "Такий логін уже є",
    "weak": "Пароль — не коротший за 10 символів",
    "badUsername": "Логін — 3–32 символи: літери, цифри, крапка, _ і -",
    "disabled": "Обліковий запис вимкнено. Напишіть адміністраторові",
    "failed": "Не вдалося. Перевірте зв’язок із сервером",
    "toLogin": "До входу",
    "invalid": {
      "title": "Посилання не діє",
      "text": "Його вже використали або термін минув. Попросіть нове в адміністратора"
    },
    "down": {
      "title": "Сервер не відповідає",
      "text": "Найімовірніше, це ненадовго. Спробуйте за хвилину"
    }
  },
  "en": {
    "title": "New password",
    "heading": "New password",
    "lead": "{name}, pick a new password — the old one stops working",
    "leadNew": "{name}, pick a username and password to sign in",
    "submit": "Save and sign in",
    "taken": "This username is taken",
    "weak": "The password must be at least 10 characters",
    "badUsername": "Username: 3–32 letters, digits, dot, _ and -",
    "disabled": "This account is disabled. Contact the administrator",
    "failed": "Something went wrong. Check the connection to the server",
    "toLogin": "To sign-in",
    "invalid": {
      "title": "This link doesn't work",
      "text": "It was used or has expired. Ask the administrator for a new one"
    },
    "down": {
      "title": "The server isn't responding",
      "text": "It's probably brief. Try again in a minute"
    }
  }
}
</i18n>
