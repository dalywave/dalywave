<script setup lang="ts">
import type { Health } from '~/api/types'
import type { TransitionMode } from '~/stores/player'

// Настройки: аккаунт, воспроизведение, волна, внешний вид, о приложении.
// Колонка по центру; каждая группа — <SettingsGroup> из строк <SettingsRow> (см. components/settings).
// Разделы других модулей встают между группами одной строкой — тем же набором кирпичей
const auth = useAuthStore()
const player = usePlayerStore()
const prefs = usePrefsStore()
const ui = useUiStore()
const bridge = useBridge()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()
const i18n = useNuxtApp().$i18n
useHead({ title: () => t('title') })

const health = ref<Health | null>(null)
const healthFailed = ref(false)
onMounted(async () => {
  try {
    health.value = await bridge.admin.health()
  }
  catch {
    healthFailed.value = true
  }
})

// ---------- Аккаунт ----------

const initial = computed(() => (auth.user?.name ?? '?').trim().charAt(0).toUpperCase())
const accountMeta = computed(() => [auth.user?.email, auth.isAdmin ? t('roleAdmin') : null].filter(Boolean).join(' · '))

async function logout() {
  await auth.logout().catch(() => toast.error(t('logoutFailed')))
  await navigateTo('/login')
}

// ---------- Воспроизведение ----------

const MODES: TransitionMode[] = ['gapless', 'smooth', 'smart', 'dj']
const hasExpressiveness = (m: TransitionMode) => m === 'smart' || m === 'dj'
const exprLabel = computed(() => (player.expressiveness < 0.34 ? tg('transition.subtle') : player.expressiveness < 0.67 ? tg('transition.set') : tg('transition.bold')))

// ---------- Волна ----------

const waveIsDefault = computed(() => player.wave.mood === 'any' && player.wave.activity === 'any' && player.wave.character === 'mix' && player.wave.language === 'any')
function resetWave() {
  player.setWave({ mood: 'any', activity: 'any', character: 'mix', language: 'any' })
  toast.show({ text: t('waveReset'), icon: 'ph:arrow-counter-clockwise' })
}

// ---------- Внешний вид ----------

const language = computed({
  get: () => (i18n.locale.value === 'en' ? 'en' : 'ru') as 'ru' | 'en',
  set: (v: 'ru' | 'en') => {
    void i18n.setLocale(v)
    document.documentElement.lang = v
    bridge.setLocale(v)
    try {
      localStorage.setItem('dw.locale', v)
    }
    catch {
      // Без хранилища язык вернётся к русскому при следующем запуске
    }
  },
})
</script>

<template>
  <div class="set">
    <h1 class="set__title">{{ t('title') }}</h1>

    <SettingsGroup :title="t('account')">
      <SettingsRow class="acct" :title="auth.user?.name" :description="accountMeta || undefined">
        <template #lead>
          <span class="acct__avatar">
            <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="">
            <span v-else>{{ initial }}</span>
          </span>
        </template>
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup :title="t('playback')">
      <SettingsRow :title="t('normalize')">
        <UiSwitch v-model="player.normalize" :label="t('normalize')" hide-label />
      </SettingsRow>
      <SettingsRow :title="t('scrubber')" stack>
        <MusicScrubberStylePicker v-model="prefs.scrubber" :label="t('scrubber')" />
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup :title="t('transitions')">
      <div role="radiogroup" :aria-label="t('transitions')" class="set__radios">
        <template v-for="m in MODES" :key="m">
          <SettingsRow
            as="button"
            radio
            :checked="player.transition === m"
            :description="tg(`transition.${m}Desc`)"
            @click="player.transition = m"
          >
            <template #title>
              {{ tg(`transition.${m}`) }}<span v-if="m === 'dj' && bridge.kind === 'web'" class="set__quiet">{{ tg('transition.inApp') }}</span>
            </template>
          </SettingsRow>
          <SettingsRow v-if="player.transition === m && hasExpressiveness(m)" indent>
            <div class="expr">
              <div class="expr__head"><span>{{ tg('transition.expressiveness') }}</span><span>{{ exprLabel }}</span></div>
              <UiSlider v-model="player.expressiveness" :label="tg('transition.expressiveness')" :value-text="exprLabel" />
            </div>
          </SettingsRow>
        </template>
      </div>
    </SettingsGroup>

    <SettingsGroup :title="t('wave')">
      <SettingsRow :title="t('waveRow')">
        <UiButton size="sm" :disabled="waveIsDefault" @click="resetWave">{{ t('waveResetBtn') }}</UiButton>
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup :title="t('appearance')">
      <SettingsRow :title="t('theme')">
        <UiSegmentedControl
          model-value="dark"
          :options="[{ value: 'dark', label: t('dark') }, { value: 'light', label: t('light'), disabled: true, hint: t('soon') }]"
          :label="t('theme')"
          :size="ui.isPhone ? 'sm' : 'md'"
        />
      </SettingsRow>
      <SettingsRow :title="t('language')">
        <UiSegmentedControl v-model="language" :options="[{ value: 'ru', label: 'Русский' }, { value: 'en', label: 'English' }]" :label="t('language')" :size="ui.isPhone ? 'sm' : 'md'" />
      </SettingsRow>
    </SettingsGroup>

    <!-- Сюда одной строкой встают разделы других модулей, например <DownloaderSettings /> -->
    <ShellDownloaderSettings />

    <SettingsGroup :title="t('about')">
      <SettingsRow :title="t('server')">
        <span class="status" :class="health ? 'status--ok' : healthFailed ? 'status--bad' : ''"><i />{{ health ? t('serverOk') : healthFailed ? t('serverDown') : '…' }}</span>
      </SettingsRow>
      <SettingsRow v-if="health" :title="t('version')">
        <span class="value tnum">{{ health.version }}</span>
      </SettingsRow>
    </SettingsGroup>

    <!-- Выход — отдельно в самом низу, как в Spotify и Apple Music -->
    <SettingsGroup :aria-label="t('logout')">
      <SettingsRow as="button" :title="t('logout')" danger @click="logout" />
    </SettingsGroup>
  </div>
