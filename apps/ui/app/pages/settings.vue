<script setup lang="ts">
import type { Health } from '~/api/types'
import type { Section } from '~/bridge/types'
import { waveSummary } from '~/utils/wave-options'

// Настройки: аккаунт, воспроизведение, внешний вид и язык, о приложении
const auth = useAuthStore()
const player = usePlayerStore()
const prefs = usePrefsStore()
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const i18n = useNuxtApp().$i18n
useHead({ title: () => t('title') })

const health = ref<Health | null>(null)
const healthFailed = ref(false)
const mockedSections = ref<Section[]>([])
onMounted(async () => {
  try {
    health.value = await bridge.admin.health()
  }
  catch {
    healthFailed.value = true
  }
  await bridge.ready()
  mockedSections.value = (['catalog', 'home', 'library', 'events', 'stream', 'wave', 'devices'] as Section[]).filter(s => bridge.mocked(s))
})

const via = computed(() => (auth.me?.via === 'password' ? t('viaPassword') : auth.me?.via === 'device' ? t('viaDevice') : t('viaOidc')))
const initial = computed(() => (auth.user?.name ?? '?').charAt(0).toUpperCase())
const uptime = computed(() => {
  const s = health.value?.uptimeS ?? 0
  if (s < 3600) return t('minutes', { n: Math.max(1, Math.round(s / 60)) })
  if (s < 86400) return t('hours', { n: Math.round(s / 3600) })
  return t('days', { n: Math.round(s / 86400) })
})

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

function resetWave() {
  player.setWave({ mood: 'any', activity: 'any', character: 'mix', language: 'any' })
  toast.show({ text: t('waveReset'), icon: 'ph:arrow-counter-clockwise' })
}

async function logout() {
  await auth.logout().catch(() => toast.error(t('logoutFailed')))
  await navigateTo('/login')
}
</script>

