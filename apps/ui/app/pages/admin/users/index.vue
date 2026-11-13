<script setup lang="ts">
import type { AuthOverview, Role, UserSummary } from '~/api/types'
import { ApiError } from '~/api/types'

// Пользователи: таблица с поиском, видами (все, в сети, ждут доступа, админы, заблокированные),
// фильтрами по роли и способу входа, сортировкой по колонкам и массовыми действиями. Строка ведёт
// на страницу человека. Новый свой аккаунт — кнопкой в шапке
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const ui = useUiStore()
const P = usePeopleLabels()
const { refresh: refreshCounts } = useAdminCounts()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => t('title') })
const { failed, attempt } = useAttempts()

const rows = ref<UserSummary[] | null>(null)
const overview = ref<AuthOverview | null>(null)
const load = () => attempt('users', () => bridge.admin.userTable(), d => (rows.value = d.items))
onMounted(() => {
  void load()
  void attempt('overview', () => bridge.admin.authOverview(), o => (overview.value = o))
})
// «В сети» меняется само: список перечитывается по событиям присутствия
const soon = useDebounced(() => void load(), 1500)
useAdminLive({ 'admin.presence': soon }, soon)

// ---------- Виды, фильтры, сортировка ----------

type View = 'all' | 'online' | 'waiting' | 'admins' | 'disabled'
const VIEWS: View[] = ['all', 'online', 'waiting', 'admins', 'disabled']
type SortKey = 'activity' | 'name' | 'hours' | 'created'
const STORE_KEY = 'dw-admin-users-view'
function restore(): { view: View, sort: SortKey, desc: boolean } {
  try {
    const raw = JSON.parse(localStorage.getItem(STORE_KEY) ?? 'null')
    if (raw && VIEWS.includes(raw.view)) return raw
  }
  catch {
    // Хранилище недоступно — вид по умолчанию
  }
  return { view: 'all', sort: 'activity', desc: true }
}
const saved = import.meta.client ? restore() : { view: 'all' as View, sort: 'activity' as SortKey, desc: true }
// Ссылка с «Обзора» открывает нужный вид: /admin/users?view=waiting
const route = useRoute()
const asked = VIEWS.find(v => v === route.query.view)
const view = ref<View>(asked ?? saved.view)
const sort = ref<SortKey>(saved.sort)
const desc = ref(saved.desc)
watch([view, sort, desc], () => {
  try {
    localStorage.setItem(STORE_KEY, JSON.stringify({ view: view.value, sort: sort.value, desc: desc.value }))
  }
  catch {
    // Приватное окно: вид просто не запомнится
  }
})
const q = ref('')
const role = ref<'' | Role | 'none'>('')
const method = ref('')

const inView = (r: UserSummary, v: View) => {
  const u = r.user
  switch (v) {
    case 'online': return r.online && !u.disabled
    case 'waiting': return !u.roles.length && !u.disabled
    case 'admins': return u.roles.includes('admin')
    case 'disabled': return u.disabled
    default: return true
  }
}
const viewCount = (v: View) => rows.value?.filter(r => inView(r, v)).length ?? 0
const methodOf = (r: UserSummary) => [...r.user.providers, ...(r.user.username && r.user.hasPassword ? ['local'] : [])]
const methodOptions = computed(() => {
  const all = new Set<string>()
  rows.value?.forEach(r => methodOf(r).forEach(m => all.add(m)))
  return [{ value: '', label: t('anyMethod') }, ...[...all].sort().map(m => ({ value: m, label: m === 'local' ? t('localMethod') : m }))]
})
const roleOptions = computed(() => [
  { value: '' as const, label: t('anyRole') },
  { value: 'admin' as const, label: P.role('admin') },
  { value: 'listener' as const, label: P.role('listener') },
  { value: 'none' as const, label: t('noRole') },
])
const sortOptions = computed(() => (['activity', 'name', 'hours', 'created'] as SortKey[]).map(s => ({ value: s, label: t(`sort.${s}`) })))

