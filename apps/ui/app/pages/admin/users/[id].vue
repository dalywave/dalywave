<script setup lang="ts">
import type { AuthOverview, Role, SessionInfo, UserDetail, UserHistoryItem, UserInvite } from '~/api/types'
import { ApiError } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Страница человека: профиль и действия (имя, роли, блокировка, ссылка для пароля, «выйти везде»,
// удаление), «сейчас слушает» живьём, статистика, история прослушиваний, входы, сессии,
// приглашения, медиатека и скачивания, которые он вызвал
definePageMeta({ layout: 'admin' })

const route = useRoute()
const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const P = usePeopleLabels()
const { refresh: refreshCounts } = useAdminCounts()
const { t } = useI18n({ useScope: 'local' })
const id = computed(() => String(route.params.id))
const { failed, attempt } = useAttempts()

const d = ref<UserDetail | null>(null)
const sessions = ref<SessionInfo[] | null>(null)
const overview = ref<AuthOverview | null>(null)
const missing = ref(false)
const user = computed(() => d.value?.summary.user ?? null)
useHead({ title: () => user.value?.name ?? t('title') })

async function load() {
  try {
    d.value = await bridge.admin.userDetail(id.value)
    failed.value = { ...failed.value, detail: false }
  }
  catch (e) {
    if (e instanceof ApiError && e.status === 404) missing.value = true
    else failed.value = { ...failed.value, detail: true }
  }
}
const loadSessions = () => attempt('sessions', () => bridge.admin.sessions(id.value), l => (sessions.value = l.items))
onMounted(() => {
  void load()
  void loadSessions()
  void loadHistory()
  void attempt('overview', () => bridge.admin.authOverview(), o => (overview.value = o))
})

// «Сейчас слушает» — живьём: событие присутствия этого человека перечитывает страницу
const now = useTicker()
const soon = useDebounced(() => void load(), 500)
useAdminLive({ 'admin.presence': (e) => { if (e.userId === id.value) soon() } }, soon)

const self = computed(() => user.value?.id === auth.user?.id)
const status = computed(() => (d.value ? P.status(d.value.summary.user, d.value.summary.online, d.value.summary.lastSeenAt) : null))

// ---------- Действия ----------

function conflictText(e: unknown) {
  if (e instanceof ApiError) {
    if (e.code === 'last_admin') return t('err.lastAdmin')
    if (e.code === 'cannot_demote_self' || e.code === 'cannot_disable_self' || e.code === 'cannot_delete_self') return t('err.self')
  }
  return t('err.failed')
}
async function patch(body: { roles?: Role[], disabled?: boolean, name?: string }, text: string, icon: string) {
  try {
    await bridge.admin.updateUser(id.value, body)
    toast.show({ text, icon })
    await load()
    void refreshCounts()
    if (body.disabled) void loadSessions()
    return true
  }
  catch (e) {
    toast.error(conflictText(e))
    return false
  }
}
function toggleRole(r: Role) {
  const u = user.value!
  const has = u.roles.includes(r)
  const roles = has ? u.roles.filter(x => x !== r) : [...u.roles, r]
  void patch({ roles }, t(has ? `roles.removed.${r}` : `roles.given.${r}`, { name: u.name }), has ? 'ph:shield-slash' : 'ph:shield-check')
}
const roleItems = computed<MenuEntry[]>(() => {
  const u = user.value
  if (!u || !d.value) return []
  const lockAdmin = u.roles.includes('admin') && (self.value || d.value.lastAdmin)
  return [
    { kind: 'heading', label: t('roles.title') },
    {
      label: P.role('admin'),
      checked: u.roles.includes('admin'),
      disabled: lockAdmin,
      hint: lockAdmin ? (self.value ? t('err.self') : t('err.lastAdminShort')) : u.idpAdmin ? t('roles.idp') : undefined,
      onSelect: () => toggleRole('admin'),
    },
    { label: P.role('listener'), checked: u.roles.includes('listener'), onSelect: () => toggleRole('listener') },
  ]
})
const blockLocked = computed(() => self.value || (!!d.value?.lastAdmin && !user.value?.disabled))
async function toggleBlock() {
  const u = user.value!
  await patch({ disabled: !u.disabled }, u.disabled ? t('block.undone', { name: u.name }) : t('block.done', { name: u.name }), u.disabled ? 'ph:user-check' : 'ph:prohibit')
}

const renameOpen = ref(false)
const newName = ref('')
const renameBusy = ref(false)
function openRename() {
  newName.value = user.value?.name ?? ''
  renameOpen.value = true
}
async function rename() {
  if (!newName.value.trim()) return
  renameBusy.value = true
  if (await patch({ name: newName.value.trim() }, t('rename.done'), 'ph:pencil-simple')) renameOpen.value = false
  renameBusy.value = false
}

const link = ref<string | null>(null)
const linkOpen = ref(false)
async function resetLink() {
  try {
    link.value = (await bridge.admin.resetLink(id.value)).url
    linkOpen.value = true
  }
  catch {
    toast.error(t('err.failed'))
  }
}

async function revokeAll() {
  try {
    const n = await bridge.admin.revokeUserSessions(id.value)
    toast.show({ text: t('sessions.allDone', { n }), icon: 'ph:sign-out' })
    void loadSessions()
  }
  catch (e) {
    toast.error(conflictText(e))
  }
}
async function revoke(s: SessionInfo) {
  try {
    await bridge.admin.revokeSession(s.id)
    sessions.value = sessions.value?.filter(x => x.id !== s.id) ?? null
    toast.show({ text: t('sessions.done'), icon: 'ph:sign-out' })
  }
  catch {
    toast.error(t('err.failed'))
  }
}

const delOpen = ref(false)
const delName = ref('')
const delBusy = ref(false)
const delError = ref<string | null>(null)
const delLocked = computed(() => self.value || !!d.value?.lastAdmin)
watch(delOpen, (v) => {
  if (v) {
    delName.value = ''
    delError.value = null
  }
})
async function remove() {
  const u = user.value!
  if (delName.value.trim() !== u.name.trim()) return
  delBusy.value = true
  try {
    await bridge.admin.deleteUser(u.id, delName.value.trim())
    delOpen.value = false
    toast.show({ text: t('delete.done', { name: u.name }), icon: 'ph:trash' })
    void refreshCounts()
    await navigateTo('/admin/users')
  }
  catch (e) {
    delError.value = conflictText(e)
  }
  finally {
    delBusy.value = false
  }
}

const moreItems = computed<MenuEntry[]>(() => {
  const u = user.value
  if (!u) return []
  return [
    { label: t('rename.action'), icon: 'ph:pencil-simple', onSelect: openRename },
    ...(overview.value?.local ? [{ label: u.hasPassword ? t('reset.action') : t('reset.actionNew'), icon: 'ph:key', onSelect: resetLink }] : []),
    { label: t('sessions.all'), icon: 'ph:sign-out', disabled: self.value, onSelect: revokeAll },
    { kind: 'separator' },
    { label: t('delete.action'), icon: 'ph:trash', danger: true, disabled: delLocked.value, hint: delLocked.value ? (self.value ? t('err.self') : t('err.lastAdminShort')) : undefined, onSelect: () => (delOpen.value = true) },
  ]
})

// ---------- Статистика ----------

const statCells = computed(() => {
  const s = d.value?.stats
  if (!s) return []
  return [
    { key: 'h7', value: P.hours(s.listenedMs7d) },
    { key: 'h30', value: P.hours(s.listenedMs30d) },
    { key: 'p30', value: formatNumber(s.plays30d, lang.value) },
    { key: 'total', value: formatNumber(s.playsTotal, lang.value) },
  ]
})
const days = computed(() => (d.value?.stats.days ?? []).map(x => ({
  day: x.day,
  values: { min: Math.round(x.listenedMs / 60_000) },
  title: `${formatShortDate(`${x.day}T12:00:00Z`, lang.value)}: ${P.hours(x.listenedMs)} · ${playsN(x.plays)}`,
})))
const playsN = (n: number) => (lang.value === 'en' ? `${n} ${n === 1 ? 'play' : 'plays'}` : `${n} ${plural(n, ['раз', 'раза', 'раз'])}`)
const genreMax = computed(() => Math.max(1, ...(d.value?.stats.genres ?? []).map(g => g.plays)))

// ---------- История ----------

const history = ref<UserHistoryItem[] | null>(null)
const historyCursor = ref<string | null>(null)
const historyMore = ref(false)
const loadHistory = () => attempt('history', () => bridge.admin.userHistory(id.value), (p) => {
  history.value = p.items
  historyCursor.value = p.nextCursor
})
async function moreHistory() {
  if (!historyCursor.value) return
  historyMore.value = true
  try {
    const p = await bridge.admin.userHistory(id.value, historyCursor.value)
    history.value = [...(history.value ?? []), ...p.items]
    historyCursor.value = p.nextCursor
  }
  catch {
    toast.error(t('err.failed'))
  }
  finally {
    historyMore.value = false
  }
}
const time = (iso: string) => new Intl.DateTimeFormat(lang.value === 'en' ? 'en-GB' : 'ru-RU', { hour: '2-digit', minute: '2-digit' }).format(new Date(iso))
const historyGroups = computed(() => {
  const out: { day: string, items: UserHistoryItem[] }[] = []
  for (const h of history.value ?? []) {
    const day = formatShortDate(h.startedAt, lang.value)
    const last = out.at(-1)
    if (last?.day === day) last.items.push(h)
    else out.push({ day, items: [h] })
  }
  return out
})

// ---------- Разное ----------

function agent(s: SessionInfo) {
  if (s.device) return `${s.device.name} · ${s.device.platform}${s.device.appVersion ? ` ${s.device.appVersion}` : ''}`
  const ua = s.userAgent ?? ''
  const browser = /Edg\//.test(ua) ? 'Edge' : /Chrome\//.test(ua) ? 'Chrome' : /Firefox\//.test(ua) ? 'Firefox' : /Safari\//.test(ua) ? 'Safari' : t('sessions.browser')
  const os = /Mac OS X/.test(ua) ? 'macOS' : /Windows/.test(ua) ? 'Windows' : /iPhone|iPad/.test(ua) ? 'iOS' : /Android/.test(ua) ? 'Android' : /Linux/.test(ua) ? 'Linux' : ''
  return [browser, os].filter(Boolean).join(' · ')
}
const inviteStatus = (i: UserInvite) => t(`invite.status.${i.status}`)
const ACQ_TONE: Record<string, StatusTone> = { ready: 'ok', failed: 'bad', unavailable: 'bad', disputed: 'warn', cancelled: 'idle' }
const L = useAdminLabels()
</script>

<template>
  <div class="ap">
    <NuxtLink to="/admin/users" class="ud__back"><Icon name="ph:arrow-left" />{{ t('back') }}</NuxtLink>

    <UiEmptyState v-if="missing" icon="ph:user-minus" :title="t('missing')" :text="t('missingText')">
      <UiButton to="/admin/users" icon="ph:arrow-left">{{ t('back') }}</UiButton>
    </UiEmptyState>
    <AdminState v-else :loading="!d && !failed.detail" :error="failed.detail && !d" :rows="5" @retry="load">
      <template v-if="d && user">
        <header class="ud__head">
          <span class="ud__avatar">
            <img v-if="user.avatarUrl" :src="user.avatarUrl" alt="">
            <span v-else>{{ user.name.charAt(0).toUpperCase() }}</span>
          </span>
          <div class="ud__titles">
            <h1 class="ap-title ud__name">{{ user.name }}<span v-if="self" class="ud__you">{{ t('you') }}</span></h1>
            <div class="ud__meta">
              <AdminStatus v-if="status" :tone="status.tone" :pulse="d.summary.online && !user.disabled">{{ status.text }}</AdminStatus>
              <span v-if="user.roles.length">{{ user.roles.map(P.role).join(' · ') }}</span>
              <span v-if="user.email" class="ap-muted">{{ user.email }}</span>
            </div>
          </div>
          <div class="ud__actions">
            <UiButton v-if="!user.roles.length && !user.disabled" variant="primary" icon="ph:user-check" @click="patch({ roles: ['listener'] }, t('roles.given.listener', { name: user.name }), 'ph:user-check')">{{ t('give') }}</UiButton>
            <UiMenu :items="roleItems">
              <UiButton variant="secondary" icon="ph:shield-check" icon-right="ph:caret-down">{{ t('roles.title') }}</UiButton>
            </UiMenu>
            <UiButton :variant="user.disabled ? 'secondary' : 'ghost'" :icon="user.disabled ? 'ph:user-check' : 'ph:prohibit'" :disabled="blockLocked" :title="blockLocked ? (self ? t('err.self') : t('err.lastAdminShort')) : undefined" @click="toggleBlock">{{ user.disabled ? t('block.undo') : t('block.action') }}</UiButton>
            <UiMenu :items="moreItems">
              <UiIconButton icon="ph:dots-three" :label="t('more')" :tooltip="false" />
            </UiMenu>
          </div>
        </header>

        <p v-if="user.disabled" class="ud__banner" role="status">
          <Icon name="ph:prohibit" />{{ d.disabledAt ? t('block.since', { when: formatRelative(d.disabledAt, lang) }) : t('block.now') }}
        </p>

        <AdminSection v-if="d.live" :title="t('live')">
          <AdminLivePerson :person="d.live" :now="now" bare />
        </AdminSection>

        <AdminSection :title="t('stats.title')">
          <div class="ap-stats" style="--n: 4">
            <div v-for="s in statCells" :key="s.key" class="ap-stat">
              <span class="ap-stat__value">{{ s.value }}</span>
              <span class="ap-stat__label">{{ t(`stats.${s.key}`) }}</span>
            </div>
          </div>
          <AdminBars :days="days" :series="[{ key: 'min', label: t('stats.minutes'), color: 'var(--accent)' }]" :label="t('stats.chart')" :height="96" />
        </AdminSection>

        <div class="ap-cols">
          <AdminSection :title="t('top.artists')">
            <ol v-if="d.stats.topArtists.length" class="ud__top">
              <li v-for="a in d.stats.topArtists" :key="a.id">
                <MusicCoverArt class="ud__thumb" :image="a.image" size="small" round fallback-icon="ph:user" />
                <NuxtLink :to="`/artist/${a.id}`" class="ud__top-name">{{ a.name }}</NuxtLink>
                <span class="ap-muted tnum">{{ playsN(a.plays) }}</span>
              </li>
            </ol>
            <p v-else class="ap-hint">{{ t('top.empty') }}</p>
          </AdminSection>
          <AdminSection :title="t('top.tracks')">
            <ol v-if="d.stats.topTracks.length" class="ud__top">
              <li v-for="x in d.stats.topTracks" :key="x.track.id">
                <MusicCoverArt class="ud__thumb" :image="x.track.album?.image" size="small" radius="var(--r-1)" />
                <span class="ap-cell"><b>{{ x.track.title }}</b><small>{{ artistNames(x.track.artists) }}</small></span>
                <span class="ap-muted tnum">{{ playsN(x.plays) }}</span>
              </li>
            </ol>
            <p v-else class="ap-hint">{{ t('top.empty') }}</p>
          </AdminSection>
        </div>

        <AdminSection v-if="d.stats.genres.length" :title="t('top.genres')">
          <ul class="ud__genres">
            <li v-for="g in d.stats.genres" :key="g.name">
              <span class="ud__genre-name">{{ g.name }}</span>
              <span class="ud__genre-bar"><i :style="{ width: `${(g.plays / genreMax) * 100}%` }" /></span>
              <span class="ap-muted tnum">{{ g.plays }}</span>
            </li>
          </ul>
        </AdminSection>

        <div class="ap-cols">
          <AdminSection :title="t('profile.title')">
            <dl class="ap-kv">
              <div><dt>{{ t('profile.created') }}</dt><dd>{{ formatDateTime(user.createdAt, lang) }}</dd></div>
              <div><dt>{{ t('profile.lastLogin') }}</dt><dd>{{ user.lastLoginAt ? formatRelative(user.lastLoginAt, lang) : '—' }}</dd></div>
              <div v-if="user.username"><dt>{{ t('profile.username') }}</dt><dd>{{ user.username }}<span class="ap-muted">{{ user.hasPassword ? '' : ` · ${t('profile.noPassword')}` }}</span></dd></div>
              <div v-for="i in d.identities" :key="i.id">
                <dt>{{ i.provider }}</dt>
                <dd>{{ i.email ?? '—' }}<span v-if="i.lastLoginAt" class="ap-muted"> · {{ formatRelative(i.lastLoginAt, lang) }}</span></dd>
              </div>
              <div v-if="!d.identities.length && !user.username"><dt>{{ t('profile.method') }}</dt><dd>{{ P.methods(user) }}</dd></div>
              <div v-if="d.nameLocked"><dt>{{ t('profile.name') }}</dt><dd class="ap-muted">{{ t('profile.nameLocked') }}</dd></div>
            </dl>
          </AdminSection>
          <AdminSection :title="t('library.title')">
            <dl class="ap-kv">
              <div><dt>{{ t('library.playlists') }}</dt><dd>{{ formatNumber(d.counts.playlists, lang) }}</dd></div>
              <div><dt>{{ t('library.liked') }}</dt><dd>{{ formatNumber(d.counts.likedTracks, lang) }}</dd></div>
              <div><dt>{{ t('library.albums') }}</dt><dd>{{ formatNumber(d.counts.savedAlbums, lang) }}</dd></div>
              <div><dt>{{ t('library.followed') }}</dt><dd>{{ formatNumber(d.counts.followedArtists, lang) }}</dd></div>
              <div><dt>{{ t('library.friends') }}</dt><dd>{{ formatNumber(d.counts.friends, lang) }}</dd></div>
              <div><dt>{{ t('library.imports') }}</dt><dd>{{ formatNumber(d.counts.imports, lang) }}</dd></div>
            </dl>
            <div v-if="d.followed.length" class="ud__follows">
              <NuxtLink v-for="a in d.followed" :key="a.id" :to="`/artist/${a.id}`" class="ud__follow" :title="a.name">
                <MusicCoverArt :image="a.image" size="small" round fallback-icon="ph:user" />
              </NuxtLink>
            </div>
          </AdminSection>
        </div>

        <AdminSection :title="t('sessions.title')" :hint="t('sessions.hint')">
          <template v-if="sessions?.length && !self" #tools>
            <UiButton size="sm" variant="secondary" icon="ph:sign-out" @click="revokeAll">{{ t('sessions.all') }}</UiButton>
          </template>
          <AdminState :loading="!sessions && !failed.sessions" :error="failed.sessions && !sessions" :rows="2" @retry="loadSessions">
            <p v-if="sessions && !sessions.length" class="ap-hint">{{ t('sessions.empty') }}</p>
            <div class="ap-table ud__sessions">
              <div v-for="s in sessions" :key="s.id" class="ap-tr">
                <span class="ud__where"><Icon :name="s.kind === 'device' ? 'ph:laptop' : 'ph:browser'" />{{ agent(s) }}</span>
                <span class="ap-muted ap-hide-phone">{{ s.ip ?? '' }}</span>
                <span class="ap-num ap-muted">{{ formatRelative(s.lastSeenAt, lang) }}</span>
                <span class="ap-end"><UiIconButton v-if="!s.current" icon="ph:sign-out" :label="t('sessions.revoke')" size="sm" @click="revoke(s)" /></span>
              </div>
            </div>
          </AdminState>
        </AdminSection>

        <div class="ap-cols">
          <AdminSection :title="t('invite.title')">
            <dl class="ap-kv">
              <div>
                <dt>{{ t('invite.came') }}</dt>
                <dd>
                  <template v-if="d.invitedBy">
                    <NuxtLink v-if="d.invitedBy.person" :to="`/admin/users/${d.invitedBy.person.id}`" class="ud__link">{{ d.invitedBy.person.name }}</NuxtLink>
                    <span v-else class="ap-muted">{{ t('invite.unknown') }}</span>
                    <span v-if="d.invitedBy.usedAt" class="ap-muted"> · {{ formatShortDate(d.invitedBy.usedAt, lang) }}</span>
                  </template>
                  <span v-else class="ap-muted">{{ t('invite.none') }}</span>
                </dd>
              </div>
              <div v-for="i in d.invitesSent" :key="i.id">
                <dt>{{ i.note || t('invite.sent') }} <span class="ap-muted">· {{ formatShortDate(i.createdAt, lang) }}</span></dt>
                <dd>
                  <NuxtLink v-if="i.person" :to="`/admin/users/${i.person.id}`" class="ud__link">{{ i.person.name }}</NuxtLink>
                  <span v-else class="ap-muted">{{ inviteStatus(i) }}</span>
                </dd>
              </div>
            </dl>
          </AdminSection>
          <AdminSection :title="t('acq.title')" :hint="t('acq.hint')">
            <dl class="ap-kv">
              <div><dt>{{ t('acq.total') }}</dt><dd>{{ formatNumber(d.acquisitions.total, lang) }}</dd></div>
              <div><dt>{{ t('acq.ready') }}</dt><dd class="ap-ok">{{ formatNumber(d.acquisitions.ready, lang) }}</dd></div>
              <div v-if="d.acquisitions.active"><dt>{{ t('acq.active') }}</dt><dd>{{ formatNumber(d.acquisitions.active, lang) }}</dd></div>
              <div v-if="d.acquisitions.failed"><dt>{{ t('acq.failed') }}</dt><dd class="ap-warn">{{ formatNumber(d.acquisitions.failed, lang) }}</dd></div>
            </dl>
            <div v-if="d.acquisitions.recent.length" class="ap-table">
              <div v-for="a in d.acquisitions.recent.slice(0, 5)" :key="a.id" class="ap-tr ud__acq">
                <span class="ap-cell"><b>{{ a.title }}</b><small>{{ a.artist }} · {{ L.reason(a.reason as never) }}</small></span>
                <AdminStatus :tone="ACQ_TONE[a.status] ?? 'on'">{{ L.acqStatus(a.status as never) }}</AdminStatus>
              </div>
            </div>
          </AdminSection>
        </div>

        <AdminSection :title="t('history.title')" :hint="t('history.hint')">
          <AdminState :loading="!history && !failed.history" :error="failed.history && !history" :empty="!!history && !history.length" empty-icon="ph:clock-counter-clockwise" :empty-title="t('history.empty')" :rows="4" @retry="loadHistory">
            <div class="ap-table">
              <template v-for="g in historyGroups" :key="g.day">
                <div class="ap-group">{{ g.day }}</div>
                <div v-for="h in g.items" :key="h.id" class="ap-tr ud__hist">
                  <span class="ap-muted tnum">{{ time(h.startedAt) }}</span>
                  <template v-if="h.private || !h.track">
                    <span class="ud__thumb ud__thumb--private"><Icon name="ph:eye-slash" /></span>
                    <span class="ap-cell"><b class="ap-muted">{{ t('history.private') }}</b></span>
                  </template>
                  <template v-else>
                    <MusicCoverArt class="ud__thumb" :image="h.track.album?.image" size="small" radius="var(--r-1)" />
                    <span class="ap-cell"><b>{{ h.track.title }}</b><small>{{ artistNames(h.track.artists) }}<template v-if="h.contextType"> · {{ t(`history.ctx.${h.contextType}`) }}</template></small></span>
                  </template>
                  <span class="ap-num ap-muted ap-hide-phone">{{ formatDuration(h.playedMs) }}<template v-if="h.endReason === 'skipped'"> · {{ t('history.skipped') }}</template></span>
                  <span class="ap-muted ap-hide-phone ud__ellipsis">{{ h.deviceName ?? '' }}</span>
                </div>
              </template>
              <div v-if="historyCursor" class="ap-more">
                <UiButton variant="ghost" size="sm" :loading="historyMore" @click="moreHistory">{{ t('history.more') }}</UiButton>
              </div>
            </div>
          </AdminState>
        </AdminSection>
      </template>
    </AdminState>

    <UiModal v-model:open="renameOpen" :title="t('rename.title')" :width="420">
      <form class="ud__form" @submit.prevent="rename">
        <input v-model="newName" class="ud__input" type="text" maxlength="128" autofocus :aria-label="t('rename.title')">
        <p class="ud__note">{{ t('rename.note') }}</p>
        <UiButton type="submit" variant="primary" size="lg" block :loading="renameBusy" :disabled="!newName.trim()">{{ t('rename.submit') }}</UiButton>
      </form>
    </UiModal>

    <UiModal v-model:open="delOpen" :title="t('delete.title')" :width="440">
      <form class="ud__form" @submit.prevent="remove">
        <p class="ud__warn">{{ t('delete.text', { name: user?.name ?? '' }) }}</p>
        <label class="ud__label" for="ud-del">{{ t('delete.type', { name: user?.name ?? '' }) }}</label>
        <input id="ud-del" v-model="delName" class="ud__input" type="text" autocomplete="off" spellcheck="false" autofocus :aria-invalid="!!delError || undefined">
        <p v-if="delError" class="ud__error" role="alert">{{ delError }}</p>
        <UiButton type="submit" variant="danger" size="lg" block icon="ph:trash" :loading="delBusy" :disabled="delName.trim() !== (user?.name ?? '').trim()">{{ t('delete.submit') }}</UiButton>
      </form>
    </UiModal>

    <AdminLinkModal v-if="link" v-model:open="linkOpen" :title="t('reset.title')" :text="t('reset.text', { name: user?.name ?? '' })" :url="link" />
  </div>
</template>

<style scoped>
.ud__back { display: inline-flex; align-items: center; gap: 6px; align-self: flex-start; margin-bottom: -24px; font-size: var(--fs-small); font-weight: 600; color: var(--text-2); }
.ud__back:hover { color: var(--text-1); }
.ud__head { display: flex; flex-wrap: wrap; align-items: center; gap: 16px 20px; }
.ud__avatar { display: grid; place-items: center; width: 72px; height: 72px; flex: none; overflow: hidden; border-radius: 50%; background: var(--fill-2); font-size: 28px; font-weight: 700; color: var(--text-2); }
.ud__avatar img { width: 100%; height: 100%; object-fit: cover; }
.ud__titles { display: flex; flex: 1 1 240px; flex-direction: column; gap: 8px; min-width: 0; }
.ud__name { display: flex; align-items: baseline; gap: 12px; overflow-wrap: anywhere; }
.ud__you { font-size: var(--fs-small); font-weight: 500; letter-spacing: 0; color: var(--text-3); }
.ud__meta { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 16px; font-size: var(--fs-small); color: var(--text-2); }
.ud__actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
.ud__banner { display: flex; align-items: center; gap: 10px; margin: 0; padding: 12px 16px; border-radius: var(--r-3); background: color-mix(in oklab, var(--danger) 12%, transparent); font-size: var(--fs-small); font-weight: 600; color: var(--danger); }
.ud__top { display: flex; flex-direction: column; margin: 0; padding: 0; list-style: none; }
.ud__top li { display: grid; grid-template-columns: 40px minmax(0, 1fr) auto; align-items: center; gap: 12px; min-height: 52px; border-top: 1px solid var(--hair); font-size: var(--fs-small); }
.ud__top li:first-child { border-top: 0; }
.ud__top-name { overflow: hidden; font-weight: 600; color: var(--text-1); text-overflow: ellipsis; white-space: nowrap; }
.ud__top-name:hover { text-decoration: underline; text-underline-offset: 3px; }
.ud__thumb { width: 40px; height: 40px; flex: none; }
.ud__thumb--private { display: grid; place-items: center; border-radius: var(--r-1); background: var(--fill-2); color: var(--text-3); }
.ud__genres { display: flex; flex-direction: column; gap: 10px; margin: 0; padding: 0; list-style: none; }
.ud__genres li { display: grid; grid-template-columns: minmax(100px, 200px) minmax(0, 1fr) 40px; align-items: center; gap: 12px; font-size: var(--fs-small); }
.ud__genre-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ud__genre-bar { height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-1); }
.ud__genre-bar i { display: block; height: 100%; border-radius: inherit; background: var(--chart-2, var(--accent)); }
.ud__genres .tnum { text-align: right; }
.ud__follows { display: flex; flex-wrap: wrap; gap: 8px; padding-top: 4px; }
.ud__follow { width: 36px; height: 36px; border-radius: 50%; }
.ud__follow:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.ud__sessions { --cols: minmax(0, 1.4fr) minmax(0, 0.8fr) 120px 40px; }
.ud__where { display: inline-flex; align-items: center; gap: 8px; min-width: 0; overflow: hidden; color: var(--text-1); text-overflow: ellipsis; white-space: nowrap; }
.ud__where :deep(.iconify) { flex: none; font-size: 18px; color: var(--text-3); }
.ud__link { font-weight: 600; color: var(--text-1); }
.ud__link:hover { text-decoration: underline; text-underline-offset: 3px; }
.ud__acq { --cols: minmax(0, 1fr) auto; }
.ud__hist { --cols: 44px 40px minmax(0, 1fr) 110px minmax(0, 0.5fr); }
.ud__ellipsis { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ud__form { display: flex; flex-direction: column; gap: 12px; padding: 0 20px 16px; }
.ud__input { width: 100%; height: 44px; padding: 0 16px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.ud__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.ud__input[aria-invalid='true'] { box-shadow: inset 0 0 0 1.5px var(--danger); }
.ud__note, .ud__label { margin: 0; padding: 0 4px; font-size: var(--fs-caption); line-height: 1.4; color: var(--text-3); }
.ud__label { color: var(--text-2); }
.ud__warn { margin: 0; font-size: var(--fs-body); line-height: 1.45; color: var(--text-2); text-wrap: pretty; }
.ud__error { margin: 0; padding: 0 4px; font-size: var(--fs-caption); color: var(--danger); }
@media (max-width: 699px) {
  .ud__back { margin-bottom: -16px; }
  .ud__avatar { width: 56px; height: 56px; font-size: 22px; }
  .ud__actions { width: 100%; }
  .ud__sessions { --cols: minmax(0, 1fr) auto 40px; }
  .ud__hist { --cols: 44px 40px minmax(0, 1fr); }
  .ud__genres li { grid-template-columns: minmax(80px, 120px) minmax(0, 1fr) 32px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Пользователь",
    "back": "Пользователи",
    "missing": "Такого пользователя нет",
    "missingText": "Возможно, аккаунт удалили",
    "you": "это вы",
    "more": "Ещё",
    "give": "Выдать доступ",
    "live": "Сейчас слушает",
    "roles": {
      "title": "Роли",
      "idp": "выдана провайдером входа",
      "given": { "admin": "{name} теперь администратор", "listener": "{name} получил доступ к музыке" },
      "removed": { "admin": "{name} больше не администратор", "listener": "{name} больше не слушатель" }
    },
    "block": {
      "action": "Заблокировать",
      "undo": "Разблокировать",
      "done": "{name} заблокирован: вход закрыт, сессии завершены",
      "undone": "{name} снова может входить",
      "since": "Заблокирован {when}: войти не может",
      "now": "Заблокирован: войти не может"
    },
    "rename": {
      "action": "Изменить имя",
      "title": "Имя",
      "note": "Провайдер входа больше не будет менять это имя",
      "submit": "Сохранить",
      "done": "Имя изменено"
    },
    "reset": {
      "action": "Ссылка для нового пароля",
      "actionNew": "Ссылка, чтобы задать пароль",
      "title": "Ссылка для нового пароля",
      "text": "Передайте её: {name} задаст новый пароль, прежние сессии закроются. Ссылка одноразовая, действует 48 часов"
    },
    "delete": {
      "action": "Удалить аккаунт",
      "title": "Удалить аккаунт",
      "text": "Аккаунт «{name}» исчезнет вместе со всеми данными: медиатекой, плейлистами, историей, устройствами и сессиями. Вернуть не получится.",
      "type": "Чтобы подтвердить, введите имя: {name}",
      "submit": "Удалить навсегда",
      "done": "Аккаунт «{name}» удалён"
    },
    "err": {
      "failed": "Не получилось — попробуйте ещё раз",
      "self": "С собой так нельзя",
      "lastAdmin": "Это единственный администратор: сначала выдайте роль кому-нибудь ещё",
      "lastAdminShort": "единственный администратор"
    },
    "stats": {
      "title": "Статистика",
      "h7": "слушал за 7 дней",
      "h30": "слушал за 30 дней",
      "p30": "прослушиваний за 30 дней",
      "total": "прослушиваний всего",
      "minutes": "Минуты в день",
      "chart": "Сколько слушал по дням за 30 дней"
    },
    "top": {
      "artists": "Любимые артисты",
      "tracks": "Любимые треки",
      "genres": "Жанры",
      "empty": "За 30 дней пока ничего"
    },
    "profile": {
      "title": "Профиль и вход",
      "created": "Зарегистрирован",
      "lastLogin": "Последний вход",
      "username": "Логин",
      "noPassword": "пароль не задан",
      "method": "Вход",
      "name": "Имя",
      "nameLocked": "задано вручную"
    },
    "library": {
      "title": "Медиатека",
      "playlists": "Плейлисты",
      "liked": "Любимые треки",
      "albums": "Сохранённые альбомы",
      "followed": "Подписки на артистов",
      "friends": "Друзья",
      "imports": "Импорты"
    },
    "sessions": {
      "title": "Устройства и сессии",
      "hint": "Вкладки сайта и приложения, где выполнен вход",
      "all": "Выйти везде",
      "allDone": "Сессии завершены: {n}",
      "done": "Сессия завершена",
      "revoke": "Завершить сессию",
      "browser": "Браузер",
      "empty": "Открытых сессий нет"
    },
    "invite": {
      "title": "Приглашения",
      "came": "Пришёл по приглашению",
      "none": "без приглашения",
      "unknown": "от удалённого аккаунта",
      "sent": "Приглашение",
      "status": { "active": "ждёт", "used": "принято", "expired": "истекло", "revoked": "отозвано" }
    },
    "acq": {
      "title": "Скачивания",
      "hint": "Записи, которые сервер скачал из-за этого человека: лайк, плейлист, воспроизведение",
      "total": "Всего",
      "ready": "Готово",
      "active": "В работе",
      "failed": "Не вышло"
    },
    "history": {
      "title": "История прослушиваний",
      "hint": "В приватной сессии видно только время",
      "private": "Приватная сессия",
      "skipped": "пропущен",
      "more": "Показать ещё",
      "empty": "Ещё ничего не слушал",
      "ctx": { "album": "альбом", "playlist": "плейлист", "artist": "артист", "wave": "волна", "radio": "радио", "mix": "микс", "search": "поиск", "library": "медиатека", "queue": "очередь", "remote": "пульт" }
    }
  },
  "en": {
    "title": "User",
    "back": "Users",
    "missing": "No such user",
    "missingText": "The account may have been deleted",
    "you": "you",
    "more": "More",
    "give": "Give access",
    "live": "Listening now",
    "roles": {
      "title": "Roles",
      "idp": "granted by the sign-in provider",
      "given": { "admin": "{name} is now an admin", "listener": "{name} can now listen" },
      "removed": { "admin": "{name} is no longer an admin", "listener": "{name} is no longer a listener" }
    },
    "block": {
      "action": "Block",
      "undo": "Unblock",
      "done": "{name} blocked: sign-in closed, sessions ended",
      "undone": "{name} can sign in again",
      "since": "Blocked {when}: can't sign in",
      "now": "Blocked: can't sign in"
    },
    "rename": {
      "action": "Change name",
      "title": "Name",
      "note": "The sign-in provider won't change this name anymore",
      "submit": "Save",
      "done": "Name changed"
    },
    "reset": {
      "action": "New password link",
      "actionNew": "Link to set a password",
      "title": "New password link",
      "text": "Pass it on: {name} sets a new password, old sessions end. One-time link, valid for 48 hours"
    },
    "delete": {
      "action": "Delete account",
      "title": "Delete account",
      "text": "The account of {name} will disappear with all its data: library, playlists, history, devices and sessions. This can't be undone.",
      "type": "To confirm, type the name: {name}",
      "submit": "Delete forever",
      "done": "Account of {name} deleted"
    },
    "err": {
      "failed": "That didn't work — try again",
      "self": "You can't do that to yourself",
      "lastAdmin": "This is the only admin: give the role to someone else first",
      "lastAdminShort": "the only admin"
    },
    "stats": {
      "title": "Statistics",
      "h7": "listened in 7 days",
      "h30": "listened in 30 days",
      "p30": "plays in 30 days",
      "total": "plays in total",
      "minutes": "Minutes a day",
      "chart": "Listening per day, last 30 days"
    },
    "top": {
      "artists": "Top artists",
      "tracks": "Top tracks",
      "genres": "Genres",
      "empty": "Nothing in the last 30 days"
    },
    "profile": {
      "title": "Profile and sign-in",
      "created": "Joined",
      "lastLogin": "Last sign-in",
      "username": "Username",
      "noPassword": "no password yet",
      "method": "Sign-in",
      "name": "Name",
      "nameLocked": "set manually"
    },
    "library": {
      "title": "Library",
      "playlists": "Playlists",
      "liked": "Liked tracks",
      "albums": "Saved albums",
      "followed": "Followed artists",
      "friends": "Friends",
      "imports": "Imports"
    },
    "sessions": {
      "title": "Devices and sessions",
      "hint": "Website tabs and apps that are signed in",
      "all": "Sign out everywhere",
      "allDone": "Sessions ended: {n}",
      "done": "Session ended",
      "revoke": "End session",
      "browser": "Browser",
      "empty": "No open sessions"
    },
    "invite": {
      "title": "Invites",
      "came": "Came with an invite",
      "none": "no invite",
      "unknown": "from a deleted account",
      "sent": "Invite",
      "status": { "active": "waiting", "used": "accepted", "expired": "expired", "revoked": "revoked" }
    },
    "acq": {
      "title": "Downloads",
      "hint": "Recordings the server downloaded because of this person: a like, a playlist, a play",
      "total": "Total",
      "ready": "Ready",
      "active": "In progress",
      "failed": "Failed"
    },
    "history": {
      "title": "Listening history",
      "hint": "Only the time is visible in a private session",
      "private": "Private session",
      "skipped": "skipped",
      "more": "Show more",
      "empty": "Hasn't listened to anything yet",
      "ctx": { "album": "album", "playlist": "playlist", "artist": "artist", "wave": "wave", "radio": "radio", "mix": "mix", "search": "search", "library": "library", "queue": "queue", "remote": "remote" }
    }
  }
}
</i18n>
