<script setup lang="ts">
import type { AuthOverview } from '~/api/types'
import { copyText } from '~/utils/web-api'

// «Способы входа» в «Доступе» — только чтение: провайдеры и свои аккаунты настраиваются переменными
// окружения сервера (docs/self-hosting/auth.md). Адрес возврата — тот, что регистрируют у провайдера
defineProps<{ overview: AuthOverview }>()
const toast = useToast()
const { t } = useScopedI18n('components.admin.SignInMethods')

async function copy(text: string) {
  try {
    await copyText(text)
    toast.show({ text: t('copied'), icon: 'ph:link' })
  }
  catch {
    toast.error(t('copyFailed'))
  }
}
</script>

<template>
  <div class="ap-table">
    <div v-for="p in overview.providers" :key="p.id" class="sm__row">
      <span class="sm__icon">
        <img v-if="p.icon" :src="p.icon" alt="">
        <Icon v-else name="ph:key" />
      </span>
      <span class="ap-cell sm__main">
        <b>{{ p.title }} <span class="sm__id">{{ p.id }}</span></b>
        <small>{{ p.issuer }}</small>
        <small class="sm__facts">{{ [
          p.adminClaim ? t('admins', { claim: p.adminClaim, values: p.adminValues.join(', ') }) : t('noAdmins'),
          p.autoRegister ? t('open') : t('inviteOnly'),
          p.trustEmail ? t('trustEmail') : null,
          t('users', { n: p.users }),
        ].filter(Boolean).join(' · ') }}</small>
        <button type="button" class="sm__callback" :title="t('copyCallback')" @click="copy(p.callbackUrl)">
          <Icon name="ph:copy" aria-hidden="true" />{{ p.callbackUrl }}
        </button>
      </span>
    </div>
    <div class="sm__row">
      <span class="sm__icon"><Icon name="ph:password" /></span>
      <span class="ap-cell sm__main">
        <b>{{ t('local') }}</b>
        <small>{{ overview.local ? t('localOn') : t('localOff') }}</small>
      </span>
      <AdminStatus :tone="overview.local ? 'ok' : 'idle'">{{ overview.local ? t('on') : t('off') }}</AdminStatus>
    </div>
    <div class="sm__row">
      <span class="sm__icon"><Icon name="ph:lifebuoy" /></span>
      <span class="ap-cell sm__main">
        <b>{{ t('emergency') }}</b>
        <small>DALYWAVE_ADMIN_PASSWORD</small>
      </span>
      <AdminStatus :tone="overview.emergencyPassword ? 'ok' : 'idle'">{{ overview.emergencyPassword ? t('on') : t('off') }}</AdminStatus>
    </div>
  </div>
</template>

<style scoped>
.sm__row { display: flex; align-items: flex-start; gap: 14px; padding: 12px 0; border-top: 1px solid var(--hair); }
.sm__row:first-child { border-top: 0; }
.sm__icon { display: grid; place-items: center; flex: none; width: 36px; height: 36px; overflow: hidden; border-radius: var(--r-2); background: var(--fill-1); color: var(--text-2); font-size: 18px; }
.sm__icon img { width: 22px; height: 22px; object-fit: contain; }
.sm__main { flex: 1; gap: 2px; }
.sm__id { margin-left: 6px; font-weight: 500; font-size: var(--fs-caption); color: var(--text-3); }
.sm__facts { color: var(--text-3); }
.sm__callback {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  max-width: 100%;
  min-height: 32px;
  margin-top: 2px;
  padding: 0;
  overflow: hidden;
  border: 0;
  background: none;
  color: var(--text-2);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}
.sm__callback:hover { color: var(--text-1); }
.sm__callback:focus-visible { outline: none; border-radius: var(--r-1, 4px); box-shadow: 0 0 0 2px var(--focus); }
</style>