const seen = (r: UserSummary) => (r.online ? Number.MAX_SAFE_INTEGER : r.lastSeenAt ? new Date(r.lastSeenAt).getTime() : 0)
const shown = computed(() => {
  const needle = q.value.trim().toLowerCase()
  const list = (rows.value ?? []).filter((r) => {
    const u = r.user
    if (!inView(r, view.value)) return false
    if (role.value === 'none' ? u.roles.length > 0 : role.value && !u.roles.includes(role.value)) return false
    if (method.value && !methodOf(r).includes(method.value)) return false
    if (needle && ![u.name, u.email, u.username].some(x => x?.toLowerCase().includes(needle))) return false
    return true
  })
  const dir = desc.value ? -1 : 1
  const key = (r: UserSummary): number | string => {
    switch (sort.value) {
      case 'name': return r.user.name.toLowerCase()
      case 'hours': return r.listenedMs30d
      case 'created': return new Date(r.user.createdAt).getTime()
      default: return seen(r)
    }
  }
  return [...list].sort((a, b) => {
    const x = key(a)
    const y = key(b)
    return (x < y ? -1 : x > y ? 1 : 0) * dir
  })
})
function sortBy(k: SortKey) {
  if (sort.value === k) desc.value = !desc.value
  else {
    sort.value = k
    desc.value = k !== 'name'
  }
}
const sortIcon = (k: SortKey) => (sort.value === k ? (desc.value ? 'ph:caret-down' : 'ph:caret-up') : undefined)
const ariaSort = (k: SortKey) => (sort.value === k ? (desc.value ? 'descending' : 'ascending') : 'none')

// ---------- Выбор и массовые действия ----------

const picked = ref(new Set<string>())
const pickedRows = computed(() => shown.value.filter(r => picked.value.has(r.user.id)))
watch(shown, (list) => {
  const ids = new Set(list.map(r => r.user.id))
  for (const id of picked.value) if (!ids.has(id)) picked.value.delete(id)
})
function toggle(id: string) {
  const s = new Set(picked.value)
  if (s.has(id)) s.delete(id)
  else s.add(id)
  picked.value = s
}
const allPicked = computed(() => shown.value.length > 0 && shown.value.every(r => picked.value.has(r.user.id)))
function toggleAll() {
  picked.value = allPicked.value ? new Set() : new Set(shown.value.map(r => r.user.id))
}
const busy = ref(false)
async function bulk(kind: 'listener' | 'admin' | 'disable' | 'enable') {
  busy.value = true
  let ok = 0
  let refused = 0
  for (const r of pickedRows.value) {
    const u = r.user
    if (u.id === auth.user?.id && kind === 'disable') {
      refused++
      continue
    }
    try {
      if (kind === 'disable' || kind === 'enable') await bridge.admin.updateUser(u.id, { disabled: kind === 'disable' })
      else await bridge.admin.updateUser(u.id, { roles: [...new Set<Role>([...u.roles, kind, ...(kind === 'admin' ? ['listener' as const] : [])])] })
      ok++
    }
    catch {
      refused++
    }
  }
  busy.value = false
  picked.value = new Set()
  toast.show({ text: refused ? t('bulk.partial', { ok, refused }) : t('bulk.done', { n: ok }), icon: 'ph:users-three', tone: refused && !ok ? 'error' : 'default' })
  void load()
  void refreshCounts()
}

async function giveAccess(r: UserSummary) {
  try {
    await bridge.admin.updateUser(r.user.id, { roles: ['listener'] })
    r.user.roles = ['listener']
    toast.show({ text: t('given', { name: r.user.name }), icon: 'ph:user-check' })
    void refreshCounts()
  }
  catch {
    toast.error(t('failed'))
  }
}

// ---------- Новый аккаунт ----------

