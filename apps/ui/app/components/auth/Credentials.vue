<script setup lang="ts">
// Поля своего аккаунта для экранов входа: вход (логин и пароль), новый аккаунт (имя, логин, пароль —
// первый запуск и приглашение), новый пароль по ссылке сброса. Поля-пилюли без подписей над ними:
// подсказка — внутри поля, ошибка — одной строкой под полями
const props = withDefaults(defineProps<{
  mode: 'login' | 'create' | 'reset'
  submit: string
  busy?: boolean
  error?: string | null
  /** Сброс: логин уже есть — поле только для чтения (по нему менеджер паролей узнает аккаунт) */
  username?: string | null
}>(), { busy: false, error: null, username: null })

const emit = defineEmits<{ submit: [value: { name: string, username: string, password: string }] }>()
const { t } = useI18n({ useScope: 'local' })

const name = ref('')
const login = ref(props.username ?? '')
const password = ref('')
watch(() => props.username, (v) => {
  if (v) login.value = v
})

const form = ref<HTMLFormElement | null>(null)
// Фокус — в первое пустое поле (на сбросе логин уже заполнен)
defineExpose({ focus: () => form.value?.querySelector<HTMLInputElement>('input:not([readonly])')?.focus() })

const ready = computed(() => !!login.value.trim() && !!password.value && !props.busy)
const newPassword = computed(() => props.mode !== 'login')

function onSubmit() {
  if (!ready.value) return
  emit('submit', { name: name.value.trim(), username: login.value.trim(), password: password.value })
}
</script>

<template>
  <form ref="form" class="cr" novalidate @submit.prevent="onSubmit">
    <input
      v-if="mode === 'create'"
      v-model="name"
      class="cr__input"
      type="text"
      autocomplete="name"
      :aria-label="t('name')"
      :placeholder="t('name')"
      maxlength="128"
    >
    <input
      v-model="login"
      class="cr__input"
      type="text"
      autocomplete="username"
      autocapitalize="none"
      autocorrect="off"
      spellcheck="false"
      :readonly="!!username"
      :aria-label="t('username')"
      :placeholder="t('username')"
      maxlength="32"
    >
    <input
      v-model="password"
      class="cr__input"
      type="password"
      :autocomplete="newPassword ? 'new-password' : 'current-password'"
      :aria-label="t('password')"
      :aria-invalid="!!error || undefined"
      :aria-describedby="error ? 'cr-error' : newPassword ? 'cr-hint' : undefined"
      :placeholder="newPassword ? t('newPassword') : t('password')"
      maxlength="256"
    >
    <p v-if="error" id="cr-error" class="cr__error" role="alert">{{ error }}</p>
    <p v-else-if="newPassword" id="cr-hint" class="cr__hint">{{ t('hint') }}</p>
    <UiButton type="submit" variant="primary" size="lg" block :loading="busy" :disabled="!ready">{{ submit }}</UiButton>
  </form>
</template>

<style scoped>
.cr { display: flex; flex-direction: column; gap: 10px; width: 100%; text-align: left; }
.cr__input {
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
.cr__input::placeholder { color: var(--text-4); }
.cr__input:hover { box-shadow: inset 0 0 0 1px var(--line-strong); }
.cr__input:focus-visible { outline: none; background: var(--fill-2); box-shadow: inset 0 0 0 1.5px var(--focus); }
.cr__input[readonly] { color: var(--text-3); }
.cr__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.cr__error, .cr__hint { margin: -2px 0 2px; padding: 0 18px; font-size: var(--fs-caption); line-height: 1.4; }
.cr__error { color: var(--danger); }
.cr__hint { color: var(--text-3); }
.cr :deep(.btn) { margin-top: 6px; }

/* Компьютер: поля чуть ниже, как остальные пилюли */
@media (pointer: fine) {
  .cr__input { height: 44px; font-size: 15px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "name": "Имя",
    "username": "Логин",
    "password": "Пароль",
    "newPassword": "Пароль",
    "hint": "Не короче 10 символов"
  },
  "uk": {
    "name": "Ім’я",
    "username": "Логін",
    "password": "Пароль",
    "newPassword": "Пароль",
    "hint": "Щонайменше 10 символів"
  },
  "en": {
    "name": "Name",
    "username": "Username",
    "password": "Password",
    "newPassword": "Password",
    "hint": "At least 10 characters"
  }
}
</i18n>
