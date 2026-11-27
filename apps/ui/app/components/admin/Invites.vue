<script setup lang="ts">
import type { Invite, Role } from '~/api/types'

// Приглашения в «Доступе»: ссылка с ролями и сроком. Человек по ней придумывает логин и пароль или
// входит через провайдера — и сразу получает роли. Ссылка одноразовая, показывается один раз
const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { failed, attempt } = useAttempts()

const invites = ref<Invite[] | null>(null)
const load = () => attempt('invites', () => bridge.admin.invites(), l => (invites.value = l.items))
onMounted(load)

// ---------- Новое приглашение ----------
const open = ref(false)
const role = ref<'listener' | 'admin' | 'none'>('listener')
const days = ref<'1' | '7' | '30'>('7')
const note = ref('')
const busy = ref(false)
const link = ref<{ url: string } | null>(null)
const linkOpen = ref(false)

const roleOptions = computed(() => [
  { value: 'listener' as const, label: t('role.listener') },
  { value: 'admin' as const, label: t('role.admin') },
  { value: 'none' as const, label: t('role.none') },
])
const dayOptions = computed(() => (['1', '7', '30'] as const).map(d => ({ value: d, label: t('days', { n: Number(d) }) })))

watch(open, (v) => {
  if (!v) return
  role.value = 'listener'
  days.value = '7'
  note.value = ''
})