const link = ref<{ title: string, text: string, url: string } | null>(null)
const linkOpen = ref(false)
const newOpen = ref(false)
const newName = ref('')
const newLogin = ref('')
const newRole = ref<'listener' | 'admin' | 'none'>('listener')
const newBusy = ref(false)
const newError = ref<string | null>(null)
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
    void load()
    if (res.link) {
      link.value = { title: t('newUser.linkTitle'), text: t('newUser.linkText', { name: res.user.name }), url: res.link.url }
      linkOpen.value = true
    }
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
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <div class="ap-head__side">
        <UiButton v-if="overview?.local" variant="secondary" icon="ph:user-plus" @click="newOpen = true">{{ t('newUser.action') }}</UiButton>
        <UiButton variant="ghost" icon="ph:envelope-simple" to="/admin/access">{{ t('invites') }}</UiButton>
      </div>
    </header>

    <div class="ap-sec">
      <nav class="us__views" :aria-label="t('views')">
        <button
          v-for="v in VIEWS"
          :key="v"
          type="button"
          class="us__view"
          :class="{ 'is-active': view === v, 'is-warn': v === 'waiting' && viewCount(v) > 0 }"
          :aria-pressed="view === v"
          @click="view = v"
        >
          {{ t(`view.${v}`) }}<span v-if="rows" class="tnum">{{ viewCount(v) }}</span>
        </button>
      </nav>

      <div class="ap-filters us__filters">
        <input v-model="q" class="ap-input us__search" type="search" :placeholder="t('search')" :aria-label="t('search')">
        <AdminSelect v-model="role" :options="roleOptions" :label="t('role')" />
        <AdminSelect v-model="method" :options="methodOptions" :label="t('method')" />
        <AdminSelect v-if="ui.isPhone" v-model="sort" :options="sortOptions" :label="t('sortLabel')" />
      </div>
    </div>

    <AdminState :loading="!rows && !failed.users" :error="failed.users && !rows" :empty="!!rows && !shown.length" empty-icon="ph:users-three" :empty-title="t('empty')" :rows="6" @retry="load">
      <div class="ap-table us__table" role="table" :aria-label="t('title')">
        <div class="ap-tr ap-tr--head" role="row">
          <span role="columnheader" class="us__check">
            <input type="checkbox" :checked="allPicked" :aria-label="t('pickAll')" @change="toggleAll">
          </span>
          <button type="button" role="columnheader" class="us__sort" :aria-sort="ariaSort('name')" @click="sortBy('name')">{{ t('col.name') }}<Icon v-if="sortIcon('name')" :name="sortIcon('name')!" /></button>
          <span role="columnheader" class="ap-hide-phone">{{ t('col.roles') }}</span>
          <span role="columnheader" class="ap-hide-phone us__wide">{{ t('col.method') }}</span>
          <button type="button" role="columnheader" class="us__sort" :aria-sort="ariaSort('activity')" @click="sortBy('activity')">{{ t('col.status') }}<Icon v-if="sortIcon('activity')" :name="sortIcon('activity')!" /></button>
          <button type="button" role="columnheader" class="us__sort ap-num ap-hide-phone" :aria-sort="ariaSort('hours')" @click="sortBy('hours')">{{ t('col.hours') }}<Icon v-if="sortIcon('hours')" :name="sortIcon('hours')!" /></button>
          <button type="button" role="columnheader" class="us__sort ap-num ap-hide-phone us__wide" :aria-sort="ariaSort('created')" @click="sortBy('created')">{{ t('col.created') }}<Icon v-if="sortIcon('created')" :name="sortIcon('created')!" /></button>
        </div>
        <div v-for="r in shown" :key="r.user.id" class="ap-tr us__row" role="row" :class="{ 'is-picked': picked.has(r.user.id), 'is-disabled': r.user.disabled }">
          <span class="us__check" role="cell">
            <input type="checkbox" :checked="picked.has(r.user.id)" :aria-label="t('pick', { name: r.user.name })" @change="toggle(r.user.id)">
          </span>
          <NuxtLink :to="`/admin/users/${r.user.id}`" class="us__who" role="cell">
            <span class="us__avatar">
              <img v-if="r.user.avatarUrl" :src="r.user.avatarUrl" alt="">
              <span v-else>{{ r.user.name.charAt(0).toUpperCase() }}</span>
            </span>
            <span class="ap-cell">
              <b>{{ r.user.name }}<span v-if="r.user.id === auth.user?.id" class="us__you">{{ t('you') }}</span></b>
              <small>{{ r.user.email ?? (r.user.username ? `@${r.user.username}` : P.methods(r.user)) }}</small>
            </span>
          </NuxtLink>
          <span class="ap-hide-phone us__roles" role="cell">
            <template v-if="r.user.roles.length">{{ r.user.roles.map(P.role).join(' · ') }}</template>
            <UiButton v-else-if="!r.user.disabled" size="sm" variant="primary" icon="ph:user-check" @click="giveAccess(r)">{{ t('give') }}</UiButton>
            <span v-else class="ap-muted">—</span>
          </span>
          <span class="ap-hide-phone ap-muted us__wide us__ellipsis" role="cell">{{ P.methods(r.user) }}</span>
          <span role="cell">
            <AdminStatus :tone="P.status(r.user, r.online, r.lastSeenAt).tone" :pulse="r.online && !r.user.disabled">{{ P.status(r.user, r.online, r.lastSeenAt).text }}</AdminStatus>
          </span>
          <span class="ap-num ap-hide-phone" role="cell">{{ P.hours(r.listenedMs30d) }}</span>
          <span class="ap-num ap-muted ap-hide-phone us__wide" role="cell">{{ formatShortDate(r.user.createdAt, lang) }}</span>
        </div>
      </div>
    </AdminState>

    <Transition name="us-bulk">
      <div v-if="picked.size" class="us__bulk" role="toolbar" :aria-label="t('bulk.title')">
        <span class="us__bulk-count tnum">{{ t('bulk.picked', { n: picked.size }) }}</span>
        <UiButton size="sm" variant="secondary" icon="ph:headphones" :disabled="busy" @click="bulk('listener')">{{ t('bulk.listener') }}</UiButton>
        <UiButton size="sm" variant="secondary" icon="ph:shield-check" :disabled="busy" @click="bulk('admin')">{{ t('bulk.admin') }}</UiButton>
        <UiButton size="sm" variant="secondary" icon="ph:prohibit" :disabled="busy" @click="bulk('disable')">{{ t('bulk.disable') }}</UiButton>
        <UiButton size="sm" variant="secondary" icon="ph:arrow-counter-clockwise" :disabled="busy" @click="bulk('enable')">{{ t('bulk.enable') }}</UiButton>
        <UiIconButton icon="ph:x" size="sm" :label="t('bulk.clear')" @click="picked = new Set()" />
      </div>
    </Transition>

    <UiModal v-model:open="newOpen" :title="t('newUser.title')" :width="460">
      <form class="us__form" novalidate @submit.prevent="createUser">
        <input v-model="newName" class="us__input" type="text" maxlength="128" :placeholder="t('newUser.name')" :aria-label="t('newUser.name')">
        <input v-model="newLogin" class="us__input" type="text" maxlength="32" autocapitalize="none" spellcheck="false" :placeholder="t('newUser.username')" :aria-label="t('newUser.username')" :aria-invalid="!!newError || undefined">
        <UiSegmentedControl v-model="newRole" :options="[{ value: 'listener', label: P.role('listener') }, { value: 'admin', label: P.role('admin') }, { value: 'none', label: t('noRole') }]" :label="t('role')" block />
        <p v-if="newError" class="us__error" role="alert">{{ newError }}</p>
        <p v-else class="us__note">{{ t('newUser.note') }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="newBusy" :disabled="!newLogin.trim()">{{ t('newUser.submit') }}</UiButton>
      </form>
    </UiModal>
    <AdminLinkModal v-if="link" v-model:open="linkOpen" :title="link.title" :text="link.text" :url="link.url" />
  </div>
</template>

<style scoped>
.us__views { display: flex; gap: 4px; overflow-x: auto; scrollbar-width: none; }
.us__views::-webkit-scrollbar { display: none; }
.us__view { display: inline-flex; flex: none; align-items: center; gap: 8px; height: 34px; padding: 0 14px; border: 0; border-radius: var(--r-pill); background: none; color: var(--text-2); font: inherit; font-size: var(--fs-small); font-weight: 600; cursor: pointer; transition: color var(--dur-1), background-color var(--dur-1); }
.us__view span { font-size: var(--fs-caption); color: var(--text-3); }
.us__view:hover { background: var(--fill-1); color: var(--text-1); }
.us__view:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.us__view.is-active { background: var(--text-1); color: var(--on-light); }
.us__view.is-active span { color: inherit; opacity: 0.7; }
.us__view.is-warn:not(.is-active) span { color: var(--warning); font-weight: 700; }
.us__filters { margin-bottom: 0; }
.us__search { flex: 1 1 220px; max-width: 320px; }
.us__table { --cols: 28px minmax(200px, 1.6fr) minmax(110px, 0.9fr) minmax(110px, 0.9fr) minmax(130px, 1fr) 96px 88px; }
.us__check { display: grid; place-items: center; }
.us__check input { width: 16px; height: 16px; margin: 0; accent-color: var(--accent); cursor: pointer; }
.us__sort { display: inline-flex; align-items: center; gap: 4px; padding: 0; border: 0; background: none; color: inherit; font: inherit; cursor: pointer; }
.us__sort.ap-num { justify-content: flex-end; }
.us__sort:hover { color: var(--text-1); }
.us__sort:focus-visible { outline: none; border-radius: 4px; box-shadow: 0 0 0 2px var(--focus); }
.us__row { min-height: 60px; }
.us__row.is-picked { background: var(--fill-1); }
.us__row.is-disabled .us__who { opacity: 0.6; }
.us__who { display: flex; align-items: center; gap: 12px; min-width: 0; color: inherit; }
.us__who:hover b { text-decoration: underline; text-underline-offset: 3px; }
.us__who:focus-visible { outline: none; border-radius: var(--r-2); box-shadow: 0 0 0 2px var(--focus); }
.us__avatar { display: grid; place-items: center; width: 36px; height: 36px; flex: none; overflow: hidden; border-radius: 50%; background: var(--fill-2); font-weight: 700; color: var(--text-2); }
.us__avatar img { width: 100%; height: 100%; object-fit: cover; }
.us__you { margin-left: 8px; font-size: var(--fs-caption); font-weight: 500; color: var(--text-3); }
.us__roles { color: var(--text-2); }
.us__ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.us__bulk { position: sticky; bottom: 16px; z-index: 4; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; align-self: center; padding: 8px 8px 8px 16px; border-radius: var(--r-pill); background: var(--ink-3, var(--fill-3)); box-shadow: var(--shadow-3, 0 8px 32px rgba(0, 0, 0, 0.35)), inset 0 0 0 1px var(--line); }
.us__bulk-count { margin-right: 4px; font-size: var(--fs-small); font-weight: 650; }
.us-bulk-enter-active, .us-bulk-leave-active { transition: opacity var(--dur-2), transform var(--dur-2) var(--ease-out); }
.us-bulk-enter-from, .us-bulk-leave-to { opacity: 0; transform: translateY(12px); }
.us__form { display: flex; flex-direction: column; gap: 10px; padding: 0 20px 16px; }
.us__input { width: 100%; height: 44px; padding: 0 16px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.us__input::placeholder { color: var(--text-4); }
.us__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.us__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.us__error, .us__note { margin: 0; padding: 0 16px; font-size: var(--fs-caption); line-height: 1.4; }
.us__error { color: var(--danger); }
.us__note { color: var(--text-3); }
@media (max-width: 1099px) {
  .us__table { --cols: 28px minmax(180px, 1.6fr) minmax(100px, 0.9fr) minmax(130px, 1fr) 96px; }
  .us__wide { display: none !important; }
}
@media (max-width: 699px) {
  .us__table { --cols: 28px minmax(0, 1fr) auto; }
  .us__search { max-width: none; }
  .us__check input { width: 20px; height: 20px; }
  .us__bulk { bottom: calc(12px + var(--safe-bottom, 0px)); border-radius: var(--r-4); }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Пользователи",
    "invites": "Приглашения",
    "views": "Виды",
    "view": { "all": "Все", "online": "В сети", "waiting": "Ждут доступа", "admins": "Администраторы", "disabled": "Заблокированные" },
    "search": "Имя, почта или логин",
    "role": "Роль",
    "anyRole": "Любая роль",
    "noRole": "Без роли",
    "method": "Способ входа",
    "anyMethod": "Любой вход",
    "localMethod": "Логин и пароль",
    "sortLabel": "Порядок",
    "sort": { "activity": "По активности", "name": "По имени", "hours": "По часам", "created": "По регистрации" },
    "col": { "name": "Человек", "roles": "Роли", "method": "Вход", "status": "Статус", "hours": "Часы за 30 дн.", "created": "С нами с" },
    "you": "это вы",
    "give": "Выдать доступ",
    "given": "{name} получил доступ к музыке",
    "failed": "Не получилось — попробуйте ещё раз",
    "empty": "Никого не нашлось",
    "pick": "Выбрать: {name}",
    "pickAll": "Выбрать всех",
    "bulk": {
      "title": "Действия с выбранными",
      "picked": "Выбрано: {n}",
      "listener": "Выдать доступ",
      "admin": "Сделать админами",
      "disable": "Заблокировать",
      "enable": "Разблокировать",
      "clear": "Снять выбор",
      "done": "Готово: {n}",
      "partial": "Готово: {ok}, не получилось: {refused} — себя и последнего администратора не изменить"
    },
    "newUser": {
      "action": "Новый аккаунт",
      "title": "Новый аккаунт",
      "name": "Имя",
      "username": "Логин",
      "note": "Пароль человек задаст сам — по ссылке, которую вы ему передадите",
      "submit": "Создать",
      "taken": "Такой логин уже есть",
      "badUsername": "Логин — 3–32 символа: буквы, цифры, точка, _ и -",
      "linkTitle": "Ссылка для входа",
      "linkText": "Передайте её: {name} задаст пароль и войдёт. Ссылка одноразовая, действует 48 часов"
    }
  },
  "en": {
    "title": "Users",
    "invites": "Invites",
    "views": "Views",
    "view": { "all": "All", "online": "Online", "waiting": "Waiting", "admins": "Admins", "disabled": "Blocked" },
    "search": "Name, email or username",
    "role": "Role",
    "anyRole": "Any role",
    "noRole": "No role",
    "method": "Sign-in method",
    "anyMethod": "Any sign-in",
    "localMethod": "Username and password",
    "sortLabel": "Order",
    "sort": { "activity": "By activity", "name": "By name", "hours": "By hours", "created": "By sign-up" },
    "col": { "name": "Person", "roles": "Roles", "method": "Sign-in", "status": "Status", "hours": "Hours, 30 days", "created": "Joined" },
    "you": "you",
    "give": "Give access",
    "given": "{name} can now listen",
    "failed": "That didn't work — try again",
    "empty": "Nobody found",
    "pick": "Select {name}",
    "pickAll": "Select all",
    "bulk": {
      "title": "Actions with selected",
      "picked": "Selected: {n}",
      "listener": "Give access",
      "admin": "Make admins",
      "disable": "Block",
      "enable": "Unblock",
      "clear": "Clear selection",
      "done": "Done: {n}",
      "partial": "Done: {ok}, failed: {refused} — you and the last admin can't be changed"
    },
    "newUser": {
      "action": "New account",
      "title": "New account",
      "name": "Name",
      "username": "Username",
      "note": "The person sets the password themselves — with the link you pass on",
      "submit": "Create",
      "taken": "This username is taken",
      "badUsername": "Username: 3–32 letters, digits, dot, _ and -",
      "linkTitle": "Sign-in link",
      "linkText": "Pass it on: {name} sets a password and signs in. One-time link, valid for 48 hours"
    }
  }
}
</i18n>
