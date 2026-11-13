<script setup lang="ts">
import type { Health } from '~/api/types'
import type { TransitionMode } from '~/stores/player'

// Настройки: аккаунт, воспроизведение, волна, внешний вид, о приложении.
// Колонка по центру; каждая группа — <SettingsGroup> из строк <SettingsRow> (см. components/settings).
// Разделы других модулей встают между группами одной строкой — тем же набором кирпичей
const auth = useAuthStore()
const player = usePlayerStore()
const prefs = usePrefsStore()
const sound = useSoundStore()
const account = useAccountSettings()
const offline = useOfflineStore()
const ui = useUiStore()
const focus = useFocusStore()
const bridge = useBridge()
const toast = useToast()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()
const i18n = useNuxtApp().$i18n
useHead({ title: () => t('title') })
// Название в верхней строке телефона — когда крупный заголовок ушёл под неё
const { y: scrollY } = useWindowScroll()

// Установщики Mac и Windows — на сайте; в приложениях вместо них автообновление
const appsOpen = ref(false)

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

const hasExpressiveness = (m: TransitionMode) => m === 'smart' || m === 'dj'
const exprLabel = computed(() => (player.expressiveness < 0.34 ? tg('transition.subtle') : player.expressiveness < 0.67 ? tg('transition.set') : tg('transition.bold')))

// ---------- Без нецензурного и качество — общие для всех устройств слушателя ----------

const hideExplicit = computed({
  get: () => account.settings.hideExplicit,
  set: (v: boolean) => void account.update({ hideExplicit: v }),
})

// ---------- Пространственное аудио: iPhone и iPad, режим выбирается в Пункте управления ----------

const spatialAvailable = bridge.kind === 'tauri' && (bridge.platform === 'ios' || bridge.platform === 'ipados')
const outputs = spatialAvailable ? useAudioOutputs() : null
const spatialOutput = computed(() => outputs?.state.value.outputs.find(o => o.current && o.spatial) ?? null)

