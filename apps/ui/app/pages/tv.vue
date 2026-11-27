<script setup lang="ts">
import type { TvLogin } from '~/api/types'

// Подключить телевизор: код с экрана Apple TV или страницы вечеринки в браузере телевизора.
// QR ведёт сюда с кодом в адресе — остаётся подтвердить. Телевизор получает свою сессию: отдельная
// строка в «Доступе», которую можно отозвать
const route = useRoute()
const bridge = useBridge()
const ui = useUiStore()
const { t } = useScopedI18n('pages.tv')
useHead({ title: () => t('title') })

type Phase = 'enter' | 'confirm' | 'done'
const phase = ref<Phase>('enter')
const code = ref(typeof route.query.code === 'string' ? route.query.code : '')
const login = ref<TvLogin | null>(null)
const error = ref('')
const busy = ref(false)

/** Как на экране: KQ7M-4TXP */
function pretty(raw: string) {
  const c = raw.toUpperCase().replace(/[^A-Z0-9]/g, '').slice(0, 8)
  return c.length > 4 ? `${c.slice(0, 4)}-${c.slice(4)}` : c
}
function onInput(e: Event) {
  code.value = pretty((e.target as HTMLInputElement).value)
  error.value = ''
}

async function check() {
  if (busy.value) return
  const c = code.value.replace(/[^A-Za-z0-9]/g, '')
  if (c.length !== 8) {
    error.value = t('short')
    return
  }
  busy.value = true
  try {
    login.value = await bridge.party.lookup(c)
    phase.value = 'confirm'
  }
  catch {
    error.value = t('unknown')
  }
  finally {
    busy.value = false
  }
}

async function approve() {
  if (busy.value || !login.value) return
  busy.value = true
  try {
    await bridge.party.approve(login.value.userCode)
    phase.value = 'done'
  }
  catch {
    error.value = t('unknown')
    phase.value = 'enter'
  }
  finally {
    busy.value = false
  }
}

function cancel() {
  phase.value = 'enter'
  login.value = null
  code.value = ''
}

onMounted(() => {
  code.value = pretty(code.value)
  if (code.value.length === 9) void check()
})
</script>

<template>
  <div class="tv">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="t('title')" />
    <div class="tv__card">
      <Icon name="ph:television-simple" class="tv__icon" />
      <template v-if="phase === 'enter'">
        <h1 class="tv__title">{{ t('title') }}</h1>
        <p class="tv__hint">{{ t('hint') }}</p>
        <form class="tv__form" @submit.prevent="check">
          <input
            class="tv__input"
            :value="code"
            inputmode="text"
            autocomplete="one-time-code"
            autocapitalize="characters"
            spellcheck="false"
            maxlength="9"
            placeholder="XXXX-XXXX"
            :aria-label="t('code')"
            :aria-invalid="!!error || undefined"
            @input="onInput"
          >
          <p v-if="error" class="tv__error" role="alert">{{ error }}</p>
          <UiButton class="tv__go" variant="primary" size="lg" type="submit" :disabled="busy || code.length < 9">{{ t('next') }}</UiButton>
        </form>
      </template>
      <template v-else-if="phase === 'confirm' && login">
        <h1 class="tv__title">{{ t('confirmTitle', { name: login.deviceName }) }}</h1>
        <p class="tv__hint">{{ t('confirmHint') }}</p>
        <div class="tv__code">{{ login.userCode }}</div>
        <div class="tv__actions">
          <UiButton variant="primary" size="lg" :disabled="busy" @click="approve">{{ t('approve') }}</UiButton>
          <UiButton variant="ghost" size="lg" :disabled="busy" @click="cancel">{{ t('cancel') }}</UiButton>
        </div>
      </template>
      <template v-else>
        <h1 class="tv__title">{{ t('doneTitle') }}</h1>
        <p class="tv__hint">{{ t('doneHint') }}</p>
        <UiButton variant="secondary" size="lg" to="/">{{ t('home') }}</UiButton>
      </template>
    </div>
  </div>
</template>

<style scoped>
.tv { display: flex; flex-direction: column; align-items: center; width: 100%; padding: 48px 16px; }
.tv__card { display: flex; flex-direction: column; align-items: center; gap: 14px; width: 100%; max-width: 420px; text-align: center; }
.tv__icon { font-size: 44px; color: var(--text-2); }
.tv__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); text-wrap: balance; }
.tv__hint { margin: 0; color: var(--text-2); text-wrap: pretty; }
.tv__form { display: flex; flex-direction: column; align-items: stretch; gap: 12px; width: 100%; margin-top: 8px; }
.tv__input {
  width: 100%;
  height: 64px;
  padding: 0 16px;
  border: 1px solid var(--line-2, rgba(var(--hi-rgb), 0.14));
  border-radius: 16px;
  background: var(--fill-1);
  color: var(--text-1);
  font: inherit;
  font-size: 30px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-align: center;
  text-transform: uppercase;
  font-variant-numeric: tabular-nums;
}
.tv__input::placeholder { color: var(--text-4); }
.tv__input:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.tv__error { margin: 0; color: var(--danger, #ff5b5b); font-size: var(--fs-small); }
.tv__go { width: 100%; }
.tv__code { margin: 8px 0; font-size: 34px; font-weight: 750; letter-spacing: 0.12em; font-variant-numeric: tabular-nums; }
.tv__actions { display: flex; flex-direction: column; gap: 8px; width: 100%; }
.tv__actions :deep(.btn) { width: 100%; }

@media (max-width: 699px) {
  .tv { padding-top: 8px; }
}
</style>