</template>

<style scoped>
.set { display: flex; flex-direction: column; gap: 40px; width: 100%; max-width: 720px; margin: 0 auto; padding-top: 16px; }
.set__title { margin: 0 0 -8px; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }

/* Профиль: фото с daly.rest или буква на «Сумерках» */
.acct { --sr-lead: 44px; min-height: 68px; }
.acct__avatar { display: grid; place-items: center; width: 100%; height: 100%; overflow: hidden; border-radius: 50%; background: var(--brand-gradient); font-size: 18px; font-weight: 750; color: #fff; }
.acct__avatar img { width: 100%; height: 100%; object-fit: cover; }
.acct :deep(.sr__title) { font-size: var(--fs-h4); font-weight: 650; overflow-wrap: anywhere; }
.acct :deep(.sr__desc) { overflow-wrap: anywhere; }

.set__radios { display: contents; }
.set__quiet { font-size: var(--fs-caption); font-weight: 500; color: var(--text-3); }

/* Выразительность — под выбранным «Умным» или «Диджеем» */
.expr { display: flex; flex: 1; flex-direction: column; gap: 6px; padding-right: 32px; }
.expr__head { display: flex; justify-content: space-between; gap: 12px; font-size: var(--fs-caption); color: var(--text-3); }
.expr :deep(.sl__thumb) { transform: scale(1); }

.status { display: inline-flex; align-items: center; gap: 8px; font-size: var(--fs-small); color: var(--text-2); white-space: nowrap; }
.status i { width: 7px; height: 7px; border-radius: 50%; background: var(--text-4); }
.status--ok i { background: var(--success); }
.status--bad { color: var(--danger); }
.status--bad i { background: var(--danger); }
.value { font-size: var(--fs-small); color: var(--text-2); }

@media (max-width: 699px) {
  .set { gap: 32px; padding: 8px 16px 0; }
  .set__title { margin: 0 0 -4px; font-size: 28px; }
  .expr { padding-right: 0; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Настройки",
    "account": "Аккаунт",
    "roleAdmin": "Администратор",
    "logout": "Выйти",
    "logoutFailed": "Не получилось выйти — попробуйте ещё раз",
    "playback": "Воспроизведение",
    "normalize": "Выравнивать громкость",
    "scrubber": "Полоса прокрутки",
    "transitions": "Переходы между треками",
    "wave": "Моя волна",
    "waveRow": "Настройки волны",
    "waveResetBtn": "Сбросить",
    "waveReset": "Настройки волны сброшены",
    "appearance": "Внешний вид",
    "theme": "Тема",
    "dark": "Тёмная",
    "light": "Светлая",
    "soon": "Скоро",
    "language": "Язык",
    "about": "О приложении",
    "server": "Сервер",
    "serverOk": "Работает",
    "serverDown": "Не отвечает",
    "version": "Версия"
  },
  "en": {
    "title": "Settings",
    "account": "Account",
    "roleAdmin": "Admin",
    "logout": "Sign out",
    "logoutFailed": "Couldn't sign out — try again",
    "playback": "Playback",
    "normalize": "Normalize volume",
    "scrubber": "Progress bar",
    "transitions": "Transitions",
    "wave": "My Wave",
    "waveRow": "Wave settings",
    "waveResetBtn": "Reset",
    "waveReset": "Wave settings reset",
    "appearance": "Appearance",
    "theme": "Theme",
    "dark": "Dark",
    "light": "Light",
    "soon": "Soon",
    "language": "Language",
    "about": "About",
    "server": "Server",
    "serverOk": "Running",
    "serverDown": "Not responding",
    "version": "Version"
  }
}
</i18n>
