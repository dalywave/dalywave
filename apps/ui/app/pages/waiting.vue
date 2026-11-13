<script setup lang="ts">
// Вход есть, ролей нет: ждём, пока администратор выдаст доступ. Экран сам проверяет раз в полминуты
// (и при возвращении на вкладку) — кнопка «Проверить» нужна, только если ждать не хочется
definePageMeta({ layout: 'bare' })

const auth = useAuthStore()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const firstName = computed(() => auth.user?.name?.trim().split(/\s+/)[0] ?? '')
const account = computed(() => auth.user?.email || auth.user?.name || '')
const checking = ref(false)

async function refresh() {
  await auth.load({ force: true })
  if (auth.status === 'ready') await navigateTo('/')
}

async function check() {
  checking.value = true
  try {
    await refresh()
    if (auth.status !== 'ready') toast.show({ text: t('still'), icon: 'ph:hourglass' })
  }
  finally {
    checking.value = false
  }
}

let timer: ReturnType<typeof setInterval> | null = null
function onVisible() {
  if (document.visibilityState === 'visible') void refresh().catch(() => {})
}
onMounted(() => {
  timer = setInterval(() => {
    if (document.visibilityState === 'visible' && !checking.value) void refresh().catch(() => {})
  }, 30_000)
  document.addEventListener('visibilitychange', onVisible)
})
onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
  document.removeEventListener('visibilitychange', onVisible)
})

async function logout() {
  await auth.logout().catch(() => {})
  await navigateTo('/login')
}
</script>

<template>
  <AuthStage :title="firstName ? t('heading', { name: firstName }) : t('headingAnon')" :text="t('text')">
    <UiButton variant="primary" size="lg" block :loading="checking" @click="check">{{ t('check') }}</UiButton>
    <UiButton variant="ghost" size="lg" block @click="logout">{{ t('logout') }}</UiButton>
    <template v-if="account" #foot>
      <p class="who">{{ t('as', { account }) }}</p>
    </template>
  </AuthStage>
</template>

<style scoped>
.who { margin: 0; max-width: 100%; overflow: hidden; font-size: var(--fs-small); color: var(--text-4); text-overflow: ellipsis; white-space: nowrap; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Ждём доступа",
    "heading": "{name}, почти готово",
    "headingAnon": "Почти готово",
    "text": "Осталось дождаться, пока администратор откроет доступ. Экран обновится сам",
    "check": "Проверить сейчас",
    "still": "Доступа пока нет",
    "logout": "Выйти",
    "as": "Вы вошли как {account}"
  },
  "en": {
    "title": "Waiting for access",
    "heading": "{name}, almost there",
    "headingAnon": "Almost there",
    "text": "An admin just needs to give you access. This screen updates by itself",
    "check": "Check now",
    "still": "No access yet",
    "logout": "Sign out",
    "as": "Signed in as {account}"
  }
}
</i18n>