const qualityWifi = computed({
  get: () => account.settings.qualityWifi,
  set: (v: 'high' | 'normal') => void account.update({ qualityWifi: v }),
})
const qualityCellular = computed({
  get: () => account.settings.qualityCellular,
  set: (v: 'normal' | 'saver') => void account.update({ qualityCellular: v }),
})
const qualityDownload = computed({
  get: () => account.settings.qualityDownload,
  set: (v: 'high' | 'normal') => void account.update({ qualityDownload: v }),
})
const segSize = computed(() => (ui.isPhone ? 'sm' : 'md'))

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
    <MusicPhoneNav :title="t('title')" :condensed="scrollY > 44" />
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

    <SettingsGroup :title="t('library')">
      <SettingsRow as="button" :title="t('import')" :description="t('importDesc')" @click="navigateTo('/import')">
        <Icon name="ph:caret-right" class="set__chev" aria-hidden="true" />
      </SettingsRow>
      <!-- Выгрузка медиатеки в файл: CSV, M3U8, JSON, ZIP -->
      <SettingsExport />
    </SettingsGroup>

    <!-- Друзья: что видно друзьям и приватная сессия -->
    <SettingsFriends />

    <SettingsGroup :title="t('playback')">
      <SettingsRow :title="t('normalize')">
        <UiSwitch v-model="player.normalize" :label="t('normalize')" hide-label />
      </SettingsRow>
      <MusicEqPicker v-if="sound.eqSupported" />
      <SettingsRow
        v-if="spatialAvailable"
        :title="t('spatial')"
        :description="spatialOutput ? t('spatialOnDesc', { name: spatialOutput.name }) : t('spatialDesc')"
      >
        <span v-if="spatialOutput" class="set__state">{{ t('spatialOn') }}</span>
      </SettingsRow>
      <SettingsRow :title="t('scrubber')" stack>
        <MusicScrubberStylePicker v-model="prefs.scrubber" :label="t('scrubber')" />
      </SettingsRow>
      <SettingsRow :title="t('explicit')" :description="t('explicitDesc')">
        <UiSwitch v-model="hideExplicit" :label="t('explicit')" hide-label />
      </SettingsRow>
    </SettingsGroup>

    <!-- Сразу за воспроизведением: тот же выбор, что у кнопки «Переходы» в плеере -->
    <SettingsGroup :title="t('transitions')">
      <div role="radiogroup" :aria-label="t('transitions')" class="set__radios">
        <template v-for="m in TRANSITION_MODES" :key="m">
          <SettingsRow
            as="button"
            radio
            :icon="TRANSITION_ICONS[m]"
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

    <SettingsGroup :title="t('quality')">
      <SettingsRow :title="t('qualityWifi')">
        <UiSegmentedControl v-model="qualityWifi" :options="[{ value: 'high', label: t('q.high') }, { value: 'normal', label: t('q.normal') }]" :label="t('qualityWifi')" :size="segSize" />
      </SettingsRow>
      <SettingsRow :title="t('qualityCellular')">
        <UiSegmentedControl v-model="qualityCellular" :options="[{ value: 'normal', label: t('q.normal') }, { value: 'saver', label: t('q.saver') }]" :label="t('qualityCellular')" :size="segSize" />
      </SettingsRow>
      <SettingsRow v-if="offline.available" :title="t('qualityDownload')">
        <UiSegmentedControl v-model="qualityDownload" :options="[{ value: 'high', label: t('q.high') }, { value: 'normal', label: t('q.normal') }]" :label="t('qualityDownload')" :size="segSize" />
      </SettingsRow>
    </SettingsGroup>

    <!-- Приложения: наушники перешли на другое устройство — музыка за ними -->
    <SettingsHandoff />

    <SettingsGroup :title="t('wave')">
      <SettingsRow as="button" :title="t('taste')" :description="t('tasteDesc')" @click="navigateTo('/taste')">
        <Icon name="ph:caret-right" class="set__chev" />
      </SettingsRow>
      <!-- iPhone и iPad: сочетание волны выбирается в настройках фокуса iOS -->
      <SettingsRow v-if="focus.available" :title="t('focus')" :description="t('focusDesc')">
        <UiSwitch :model-value="focus.enabled" :label="t('focus')" hide-label @update:model-value="focus.setEnabled" />
      </SettingsRow>
      <SettingsRow :title="t('waveRow')">
        <UiButton size="sm" :disabled="waveIsDefault" @click="resetWave">{{ t('waveResetBtn') }}</UiButton>
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup :title="t('appearance')">
      <!-- Тема — своя для устройства, как и «Плеер сбоку» -->
      <SettingsRow :title="t('theme')" :stack="ui.isPhone">
        <UiSegmentedControl
          v-model="prefs.theme"
          :options="[{ value: 'dark', label: t('themeDark') }, { value: 'light', label: t('themeLight') }, { value: 'system', label: t('themeSystem') }]"
          :label="t('theme')"
          :size="ui.isPhone ? 'sm' : 'md'"
          :block="ui.isPhone"
        />
      </SettingsRow>
      <SettingsRow :title="t('language')">
        <UiSegmentedControl v-model="language" :options="[{ value: 'ru', label: 'Русский' }, { value: 'en', label: 'English' }]" :label="t('language')" :size="ui.isPhone ? 'sm' : 'md'" />
      </SettingsRow>
      <!-- Живые обложки и видео «Открытий» — свои для устройства; «только по Wi-Fi» — где бывает сотовая -->
      <SettingsRow :title="t('motion')" :description="t('motionDesc')">
        <UiSwitch v-model="prefs.motion" :label="t('motion')" hide-label />
      </SettingsRow>
      <SettingsRow v-if="prefs.motion && (ui.isTouchUi || offline.available)" :title="t('motionWifi')">
        <UiSwitch v-model="prefs.motionWifiOnly" :label="t('motionWifi')" hide-label />
      </SettingsRow>
      <!-- Планшет: «Сейчас играет» колонкой справа, когда лежит горизонтально. Своя для устройства -->
      <SettingsRow v-if="ui.isTouch" :title="t('sidePlayer')" :description="t('sidePlayerDesc')">
        <UiSwitch v-model="prefs.sidePlayer" :label="t('sidePlayer')" hide-label />
      </SettingsRow>
    </SettingsGroup>

    <!-- Уведомления: push на этом устройстве, виды, тихие часы -->
    <SettingsNotifications />

    <!-- Сюда одной строкой встают разделы других модулей, например <DownloaderSettings /> -->
    <!-- Приложение: загрузки на это устройство (умный офлайн) -->
    <SettingsDownloads />

    <ShellDownloaderSettings />

    <!-- Приложение для компьютера: помогать учить рекомендации на этом компьютере -->
    <ShellTrainerSettings />

    <!-- Приложение для компьютера: статус «Слушает DalyWave» в Discord -->
    <SettingsDiscord />

    <!-- Приложение на Mac: заставка с волной -->
    <SettingsScreensaver />

    <!-- Плашка «Сейчас играет» для OBS: секретная ссылка, вид, предпросмотр -->
    <SettingsOverlay />

    <SettingsGroup :title="t('about')">
      <!-- Приложение для компьютера: версия и «Обновление готово — Перезапустить» -->
      <ShellAppUpdateRow />
      <template v-if="bridge.kind === 'web'">
        <SettingsRow as="button" :title="tg('nav.desktopApps')" :description="t('desktopAppsDesc')" @click="appsOpen = true">
          <Icon name="ph:caret-right" class="set__chev" aria-hidden="true" />
        </SettingsRow>
        <ShellDesktopApps v-model:open="appsOpen" />
      </template>
      <!-- Сервер — только когда с ним беда; остальное про него живёт в админке -->
      <SettingsRow v-if="healthFailed" :title="t('server')">
        <span class="status status--bad"><i />{{ t('serverDown') }}</span>
      </SettingsRow>
      <SettingsRow v-if="health && !bridge.isDesktopApp" :title="t('version')">
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
.acct__avatar { display: grid; place-items: center; width: 100%; height: 100%; overflow: hidden; border-radius: 50%; background: var(--fill-3); font-size: 18px; font-weight: 700; color: var(--text-1); }
.acct__avatar img { width: 100%; height: 100%; object-fit: cover; }
.acct :deep(.sr__title) { font-size: var(--fs-h4); font-weight: 650; overflow-wrap: anywhere; }
.acct :deep(.sr__desc) { overflow-wrap: anywhere; }

