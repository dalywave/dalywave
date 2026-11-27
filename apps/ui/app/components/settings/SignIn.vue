<script setup lang="ts">
import type { Account, LinkedIdentity } from '~/api/types'
import { ApiError } from '~/api/types'

// «Вход» в настройках: логин и пароль своего аккаунта, привязанные провайдеры (daly.rest, Google…).
// Группа видна, только когда в ней есть что делать: на сервере со входом через одного провайдера и без
// своих аккаунтов её нет. Привязать провайдера — только на сайте: вход идёт через браузер с cookie
const bridge = useBridge()
const route = useRoute()
const router = useRouter()
const toast = useToast()
const { t } = useScopedI18n('components.settings.SignIn')

const account = ref<Account | null>(null)
async function load() {
  account.value = await bridge.auth.account().catch(() => null)
}

onMounted(async () => {
  await load()
  // Возврат после привязки: /settings?linked=… или ?link_error=…
  const linked = route.query.linked
  const failed = route.query.link_error
  if (typeof linked === 'string') toast.show({ text: t('linked'), icon: 'ph:link' })
  if (typeof failed === 'string') toast.error(failed === 'linked_elsewhere' ? t('linkedElsewhere') : t('linkFailed'))
  if (linked !== undefined || failed !== undefined) {
    const query = { ...route.query }
    delete query.linked
    delete query.link_error
    void router.replace({ query })
  }
})

const canLink = computed(() => bridge.kind === 'web')
const visible = computed(() => {
  const a = account.value
  return !!a && (a.local || (canLink.value && a.linkable.length > 0) || a.identities.length > 1)
})

function linkUrl(id: string) {
  return bridge.auth.providerUrl(id, { link: true, next: '/settings' })
}

async function unlink(i: LinkedIdentity) {
  try {
    await bridge.auth.unlinkIdentity(i.id)
    await load()
  }
  catch (e) {
    toast.error(e instanceof ApiError && e.code === 'last_sign_in_method' ? t('lastMethod') : t('failed'))
  }
}

// ---------- Пароль ----------
const open = ref(false)
const current = ref('')
const password = ref('')
const username = ref('')
const busy = ref(false)
const error = ref<string | null>(null)
const needUsername = computed(() => !account.value?.username)
const ready = computed(() => !!password.value && (!account.value?.hasPassword || !!current.value) && (!needUsername.value || !!username.value.trim()))

watch(open, (v) => {
  if (!v) return
  current.value = ''
  password.value = ''
  username.value = ''
  error.value = null
})

async function save() {
  if (!ready.value || busy.value) return
  busy.value = true
  error.value = null
  try {
    await bridge.auth.changePassword({
      current: account.value?.hasPassword ? current.value : null,
      password: password.value,
      username: needUsername.value ? username.value.trim() : null,
    })
    open.value = false
    toast.show({ text: t('saved'), icon: 'ph:check' })
    await load()
  }
  catch (e) {
    if (e instanceof ApiError && e.status === 401) error.value = t('wrong')
    else if (e instanceof ApiError && e.code === 'weak_password') error.value = t('weak')
    else if (e instanceof ApiError && e.code === 'bad_username') error.value = t('badUsername')
    else if (e instanceof ApiError && e.code === 'username_taken') error.value = t('taken')
    else if (e instanceof ApiError && e.status === 429) error.value = t('rateLimited')
    else error.value = t('failed')
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <SettingsGroup v-if="visible && account" :title="t('title')">
    <template v-if="account.local">
      <SettingsRow v-if="account.username" :title="t('username')">
        <span class="si__value">{{ account.username }}</span>
      </SettingsRow>
      <SettingsRow :title="t('password')">
        <UiButton size="sm" variant="secondary" @click="open = true">{{ account.hasPassword ? t('change') : t('set') }}</UiButton>
      </SettingsRow>
    </template>
    <SettingsRow v-for="i in account.identities" :key="i.id" :title="i.title" :description="i.email ?? undefined">
      <UiButton size="sm" variant="ghost" @click="unlink(i)">{{ t('unlink') }}</UiButton>
    </SettingsRow>
    <template v-if="canLink">
      <SettingsRow v-for="p in account.linkable" :key="p.id" :title="p.title">
        <UiButton size="sm" variant="secondary" :href="linkUrl(p.id)">{{ t('link') }}</UiButton>
      </SettingsRow>
    </template>

    <UiModal v-model:open="open" :title="account.hasPassword ? t('changeTitle') : t('setTitle')">
      <form class="si__form" novalidate @submit.prevent="save">
        <input
          v-if="needUsername"
          v-model="username"
          class="si__input"
          type="text"
          autocomplete="username"
          autocapitalize="none"
          spellcheck="false"
          maxlength="32"
          :aria-label="t('username')"
          :placeholder="t('username')"
        >
        <input
          v-if="account.hasPassword"
          v-model="current"
          class="si__input"
          type="password"
          autocomplete="current-password"
          :aria-label="t('current')"
          :placeholder="t('current')"
        >
        <input
          v-model="password"
          class="si__input"
          type="password"
          autocomplete="new-password"
          maxlength="256"
          :aria-label="t('new')"
          :aria-invalid="!!error || undefined"
          :placeholder="t('new')"
        >
        <p v-if="error" class="si__error" role="alert">{{ error }}</p>
        <p v-else class="si__hint">{{ t('weak') }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="busy" :disabled="!ready">{{ t('save') }}</UiButton>
      </form>
    </UiModal>
  </SettingsGroup>
</template>

<style scoped>
.si__value { color: var(--text-3); font-size: var(--fs-small); }
.si__form { display: flex; flex-direction: column; gap: 10px; padding: 4px 20px 16px; }
.si__input {
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
}
.si__input::placeholder { color: var(--text-4); }
.si__input:focus-visible { outline: none; background: var(--fill-2); box-shadow: inset 0 0 0 1.5px var(--focus); }
.si__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.si__error, .si__hint { margin: -2px 0 4px; padding: 0 18px; font-size: var(--fs-caption); }
.si__error { color: var(--danger); }
.si__hint { color: var(--text-3); }
@media (pointer: fine) {
  .si__input { height: 44px; font-size: 15px; }
}
</style>
