<script setup lang="ts">
import type { AdminUser, AuthOverview, Role, SessionInfo } from '~/api/types'
import { ApiError } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Доступ: люди и роли (ждущие доступа — первыми, выдача в одно нажатие), отключение, свои аккаунты
// (новый аккаунт, ссылка для нового пароля), приглашения, сессии сайта и приложения с завершением,
// способы входа (только чтение — настраиваются переменными окружения)
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const { refresh: refreshCounts } = useAdminCounts()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })
const { failed, attempt } = useAttempts()

const users = ref<AdminUser[] | null>(null)
const sessions = ref<SessionInfo[] | null>(null)
const loadUsers = () => attempt('users', () => bridge.admin.users(), l => (users.value = [...l.items].sort((a, b) => Number(a.roles.length > 0 || a.disabled) - Number(b.roles.length > 0 || b.disabled))))
const loadSessions = () => attempt('sessions', () => bridge.admin.sessions(), l => (sessions.value = l.items))
const overview = ref<AuthOverview | null>(null)
const loadOverview = () => attempt('overview', () => bridge.admin.authOverview(), o => (overview.value = o))
onMounted(() => {
  void loadUsers()
  void loadSessions()
  void loadOverview()
})

/** Как человек входит: провайдеры и свой логин */
function sources(u: AdminUser): string {
  const list = [...u.providers]
  if (u.username && u.hasPassword) list.push(t('login', { name: u.username }))
  return list.length ? list.join(', ') : t('local')
}

// ---------- Свои аккаунты: новый аккаунт и ссылка для нового пароля ----------
const link = ref<{ title: string, text: string, url: string } | null>(null)
const linkOpen = ref(false)
function showLink(title: string, text: string, url: string) {
  link.value = { title, text, url }
  linkOpen.value = true
}

const newOpen = ref(false)
const newName = ref('')
const newLogin = ref('')
const newRole = ref<'listener' | 'admin' | 'none'>('listener')
const newBusy = ref(false)
const newError = ref<string | null>(null)
const roleOptions = computed(() => [
  { value: 'listener' as const, label: t('role.listener') },
  { value: 'admin' as const, label: t('role.admin') },
  { value: 'none' as const, label: t('noRole') },
])
watch(newOpen, (v) => {
  if (!v) return
  newName.value = ''
  newLogin.value = ''
  newRole.value = 'listener'
  newError.value = null
})
async function createUser() {
  if (!newLogin.value.trim() || newBusy.value) return
  newBusy.value = true
  newError.value = null
  try {
    const roles: Role[] = newRole.value === 'admin' ? ['admin', 'listener'] : newRole.value === 'listener' ? ['listener'] : []
    const res = await bridge.admin.createUser({ name: newName.value.trim() || null, username: newLogin.value.trim(), roles })
    newOpen.value = false
    void loadUsers()
    if (res.link) showLink(t('newUser.linkTitle'), t('newUser.linkText', { name: res.user.name }), res.link.url)
  }
  catch (e) {
    newError.value = e instanceof ApiError && e.code === 'username_taken'
      ? t('newUser.taken')
      : e instanceof ApiError && e.code === 'bad_username' ? t('newUser.badUsername') : t('failed')
  }
  finally {
    newBusy.value = false
  }
}
async function resetLink(u: AdminUser) {
  try {
    const res = await bridge.admin.resetLink(u.id)
    showLink(t('reset.title'), t('reset.text', { name: u.name }), res.url)
  }
  catch {
    toast.error(t('failed'))
  }
}

const waiting = computed(() => users.value?.filter(u => !u.roles.length && !u.disabled) ?? [])