.set__radios { display: contents; }
.set__chev { font-size: 16px; color: var(--text-4); }
.set__quiet { font-size: var(--fs-caption); font-weight: 500; color: var(--text-3); }
.set__chev { font-size: 18px; color: var(--text-3); }

/* Выразительность — под выбранным «Умным» или «Диджеем» */
.expr { display: flex; flex: 1; flex-direction: column; gap: 6px; padding-right: 32px; padding-left: 38px; }
/* Значки режимов — как в листе плеера; выбранный — акцентом */
.set__radios :deep(.is-checked .sr__lead) { color: var(--accent); }
.expr__head { display: flex; justify-content: space-between; gap: 12px; font-size: var(--fs-caption); color: var(--text-3); }
.expr :deep(.sl__thumb) { transform: scale(1); }

.status { display: inline-flex; align-items: center; gap: 8px; font-size: var(--fs-small); color: var(--text-2); white-space: nowrap; }
.status i { width: 7px; height: 7px; border-radius: 50%; background: var(--text-4); }
.status--bad { color: var(--danger); }
.status--bad i { background: var(--danger); }
.value { font-size: var(--fs-small); color: var(--text-2); }
/* Состояние справа — как у эквалайзера («Выключен») */
.set__state { font-size: var(--fs-body); color: var(--text-2); white-space: nowrap; }

