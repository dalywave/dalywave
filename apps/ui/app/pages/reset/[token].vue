<script setup lang="ts">
import type { ResetInfo } from '~/api/types'
import { ApiError } from '~/api/types'

// Одноразовая ссылка от администратора: задать новый пароль (и логин, если его ещё нет).
// Прежние сессии человека сервер закрывает, а этот браузер сразу входит
definePageMeta({ layout: 'bare' })

const route = useRoute()
const auth = useAuthStore()
const bridge = useBridge()
const { t } = useScopedI18n('pages.reset._token')
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