async function create() {
  busy.value = true
  try {
    const roles: Role[] = role.value === 'admin' ? ['admin', 'listener'] : role.value === 'listener' ? ['listener'] : []
    const res = await bridge.admin.createInvite({ roles, days: Number(days.value), note: note.value.trim() || null })
    open.value = false
    link.value = { url: res.url }
    linkOpen.value = true
    invites.value = [res.invite, ...(invites.value ?? [])]
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}

async function revoke(i: Invite) {
  try {
    await bridge.admin.revokeInvite(i.id)
    i.status = 'revoked'
  }
  catch {
    toast.error(t('failed'))
  }
}

const TONE = { active: 'on', used: 'ok', expired: 'idle', revoked: 'idle' } as const
const shown = computed(() => (invites.value ?? []).slice(0, 12))
const roleText = (i: Invite) => (i.roles.length ? i.roles.map(r => t(`role.${r}`)).join(' · ') : t('role.none'))
</script>

<template>
  <AdminSection :title="t('title')" :hint="t('hint')">
    <template #tools>
      <UiButton variant="secondary" size="sm" icon="ph:link" @click="open = true">{{ t('create') }}</UiButton>
    </template>
    <AdminState :loading="!invites && !failed.invites" :error="failed.invites && !invites" :empty="!!invites && !invites.length" empty-icon="ph:envelope-simple" :empty-title="t('empty')" :rows="2" @retry="load">
      <div class="ap-table">
        <div v-for="i in shown" :key="i.id" class="iv__row">
          <span class="ap-cell iv__main">
            <b>{{ i.note || roleText(i) }}</b>
            <small>{{ [i.note ? roleText(i) : null, i.status === 'used' && i.usedBy ? t('usedBy', { name: i.usedBy }) : t('until', { when: formatShortDate(i.expiresAt, lang) })].filter(Boolean).join(' · ') }}</small>
          </span>
          <AdminStatus :tone="TONE[i.status]">{{ t(`status.${i.status}`) }}</AdminStatus>
          <span class="ap-end iv__end">
            <UiIconButton v-if="i.status === 'active'" icon="ph:x" :label="t('revoke')" size="sm" @click="revoke(i)" />
          </span>
        </div>
      </div>
    </AdminState>

    <UiModal v-model:open="open" :title="t('createTitle')" :width="460">
      <form class="iv__form" @submit.prevent="create">
        <label class="iv__label">{{ t('roleLabel') }}</label>
        <UiSegmentedControl v-model="role" :options="roleOptions" :label="t('roleLabel')" block />
        <label class="iv__label">{{ t('daysLabel') }}</label>
        <UiSegmentedControl v-model="days" :options="dayOptions" :label="t('daysLabel')" block />
        <input v-model="note" class="iv__input" type="text" maxlength="120" :placeholder="t('note')" :aria-label="t('note')">
        <UiButton type="submit" variant="primary" size="lg" block :loading="busy">{{ t('createSubmit') }}</UiButton>
      </form>
    </UiModal>
    <AdminLinkModal v-if="link" v-model:open="linkOpen" :title="t('linkTitle')" :text="t('linkText')" :url="link.url" />
  </AdminSection>
</template>

<style scoped>
.iv__row { display: flex; align-items: center; gap: 14px; min-height: 56px; padding: 8px 0; border-top: 1px solid var(--hair); }
.iv__row:first-child { border-top: 0; }
.iv__main { flex: 1; }
.iv__end { width: 40px; }
.iv__form { display: flex; flex-direction: column; gap: 10px; padding: 0 20px 16px; }
.iv__label { margin-top: 4px; font-size: var(--fs-caption); font-weight: 600; color: var(--text-3); }
.iv__input {
  width: 100%;
  height: 44px;
  margin: 6px 0 4px;
  padding: 0 16px;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--text-1);
  font: inherit;
}
.iv__input::placeholder { color: var(--text-4); }
.iv__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Приглашения",
    "hint": "Одноразовая ссылка: человек входит и сразу получает роль",
    "create": "Пригласить",
    "createTitle": "Приглашение",
    "createSubmit": "Создать ссылку",
    "roleLabel": "Роль",
    "daysLabel": "Действует",
    "days": "{n} дн.",
    "note": "Для кого (видно только вам)",
    "role": { "listener": "Слушатель", "admin": "Администратор", "none": "Без роли" },
    "status": { "active": "действует", "used": "принято", "expired": "истекло", "revoked": "отозвано" },
    "usedBy": "принял {name}",
    "until": "до {when}",
    "revoke": "Отозвать",
    "empty": "Приглашений пока нет",
    "linkTitle": "Ссылка-приглашение",
    "linkText": "Отправьте её человеку. Ссылка показывается один раз и работает только для одного аккаунта",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "uk": {
    "title": "Запрошення",
    "hint": "Одноразове посилання: людина входить і одразу отримує роль",
    "create": "Запросити",
    "createTitle": "Запрошення",
    "createSubmit": "Створити посилання",
    "roleLabel": "Роль",
    "daysLabel": "Діє",
    "days": "{n} дн.",
    "note": "Для кого (бачите лише ви)",
    "role": { "listener": "Слухач", "admin": "Адміністратор", "none": "Без ролі" },
    "status": { "active": "діє", "used": "прийнято", "expired": "прострочено", "revoked": "відкликано" },
    "usedBy": "прийняв {name}",
    "until": "до {when}",
    "revoke": "Відкликати",
    "empty": "Запрошень поки немає",
    "linkTitle": "Посилання-запрошення",
    "linkText": "Надішліть його людині. Посилання показується один раз і працює лише для одного облікового запису",
    "failed": "Не вдалося — спробуйте ще раз"
  },
  "en": {
    "title": "Invites",
    "hint": "One-time link: the person signs in and gets the role right away",
    "create": "Invite",
    "createTitle": "Invite",
    "createSubmit": "Create link",
    "roleLabel": "Role",
    "daysLabel": "Valid for",
    "days": "{n} d",
    "note": "Who it's for (only you see this)",
    "role": { "listener": "Listener", "admin": "Admin", "none": "No role" },
    "status": { "active": "active", "used": "accepted", "expired": "expired", "revoked": "revoked" },
    "usedBy": "accepted by {name}",
    "until": "until {when}",
    "revoke": "Revoke",
    "empty": "No invites yet",
    "linkTitle": "Invite link",
    "linkText": "Send it to the person. The link is shown once and works for one account only",
    "failed": "Something went wrong — try again"
  }
}
</i18n>
