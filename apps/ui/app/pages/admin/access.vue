<script setup lang="ts">
import type { AdminUser, Role, SessionInfo } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Доступ: люди и роли (ждущие доступа — первыми, выдача в одно нажатие), отключение,
// сессии сайта и приложения с завершением
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
onMounted(() => {
  void loadUsers()
  void loadSessions()
})

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
const via = (s: SessionInfo) => (s.via === 'password' ? t('sessions.viaPassword') : s.via === 'device' ? t('sessions.viaDevice') : 'daly.rest')
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminSection :title="t('people')" :hint="t('hint')">
      <template v-if="waiting.length" #badge><span class="ac__waiting tnum">{{ t('waiting', { n: waiting.length }) }}</span></template>
      <AdminState :loading="!users && !failed.users" :error="failed.users && !users" :rows="3" @retry="loadUsers">
        <div class="ap-table">
          <div v-for="u in users" :key="u.id" class="ac__user" :class="{ 'is-waiting': !u.roles.length && !u.disabled, 'is-disabled': u.disabled }">
            <span class="ac__avatar">
              <img v-if="u.avatarUrl" :src="u.avatarUrl" alt="">
              <span v-else>{{ u.name.charAt(0).toUpperCase() }}</span>
            </span>
            <span class="ap-cell ac__main">
              <b class="ac__name">{{ u.name }}<span v-if="u.id === auth.user?.id" class="ac__tag">{{ t('you') }}</span></b>
              <small>{{ [u.email, u.issuer ? 'daly.rest' : t('local'), u.lastLoginAt ? t('lastLogin', { when: formatRelative(u.lastLoginAt, lang) }) : null].filter(Boolean).join(' · ') }}</small>
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
            <span class="ac__where"><Icon :name="s.kind === 'device' ? 'ph:laptop' : 'ph:browser'" />{{ agent(s) }}</span>
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
    "hint": "Новые люди входят через daly.rest и ждут роль. Администраторы daly.rest — администраторы и здесь",
    "waiting": "ждут: {n}",
    "you": "это вы",
    "local": "локальный вход",
    "lastLogin": "входил {when}",
    "disabled": "Отключён",
    "give": "Выдать доступ",
    "actions": "Действия",
    "role": { "admin": "Администратор", "listener": "Слушатель" },
    "makeAdmin": "Сделать администратором",
    "removeAdmin": "Снять роль администратора",
    "idpAdmin": "выдана daly.rest",
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
      "viaPassword": "аварийный",
      "viaDevice": "приложение",
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
    "hint": "New people sign in with daly.rest and wait for a role. daly.rest admins are admins here too",
    "waiting": "waiting: {n}",
    "you": "you",
    "local": "local sign-in",
    "lastLogin": "signed in {when}",
    "disabled": "Disabled",
    "give": "Give access",
    "actions": "Actions",
    "role": { "admin": "Admin", "listener": "Listener" },
    "makeAdmin": "Make admin",
    "removeAdmin": "Remove admin role",
    "idpAdmin": "granted by daly.rest",
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
      "viaPassword": "emergency",
      "viaDevice": "app",
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