<template>
  <div class="set">
    <h1 class="set__title">{{ t('title') }}</h1>

    <section class="card" :aria-label="t('account')">
      <h2 class="card__title">{{ t('account') }}</h2>
      <div class="acct">
        <span class="acct__avatar">
          <img v-if="auth.user?.avatarUrl" :src="auth.user.avatarUrl" alt="">
          <span v-else>{{ initial }}</span>
        </span>
        <div class="acct__text">
          <div class="acct__name">{{ auth.user?.name }}</div>
          <div class="acct__meta">{{ auth.user?.email ?? t('noEmail') }} · {{ via }}</div>
          <div class="acct__roles">
            <UiBadge v-for="r in auth.roles" :key="r" :variant="r === 'admin' ? 'new' : 'meta'">{{ r === 'admin' ? t('roleAdmin') : t('roleListener') }}</UiBadge>
          </div>
        </div>
        <UiButton variant="ghost" icon="ph:sign-out" @click="logout">{{ t('logout') }}</UiButton>
      </div>
      <p class="card__note">{{ t('accountNote') }}</p>
    </section>

    <section class="card" :aria-label="t('playback')">
      <h2 class="card__title">{{ t('playback') }}</h2>
      <div class="row row--block">
        <div class="row__text">
          <div class="row__label">{{ t('transitions') }}</div>
          <div class="row__hint">{{ t('transitionsHint') }}</div>
        </div>
        <div class="pop pop--static set__modes">
          <MusicTransitionModeOptions v-model:mode="player.transition" v-model:expressiveness="player.expressiveness" :web="bridge.kind === 'web'" />
        </div>
      </div>
      <div class="row">
        <UiSwitch v-model="player.normalize" :label="t('normalize')" :hint="t('normalizeHint')" />
      </div>
      <div class="row">
        <div class="row__text">
          <div class="row__label">{{ t('scrubber') }}</div>
          <div class="row__hint">{{ t('scrubberHint') }}</div>
        </div>
        <MusicScrubberStylePicker v-model="prefs.scrubber" :label="t('scrubber')" />
      </div>
    </section>

    <section class="card" :aria-label="t('wave')">
      <h2 class="card__title">{{ t('wave') }}</h2>
      <div class="row">
        <div class="row__text">
          <div class="row__label">{{ t('waveNow') }}</div>
          <div class="row__hint">{{ waveSummary(player.wave, lang) }}</div>
        </div>
        <UiButton icon="ph:arrow-counter-clockwise" @click="resetWave">{{ t('waveResetBtn') }}</UiButton>
      </div>
    </section>

    <section class="card" :aria-label="t('appearance')">
      <h2 class="card__title">{{ t('appearance') }}</h2>
      <div class="row">
        <div class="row__text">
          <div class="row__label">{{ t('theme') }}</div>
          <div class="row__hint">{{ t('themeHint') }}</div>
        </div>
        <UiSegmentedControl
          model-value="dark"
          :options="[{ value: 'dark', label: t('dark'), icon: 'ph:moon' }, { value: 'light', label: t('light'), icon: 'ph:sun', disabled: true, hint: t('soon') }]"
          :label="t('theme')"
        />
      </div>
      <div class="row">
        <div class="row__text">
          <div class="row__label">{{ t('language') }}</div>
          <div class="row__hint">{{ t('languageHint') }}</div>
        </div>
        <UiSegmentedControl v-model="language" :options="[{ value: 'ru', label: 'Русский' }, { value: 'en', label: 'English' }]" :label="t('language')" />
      </div>
    </section>

    <section class="card" :aria-label="t('about')">
      <h2 class="card__title">{{ t('about') }}</h2>
      <dl class="facts">
        <dt>{{ t('server') }}</dt>
        <dd>
          <span v-if="health" class="ok"><Icon name="ph:check-circle-fill" />{{ t('serverOk', { version: health.version, uptime }) }}</span>
          <span v-else-if="healthFailed" class="bad"><Icon name="ph:warning-circle-fill" />{{ t('serverDown') }}</span>
          <span v-else>…</span>
        </dd>
        <dt>{{ t('device') }}</dt>
        <dd>{{ bridge.kind === 'tauri' ? t('deviceApp', { platform: bridge.platform }) : t('deviceWeb') }}</dd>
        <template v-if="mockedSections.length">
          <dt>{{ t('mocks') }}</dt>
          <dd class="warn">{{ t('mocksText', { list: mockedSections.join(', ') }) }}</dd>
        </template>
      </dl>
    </section>
  </div>
</template>

