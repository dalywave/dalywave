<script setup lang="ts">
import type { AuthOverview, SessionInfo } from '~/api/types'

// Приглашения и вход: приглашения, сессии сайта и приложения с завершением, способы входа (только
// чтение — настраиваются переменными окружения). Люди, роли и свои аккаунты — в «Пользователях»
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })
const { failed, attempt } = useAttempts()

const sessions = ref<SessionInfo[] | null>(null)
const loadSessions = () => attempt('sessions', () => bridge.admin.sessions(), l => (sessions.value = l.items))
const overview = ref<AuthOverview | null>(null)
const loadOverview = () => attempt('overview', () => bridge.admin.authOverview(), o => (overview.value = o))
onMounted(() => {
  void loadSessions()
  void loadOverview()
})

// ---------- Сессии ----------

async function revoke(s: SessionInfo) {
  try {
    await bridge.admin.revokeSession(s.id)
    sessions.value = sessions.value?.filter(x => x.id !== s.id) ?? null
    toast.show({ text: t('sessions.revoked', { name: s.userName }), icon: 'ph:sign-out' })
  }
  catch {
    toast.error(t('sessions.failed'))
  }
}
function agent(s: SessionInfo) {
  if (s.device) return `${s.device.name} · ${s.device.platform}${s.device.appVersion ? ` ${s.device.appVersion}` : ''}`
  const ua = s.userAgent ?? ''
  const browser = /Edg\//.test(ua) ? 'Edge' : /Chrome\//.test(ua) ? 'Chrome' : /Firefox\//.test(ua) ? 'Firefox' : /Safari\//.test(ua) ? 'Safari' : t('sessions.browser')
  const os = /Mac OS X/.test(ua) ? 'macOS' : /Windows/.test(ua) ? 'Windows' : /iPhone|iPad/.test(ua) ? 'iOS' : /Android/.test(ua) ? 'Android' : /Linux/.test(ua) ? 'Linux' : ''
  return [browser, os].filter(Boolean).join(' · ')
}
// Своя сессия — первой; длинный хвост старых вкладок сворачивается
const SESSIONS_SHOWN = 8
const showAllSessions = ref(false)
const orderedSessions = computed(() => [...(sessions.value ?? [])].sort((a, b) => Number(b.current) - Number(a.current)))
const shownSessions = computed(() => (showAllSessions.value ? orderedSessions.value : orderedSessions.value.slice(0, SESSIONS_SHOWN)))
const via = (s: SessionInfo) => t(`sessions.via.${s.via}`)
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminInvites />

    <AdminSection :title="t('sessions.title')" :hint="t('sessions.hint')">
      <AdminState :loading="!sessions && !failed.sessions" :error="failed.sessions && !sessions" :empty="!!sessions && !sessions.length" empty-icon="ph:devices" :empty-title="t('sessions.empty')" :rows="3" @retry="loadSessions">
        <div class="ap-table ac__sessions">
          <div class="ap-tr ap-tr--head">
            <span>{{ t('sessions.who') }}</span>
            <span>{{ t('sessions.where') }}</span>
            <span class="ap-hide-phone">{{ t('sessions.how') }}</span>
            <span class="ap-num ap-hide-phone">{{ t('sessions.active') }}</span>
            <span />
          </div>
          <div v-for="s in shownSessions" :key="s.id" class="ap-tr">
            <span class="ap-cell"><b class="ac__name"><NuxtLink :to="`/admin/users/${s.userId}`" class="ac__who">{{ s.userName }}</NuxtLink><span v-if="s.current" class="ac__tag">{{ t('sessions.current') }}</span></b><small v-if="s.ip">{{ s.ip }}</small></span>
            <span class="ac__where"><Icon :name="s.device?.platform === 'watchos' ? 'ph:watch' : s.device?.platform === 'tvos' ? 'ph:television-simple' : s.device?.platform === 'streamdeck' ? 'ph:squares-four' : s.kind === 'device' ? 'ph:laptop' : 'ph:browser'" />{{ agent(s) }}</span>
            <span class="ap-muted ap-hide-phone">{{ via(s) }}</span>
            <span class="ap-num ap-muted ap-hide-phone">{{ formatRelative(s.lastSeenAt, lang) }}</span>
            <span class="ap-end">
              <UiIconButton v-if="!s.current" icon="ph:sign-out" :label="t('sessions.revoke')" size="sm" @click="revoke(s)" />
            </span>
          </div>
          <div v-if="orderedSessions.length > SESSIONS_SHOWN" class="ap-more">
            <UiButton variant="ghost" size="sm" @click="showAllSessions = !showAllSessions">{{ showAllSessions ? t('sessions.less') : t('sessions.all', { n: orderedSessions.length }) }}</UiButton>
          </div>
        </div>
      </AdminState>
    </AdminSection>

    <AdminSection :title="t('methods.title')" :hint="t('methods.hint')">
      <AdminState :loading="!overview && !failed.overview" :error="failed.overview && !overview" :rows="2" @retry="loadOverview">
        <AdminSignInMethods v-if="overview" :overview="overview" />
      </AdminState>
    </AdminSection>

  </div>
