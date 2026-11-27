<script setup lang="ts">
import type { AuthProviders } from '~/api/types'
import { ApiError } from '~/api/types'

// Ссылка-приглашение от администратора: придумать логин и пароль (свои аккаунты) или войти через
// провайдера — новый аккаунт сразу получает роли приглашения. Ссылка одноразовая
definePageMeta({ layout: 'bare' })

const route = useRoute()
const auth = useAuthStore()
const bridge = useBridge()
const { t } = useScopedI18n('pages.invite._token')
useHead({ title: () => t('title') })

const token = computed(() => String(route.params.token ?? ''))
const state = ref<'loading' | 'ready' | 'invalid' | 'down'>('loading')
const providers = ref<AuthProviders | null>(null)

onMounted(async () => {
  try {
    const [, p] = await Promise.all([bridge.auth.inviteInfo(token.value), bridge.auth.providers()])
    providers.value = p
    state.value = 'ready'
  }
  catch (e) {
    state.value = e instanceof ApiError && e.status === 404 ? 'invalid' : 'down'
  }
})

const list = computed(() => providers.value?.providers ?? [])
const local = computed(() => !!providers.value?.local)
const busy = ref(false)
const error = ref<string | null>(null)

async function create(v: { name: string, username: string, password: string }) {
  busy.value = true
  error.value = null
  try {
    auth.signedIn(await bridge.auth.acceptInvite(token.value, { name: v.name || null, username: v.username, password: v.password, app: false }))
    await navigateTo(auth.status === 'no-access' ? '/waiting' : '/')
  }
  catch (e) {
    if (e instanceof ApiError && e.code === 'invite_invalid') state.value = 'invalid'
    else if (e instanceof ApiError && e.code === 'username_taken') error.value = t('taken')
    else if (e instanceof ApiError && e.code === 'weak_password') error.value = t('weak')
    else if (e instanceof ApiError && e.code === 'bad_username') error.value = t('badUsername')
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
  if (state.value === 'loading') return ''
  return local.value ? t('lead') : t('leadProviders')
})
</script>

<template>
  <AuthStage :title="heading" :text="text">
    <template v-if="state === 'ready'">
      <AuthCredentials v-if="local" mode="create" :submit="t('submit')" :busy="busy" :error="error" @submit="create" />
      <AuthOr v-if="local && list.length" />
      <AuthProviderList v-if="list.length" :providers="list" :primary="!local && list.length === 1" :invite="token" />
    </template>
    <UiButton v-else-if="state === 'invalid'" variant="secondary" size="lg" block to="/login">{{ t('toLogin') }}</UiButton>
  </AuthStage>
</template>