<style scoped>
.set { display: flex; flex-direction: column; gap: 20px; max-width: 760px; padding-top: 16px; }
.set__title { margin: 0 0 8px; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.card { display: flex; flex-direction: column; gap: 4px; padding: 20px 24px; border-radius: var(--r-5); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.card__title { margin: 0 0 8px; font-size: var(--fs-small); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.card__note { margin: 8px 0 0; font-size: var(--fs-caption); color: var(--text-3); }
.acct { display: flex; align-items: center; gap: 16px; }
.acct__avatar { display: grid; place-items: center; width: 56px; height: 56px; flex: none; overflow: hidden; border-radius: 50%; background: radial-gradient(120% 120% at 25% 15%, #4b5568, #222834); font-size: 20px; font-weight: 700; }
.acct__avatar img { width: 100%; height: 100%; object-fit: cover; }
.acct__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.acct__name { font-size: var(--fs-h4); font-weight: 650; }
.acct__meta { overflow: hidden; font-size: var(--fs-small); color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.acct__roles { display: flex; gap: 6px; margin-top: 4px; }
.row { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 12px 0; border-top: 1px solid var(--hair); }
.card__title + .row, .card__title + .acct { border-top: 0; }
.row--block { flex-direction: column; align-items: stretch; gap: 12px; }
.row__text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.row__label { font-weight: 600; }
.row__hint { font-size: var(--fs-small); color: var(--text-3); }
.set__modes { width: 100%; max-width: 420px; }
.facts { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 10px 24px; margin: 0; font-size: var(--fs-small); }
.facts dt { color: var(--text-3); }
.facts dd { margin: 0; }
.ok, .bad { display: inline-flex; align-items: center; gap: 6px; }
.ok :deep(svg) { color: var(--success); }
.bad { color: var(--danger); }
.warn { color: var(--warning); }
@media (max-width: 699px) {
  .set { padding: 8px 16px 0; }
  .set__title { font-size: 28px; }
  .card { padding: 16px; }
  .row { flex-direction: column; align-items: stretch; gap: 12px; }
  .acct { flex-wrap: wrap; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Настройки",
    "account": "Аккаунт",
    "noEmail": "почта не указана",
    "viaOidc": "вход через daly.rest",
    "viaPassword": "аварийный вход",
    "viaDevice": "вход приложения",
    "roleAdmin": "Администратор",
    "roleListener": "Слушатель",
    "logout": "Выйти",
    "logoutFailed": "Не получилось выйти — попробуйте ещё раз",
    "accountNote": "Имя, фото и способы входа меняются на daly.rest — здесь они обновятся при следующем входе",
    "playback": "Воспроизведение",
    "transitions": "Переходы между треками",
    "transitionsHint": "Действует для волны и списков",
    "normalize": "Выравнивать громкость",
    "normalizeHint": "Тихие и громкие треки звучат одинаково — по замеру громкости на сервере",
    "scrubber": "Полоса прокрутки",
    "scrubberHint": "Действует в\u00a0панели плеера, большом плеере и\u00a0мини-плеере",
    "wave": "Моя волна",
    "waveNow": "Сейчас",
    "waveResetBtn": "Сбросить",
    "waveReset": "Настройки волны сброшены: любое настроение, знакомое и новое",
    "appearance": "Внешний вид",
    "theme": "Тема",
    "themeHint": "Светлая тема появится позже",
    "dark": "Тёмная",
    "light": "Светлая",
    "soon": "Скоро",
    "language": "Язык интерфейса",
    "languageHint": "Названия треков и артистов не переводятся",
    "about": "О приложении",
    "server": "Сервер",
    "serverOk": "Работает · версия {version} · {uptime} без перезапуска",
    "serverDown": "Не отвечает",
    "minutes": "{n} мин",
    "hours": "{n} ч",
    "days": "{n} дн.",
    "device": "Это устройство",
    "deviceWeb": "Сайт в браузере",
    "deviceApp": "Приложение · {platform}",
    "mocks": "Макеты",
    "mocksText": "Разделы из design/fixtures, пока их нет на сервере: {list}"
  },
  "en": {
    "title": "Settings",
    "account": "Account",
    "noEmail": "no email",
    "viaOidc": "signed in with daly.rest",
    "viaPassword": "emergency sign-in",
    "viaDevice": "app sign-in",
    "roleAdmin": "Admin",
    "roleListener": "Listener",
    "logout": "Sign out",
    "logoutFailed": "Couldn't sign out — try again",
    "accountNote": "Name, photo and sign-in methods are managed on daly.rest — they update here on your next sign-in",
    "playback": "Playback",
    "transitions": "Transitions",
    "transitionsHint": "For the wave and lists",
    "normalize": "Normalize volume",
    "normalizeHint": "Quiet and loud tracks play at the same level — measured on the server",
    "scrubber": "Progress bar",
    "scrubberHint": "Applies to the player bar, full player and mini player",
    "wave": "My Wave",
    "waveNow": "Now",
    "waveResetBtn": "Reset",
    "waveReset": "Wave settings reset: any mood, familiar and new",
    "appearance": "Appearance",
    "theme": "Theme",
    "themeHint": "The light theme comes later",
    "dark": "Dark",
    "light": "Light",
    "soon": "Soon",
    "language": "Interface language",
    "languageHint": "Track and artist names aren't translated",
    "about": "About",
    "server": "Server",
    "serverOk": "Running · version {version} · up for {uptime}",
    "serverDown": "Not responding",
    "minutes": "{n} min",
    "hours": "{n} h",
    "days": "{n} d",
    "device": "This device",
    "deviceWeb": "Website in a browser",
    "deviceApp": "App · {platform}",
    "mocks": "Mocks",
    "mocksText": "Sections from design/fixtures until the server has them: {list}"
  }
}
</i18n>