async function setRoles(u: AdminUser, roles: Role[], text: string) {
  const was = u.roles
  u.roles = roles
  try {
    await bridge.admin.updateUser(u.id, { roles })
    toast.show({ text, icon: 'ph:user-check', undo: () => void setRoles(u, was, t('restored')) })
    void refreshCounts()
  }
  catch {
    u.roles = was
    toast.error(t('failed'))
  }
}
async function setDisabled(u: AdminUser, disabled: boolean) {
  const was = u.disabled
  u.disabled = disabled
  try {
    await bridge.admin.updateUser(u.id, { disabled })
    toast.show({ text: disabled ? t('disabledDone', { name: u.name }) : t('enabledDone', { name: u.name }), icon: disabled ? 'ph:user-minus' : 'ph:user-plus', undo: () => void setDisabled(u, was) })
    void refreshCounts()
    if (disabled) void loadSessions()
  }
  catch {
    u.disabled = was
    toast.error(t('failed'))
  }
}
function userMenu(u: AdminUser): MenuEntry[] {
  const isAdmin = u.roles.includes('admin')
  const self = u.id === auth.user?.id
  return [
    { kind: 'heading', label: u.name, hint: u.email ?? undefined },
    isAdmin
      ? { label: t('removeAdmin'), icon: 'ph:shield-slash', disabled: u.idpAdmin || self, hint: u.idpAdmin ? t('idpAdmin') : self ? t('self') : undefined, onSelect: () => setRoles(u, u.roles.filter(r => r !== 'admin'), t('adminRemoved', { name: u.name })) }
      : { label: t('makeAdmin'), icon: 'ph:shield-check', onSelect: () => setRoles(u, [...new Set<Role>([...u.roles, 'admin'])], t('adminGiven', { name: u.name })) },
    u.roles.includes('listener')
      ? { label: t('removeListener'), icon: 'ph:headphones', disabled: self, onSelect: () => setRoles(u, u.roles.filter(r => r !== 'listener'), t('listenerRemoved', { name: u.name })) }
      : { label: t('makeListener'), icon: 'ph:headphones', onSelect: () => setRoles(u, [...u.roles, 'listener'], t('listenerGiven', { name: u.name })) },
    ...(overview.value?.local ? [{ label: u.hasPassword ? t('reset.action') : t('reset.actionNew'), icon: 'ph:key', onSelect: () => resetLink(u) }] : []),
    { kind: 'separator' },
    { label: u.disabled ? t('enable') : t('disable'), icon: u.disabled ? 'ph:user-plus' : 'ph:user-minus', danger: !u.disabled, disabled: self, onSelect: () => setDisabled(u, !u.disabled) },
  ]
}

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

    <AdminSection :title="t('people')" :hint="t('hint')">
      <template v-if="waiting.length" #badge><span class="ac__waiting tnum">{{ t('waiting', { n: waiting.length }) }}</span></template>
      <template v-if="overview?.local" #tools>
        <UiButton variant="secondary" size="sm" icon="ph:user-plus" @click="newOpen = true">{{ t('newUser.action') }}</UiButton>
      </template>
      <AdminState :loading="!users && !failed.users" :error="failed.users && !users" :rows="3" @retry="loadUsers">
        <div class="ap-table">
          <div v-for="u in users" :key="u.id" class="ac__user" :class="{ 'is-waiting': !u.roles.length && !u.disabled, 'is-disabled': u.disabled }">
            <span class="ac__avatar">
              <img v-if="u.avatarUrl" :src="u.avatarUrl" alt="">
              <span v-else>{{ u.name.charAt(0).toUpperCase() }}</span>
            </span>
            <span class="ap-cell ac__main">
              <b class="ac__name">{{ u.name }}<span v-if="u.id === auth.user?.id" class="ac__tag">{{ t('you') }}</span></b>
              <small>{{ [u.email, sources(u), u.lastLoginAt ? t('lastLogin', { when: formatRelative(u.lastLoginAt, lang) }) : null].filter(Boolean).join(' · ') }}</small>
            </span>
            <span class="ac__roles">
              <AdminStatus v-if="u.disabled" tone="bad">{{ t('disabled') }}</AdminStatus>
              <span v-else-if="u.roles.length" class="ac__role">
                <template v-for="(r, i) in u.roles" :key="r"><template v-if="i"> · </template>{{ t(`role.${r}`) }}<Icon v-if="r === 'admin' && u.idpAdmin" name="ph:link-simple" :title="t('idpAdmin')" class="ac__idp" /></template>
              </span>
              <UiButton v-else variant="primary" size="sm" icon="ph:user-check" @click="setRoles(u, ['listener'], t('listenerGiven', { name: u.name }))">{{ t('give') }}</UiButton>
            </span>
            <UiMenu :items="userMenu(u)">
              <UiIconButton icon="ph:dots-three" :label="t('actions')" size="sm" :tooltip="false" />
            </UiMenu>
          </div>
        </div>
      </AdminState>
    </AdminSection>

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
            <span class="ap-cell"><b class="ac__name">{{ s.userName }}<span v-if="s.current" class="ac__tag">{{ t('sessions.current') }}</span></b><small v-if="s.ip">{{ s.ip }}</small></span>
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

    <UiModal v-model:open="newOpen" :title="t('newUser.title')" :width="460">
      <form class="ac__form" novalidate @submit.prevent="createUser">
        <input v-model="newName" class="ac__input" type="text" maxlength="128" :placeholder="t('newUser.name')" :aria-label="t('newUser.name')">
        <input v-model="newLogin" class="ac__input" type="text" maxlength="32" autocapitalize="none" spellcheck="false" :placeholder="t('newUser.username')" :aria-label="t('newUser.username')" :aria-invalid="!!newError || undefined">
        <UiSegmentedControl v-model="newRole" :options="roleOptions" :label="t('newUser.role')" block />
        <p v-if="newError" class="ac__error" role="alert">{{ newError }}</p>
        <p v-else class="ac__note">{{ t('newUser.note') }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="newBusy" :disabled="!newLogin.trim()">{{ t('newUser.submit') }}</UiButton>
      </form>
    </UiModal>
    <AdminLinkModal v-if="link" v-model:open="linkOpen" :title="link.title" :text="link.text" :url="link.url" />
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
    "title": "Доступ",
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
    "title": "Access",
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