</template>

<style scoped>
.ac__user { display: flex; align-items: center; gap: 14px; min-height: 64px; padding: 10px 0; border-top: 1px solid var(--hair); }
.ac__user:first-child { border-top: 0; }
.ac__user.is-waiting { margin: 0 -12px; padding-inline: 12px; border-radius: var(--r-3); background: var(--fill-1); }
.ac__user.is-disabled { opacity: 0.6; }
.ac__avatar { display: grid; place-items: center; width: 40px; height: 40px; flex: none; overflow: hidden; border-radius: 50%; background: var(--fill-2); font-weight: 700; color: var(--text-2); }
.ac__avatar img { width: 100%; height: 100%; object-fit: cover; }
.ac__main { flex: 1; }
.ac__name { display: flex; align-items: baseline; gap: 8px; }
.ac__who { color: inherit; }
.ac__who:hover { text-decoration: underline; text-underline-offset: 3px; }
.ac__tag { flex: none; font-size: var(--fs-caption); font-weight: 500; color: var(--text-3); }
.ac__waiting { font-size: var(--fs-small); font-weight: 600; color: var(--warning); }
.ac__role { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-small); color: var(--text-2); white-space: nowrap; }
.ac__idp { font-size: 14px; color: var(--text-3); }
.ac__roles { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; }
.ac__sessions { --cols: minmax(140px, 1fr) minmax(160px, 1.3fr) minmax(100px, 0.7fr) 120px 40px; }
.ac__where { display: inline-flex; align-items: center; gap: 8px; min-width: 0; overflow: hidden; color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.ac__where :deep(.iconify) { flex: none; font-size: 18px; color: var(--text-3); }
.ac__form { display: flex; flex-direction: column; gap: 10px; padding: 0 20px 16px; }
.ac__input {
  width: 100%;
  height: 44px;
  padding: 0 16px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
}
.ac__input::placeholder { color: var(--text-4); }
.ac__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.ac__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.ac__error, .ac__note { margin: 0; padding: 0 16px; font-size: var(--fs-caption); line-height: 1.4; }
.ac__error { color: var(--danger); }
.ac__note { color: var(--text-3); }
@media (max-width: 699px) {
  .ac__user { flex-wrap: wrap; }
  .ac__roles { order: 3; width: 100%; justify-content: flex-start; padding-left: 54px; }
  .ac__sessions { --cols: minmax(0, 1fr) minmax(0, 1fr) auto; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Приглашения и вход",
    "people": "Люди",
    "hint": "Новые люди ждут роль, пока вы её не выдадите. Администраторы провайдера входа — администраторы и здесь",
    "waiting": "ждут: {n}",
    "you": "это вы",
    "local": "аварийный вход",
    "login": "логин {name}",
    "noRole": "Без роли",
    "newUser": {
      "action": "Новый аккаунт",
      "title": "Новый аккаунт",
      "name": "Имя",
      "username": "Логин",
      "role": "Роль",
      "note": "Пароль человек задаст сам — по ссылке, которую вы ему передадите",
      "submit": "Создать",
      "taken": "Такой логин уже есть",
      "badUsername": "Логин — 3–32 символа: буквы, цифры, точка, _ и -",
      "linkTitle": "Ссылка для входа",
      "linkText": "Передайте её: {name} задаст пароль и войдёт. Ссылка одноразовая, действует 48 часов"
    },
    "reset": {
      "action": "Ссылка для нового пароля",
      "actionNew": "Ссылка, чтобы задать пароль",
      "title": "Ссылка для нового пароля",
      "text": "Передайте её: {name} задаст новый пароль, прежние сессии закроются. Ссылка одноразовая, действует 48 часов"
    },
    "methods": {
      "title": "Способы входа",
      "hint": "Настраиваются переменными окружения сервера"
    },
    "lastLogin": "входил {when}",
    "disabled": "Отключён",
    "give": "Выдать доступ",
    "actions": "Действия",
    "role": { "admin": "Администратор", "listener": "Слушатель" },
    "makeAdmin": "Сделать администратором",
    "removeAdmin": "Снять роль администратора",
    "idpAdmin": "выдана провайдером входа",
    "self": "это вы",
    "makeListener": "Выдать роль слушателя",
    "removeListener": "Снять роль слушателя",
    "disable": "Отключить",
    "enable": "Включить",
    "adminGiven": "{name} теперь администратор",
    "adminRemoved": "{name} больше не администратор",
    "listenerGiven": "{name} получил доступ к музыке",
    "listenerRemoved": "{name} больше не слушатель",
    "disabledDone": "{name} отключён: все его сессии закрыты",
    "enabledDone": "{name} снова может входить",
    "restored": "Роли возвращены",
    "failed": "Не получилось изменить доступ — попробуйте ещё раз",
    "sessions": {
      "title": "Сессии и устройства",
      "hint": "Вкладки сайта и приложения, где выполнен вход",
      "who": "Кто",
      "where": "Где",
      "how": "Вход",
      "active": "Активна",
      "current": "эта сессия",
      "browser": "Браузер",
      "via": { "oidc": "провайдер", "password": "аварийный", "device": "приложение", "local": "логин и пароль" },
      "revoke": "Завершить сессию",
      "all": "Показать все {n}",
      "less": "Свернуть",
      "revoked": "Сессия {name} завершена",
      "failed": "Не получилось завершить сессию",
      "empty": "Открытых сессий нет"
    }
  },
  "en": {
    "title": "Invites & sign-in",
    "people": "People",
    "hint": "New people wait for a role until you give one. Sign-in provider admins are admins here too",
    "waiting": "waiting: {n}",
    "you": "you",
    "local": "emergency sign-in",
    "login": "username {name}",
    "noRole": "No role",
    "newUser": {
      "action": "New account",
      "title": "New account",
      "name": "Name",
      "username": "Username",
      "role": "Role",
      "note": "The person sets the password themselves — with the link you pass on",
      "submit": "Create",
      "taken": "This username is taken",
      "badUsername": "Username: 3–32 letters, digits, dot, _ and -",
      "linkTitle": "Sign-in link",
      "linkText": "Pass it on: {name} sets a password and signs in. One-time link, valid for 48 hours"
    },
    "reset": {
      "action": "New password link",
      "actionNew": "Link to set a password",
      "title": "New password link",
      "text": "Pass it on: {name} sets a new password, old sessions end. One-time link, valid for 48 hours"
    },
    "methods": {
      "title": "Sign-in methods",
      "hint": "Configured with the server's environment variables"
    },
    "lastLogin": "signed in {when}",
    "disabled": "Disabled",
    "give": "Give access",
    "actions": "Actions",
    "role": { "admin": "Admin", "listener": "Listener" },
    "makeAdmin": "Make admin",
    "removeAdmin": "Remove admin role",
    "idpAdmin": "granted by the sign-in provider",
    "self": "that's you",
    "makeListener": "Give listener role",
    "removeListener": "Remove listener role",
    "disable": "Disable",
    "enable": "Enable",
    "adminGiven": "{name} is now an admin",
    "adminRemoved": "{name} is no longer an admin",
    "listenerGiven": "{name} can now listen",
    "listenerRemoved": "{name} is no longer a listener",
    "disabledDone": "{name} disabled: all sessions closed",
    "enabledDone": "{name} can sign in again",
    "restored": "Roles restored",
    "failed": "Couldn't change access — try again",
    "sessions": {
      "title": "Sessions and devices",
      "hint": "Website tabs and apps that are signed in",
      "who": "Who",
      "where": "Where",
      "how": "Sign-in",
      "active": "Active",
      "current": "this session",
      "browser": "Browser",
      "via": { "oidc": "provider", "password": "emergency", "device": "app", "local": "username" },
      "revoke": "End session",
      "all": "Show all {n}",
      "less": "Collapse",
      "revoked": "Session of {name} ended",
      "failed": "Couldn't end the session",
      "empty": "No open sessions"
    }
  }
}
</i18n>