@media (max-width: 699px) {
  .set { gap: 32px; padding: 0 16px; }
  /* Строка «назад» — во всю ширину экрана, мимо полей колонки */
  .set > .pn { margin-right: -16px; margin-bottom: -32px; margin-left: -16px; }
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
    "library": "Медиатека",
    "import": "Перенести музыку",
    "importDesc": "Из Spotify, списком или своими файлами",
    "playback": "Воспроизведение",
    "normalize": "Выравнивать громкость",
    "spatial": "Пространственное аудио",
    "spatialDesc": "С AirPods: удерживайте громкость в Пункте управления",
    "spatialOnDesc": "{name}: режим — в Пункте управления, удерживая громкость",
    "spatialOn": "Включено",
    "scrubber": "Полоса прокрутки",
    "explicit": "Без нецензурного",
    "explicitDesc": "Треки с пометкой «E» не играют",
    "quality": "Качество",
    "qualityWifi": "По Wi-Fi",
    "qualityCellular": "По сотовой",
    "qualityDownload": "Загрузки",
    "q": { "high": "Высокое", "normal": "Обычное", "saver": "Экономия" },
    "transitions": "Переходы между треками",
    "wave": "Моя волна",
    "taste": "Мой вкус",
    "tasteDesc": "Что волна знает о вас и как это поправить",
    "waveRow": "Настройки волны",
    "focus": "Фокусирование",
    "focusDesc": "Волна под включённый режим фокусирования",
    "waveResetBtn": "Сбросить",
    "waveReset": "Настройки волны сброшены",
    "appearance": "Интерфейс",
    "theme": "Тема",
    "themeDark": "Тёмная",
    "themeLight": "Светлая",
    "themeSystem": "Как в системе",
    "language": "Язык",
    "sidePlayer": "Плеер сбоку",
    "sidePlayerDesc": "Горизонтально «Сейчас играет» — колонкой справа",
    "motion": "Видео вместо обложки",
    "motionDesc": "Кусок клипа без звука в плеере и в «Открытиях»",
    "motionWifi": "Только по Wi-Fi",
    "about": "О приложении",
    "desktopAppsDesc": "Mac, Windows и Stream Deck",
    "server": "Сервер",
    "serverDown": "Не отвечает",
    "version": "Версия"
  },
  "en": {
    "title": "Settings",
    "account": "Account",
    "roleAdmin": "Admin",
    "logout": "Sign out",
    "logoutFailed": "Couldn't sign out — try again",
    "library": "Library",
    "import": "Bring your music",
    "importDesc": "From Spotify, a list or your own files",
    "playback": "Playback",
    "normalize": "Normalize volume",
    "spatial": "Spatial Audio",
    "spatialDesc": "With AirPods: touch and hold volume in Control Center",
    "spatialOnDesc": "{name}: pick the mode in Control Center by holding volume",
    "spatialOn": "On",
    "scrubber": "Progress bar",
    "explicit": "No explicit content",
    "explicitDesc": "Tracks marked “E” won't play",
    "quality": "Audio quality",
    "qualityWifi": "On Wi-Fi",
    "qualityCellular": "On cellular",
    "qualityDownload": "Downloads",
    "q": { "high": "High", "normal": "Normal", "saver": "Data saver" },
    "transitions": "Transitions",
    "wave": "My Wave",
    "taste": "My taste",
    "tasteDesc": "What the wave knows about you and how to tune it",
    "waveRow": "Wave settings",
    "focus": "Focus",
    "focusDesc": "The wave follows the Focus you turn on",
    "waveResetBtn": "Reset",
    "waveReset": "Wave settings reset",
    "appearance": "Interface",
    "theme": "Theme",
    "themeDark": "Dark",
    "themeLight": "Light",
    "themeSystem": "System",
    "language": "Language",
    "sidePlayer": "Side player",
    "sidePlayerDesc": "In landscape, Now Playing stays on the right",
    "motion": "Video instead of artwork",
    "motionDesc": "A silent loop from the music video in the player and in Discover",
    "motionWifi": "Wi-Fi only",
    "about": "About",
    "desktopAppsDesc": "Mac, Windows and Stream Deck",
    "server": "Server",
    "serverDown": "Not responding",
    "version": "Version"
  }
}
</i18n>
