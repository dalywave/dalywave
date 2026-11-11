<script setup lang="ts">
// Вход есть, ролей нет: ждём, пока администратор выдаст доступ
definePageMeta({ layout: 'bare' })

const auth = useAuthStore()
const ui = useUiStore()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })

const checking = ref(false)

async function check() {
  checking.value = true
  try {
    await auth.load({ force: true })
    if (auth.status === 'ready') await navigateTo('/')
    else toast.show({ text: t('still'), icon: 'ph:hourglass' })
  }
  finally {
    checking.value = false
  }
}

async function logout() {
  await auth.logout().catch(() => {})
  await navigateTo('/login')
}
</script>

<template>
  <div class="wait">
    <div class="wait__wave"><MusicWaveCanvas :layout="ui.isPhone ? 'full-phone' : 'full'" /></div>
    <main class="wait__card">
      <img src="/favicon.svg" alt="" class="wait__icon" width="64" height="64">
      <h1 class="wait__title">{{ t('heading', { name: auth.user?.name?.split(' ')[0] ?? '' }) }}</h1>
      <p class="wait__text">{{ t('text') }}</p>
      <div class="wait__who">
        <span class="wait__avatar">{{ (auth.user?.name ?? '?').charAt(0).toUpperCase() }}</span>
        <span class="wait__whotext">
          <span class="wait__name">{{ auth.user?.name }}</span>
          <span class="wait__email">{{ auth.user?.email ?? t('noEmail') }}</span>
        </span>
      </div>
      <div class="wait__actions">
        <UiButton variant="primary" icon="ph:arrow-clockwise" :loading="checking" @click="check">{{ t('check') }}</UiButton>
        <UiButton variant="ghost" icon="ph:sign-out" @click="logout">{{ t('logout') }}</UiButton>
      </div>
    </main>
  </div>
</template>

<style scoped>
.wait { position: relative; display: grid; place-items: center; min-height: 100vh; min-height: 100dvh; padding: 32px 16px; overflow: hidden; background: #020509; }
.wait__wave { position: absolute; inset: 0; opacity: 0.7; }
.wait__wave::after { content: ''; position: absolute; inset: 0; background: radial-gradient(60% 55% at 50% 50%, rgba(2, 5, 9, 0.8), rgba(2, 5, 9, 0.4) 70%, rgba(2, 5, 9, 0.6)); }
.wait__card { position: relative; display: flex; flex-direction: column; align-items: center; width: min(440px, 100%); padding: 40px 32px 28px; border-radius: var(--r-6); background: rgba(10, 12, 16, 0.6); -webkit-backdrop-filter: blur(30px) saturate(1.4); backdrop-filter: blur(30px) saturate(1.4); box-shadow: inset 0 0 0 1px var(--line), var(--shadow-pop); text-align: center; }
.wait__icon { width: 64px; height: 64px; opacity: 0.9; filter: grayscale(0.2); }
.wait__title { margin: 18px 0 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); line-height: 1.15; }
.wait__text { max-width: 340px; margin: 10px 0 22px; color: var(--text-2); text-wrap: pretty; }
.wait__who { display: flex; align-items: center; gap: 12px; width: 100%; padding: 10px 12px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); text-align: left; }
.wait__avatar { display: grid; place-items: center; width: 40px; height: 40px; flex: none; border-radius: 50%; background: radial-gradient(120% 120% at 25% 15%, #4b5568, #222834); font-weight: 700; }
.wait__whotext { display: flex; flex-direction: column; min-width: 0; }
.wait__name { font-weight: 600; }
.wait__email { overflow: hidden; font-size: var(--fs-small); color: var(--text-3); text-overflow: ellipsis; white-space: nowrap; }
.wait__actions { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; margin-top: 22px; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Ждём доступа",
    "heading": "{name}, вы почти внутри",
    "text": "Вход через daly.rest прошёл. Осталось, чтобы администратор DalyWave выдал вам роль — тогда откроется музыка",
    "noEmail": "почта не указана",
    "check": "Проверить ещё раз",
    "still": "Доступа пока нет — администратор ещё не выдал роль",
    "logout": "Выйти"
  },
  "en": {
    "title": "Waiting for access",
    "heading": "{name}, you're almost in",
    "text": "You signed in with daly.rest. Now a DalyWave admin needs to give you a role — then the music opens up",
    "noEmail": "no email",
    "check": "Check again",
    "still": "No access yet — the admin hasn't given you a role",
    "logout": "Sign out"
  }
}
</i18n>
