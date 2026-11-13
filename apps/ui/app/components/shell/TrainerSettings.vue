<script setup lang="ts">
import type { TrainerSnapshot } from '~/bridge/types'

// «Помогать учить рекомендации на этом компьютере» — только в приложении для Mac и ПК. По умолчанию
// помогают компьютеры администраторов; остальные — если включат. Учит на питании от сети, когда
// компьютер не занят; что вышло в последний раз — одной строкой, без чисел
const bridge = useBridge()
const auth = useAuthStore()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

const trainer = bridge.trainer
const pc = bridge.platform === 'windows'
const snap = ref<TrainerSnapshot | null>(null)
const busy = ref(false)

let off: (() => void) | null = null
onMounted(async () => {
  if (!trainer) return
  off = trainer.onState(s => (snap.value = s))
  try {
    snap.value = await trainer.state()
  }
  catch {
    snap.value = null
  }
})
onBeforeUnmount(() => off?.())

/** Выбор не сделан — помогают только компьютеры администраторов */
const on = computed(() => snap.value?.enabled ?? auth.isAdmin)

async function toggle(v: boolean) {
  if (!trainer || busy.value) return
  busy.value = true
  try {
    snap.value = await trainer.setEnabled(v)
  }
  catch {
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}

function when(ms: number) {
  const d = new Date(ms)
  const today = new Date()
  const time = d.toLocaleTimeString(lang.value === 'en' ? 'en-US' : 'ru-RU', { hour: '2-digit', minute: '2-digit' })
  if (d.toDateString() === today.toDateString()) return t('today', { time })
  const date = d.toLocaleDateString(lang.value === 'en' ? 'en-US' : 'ru-RU', { day: 'numeric', month: 'long' })
  return t('onDate', { date, time })
}

/** Что сейчас — одной строкой; работает — без «последнего раза» */
const status = computed(() => {
  const s = snap.value
  if (!s || !on.value) return null
  switch (s.phase) {
    case 'fetching':
    case 'training':
    case 'sending':
      return { text: t('state.training'), tone: 'accent' }
    case 'on-battery':
      return { text: t('state.battery'), tone: '' }
    case 'busy':
      return { text: t('state.busy'), tone: '' }
    case 'signed-out':
      return { text: t('state.signedOut'), tone: '' }
    default:
      if (!s.last) return { text: t('state.waiting'), tone: '' }
      return { text: t(`last.${s.last.outcome}`, { when: when(s.last.atMs) }), tone: s.last.outcome === 'failed' ? 'warn' : '' }
  }
})
</script>

<template>
  <SettingsGroup v-if="trainer" id="trainer" :title="pc ? t('titlePc') : t('titleMac')">
    <SettingsRow :title="t('switch')" :description="t('switchDesc')">
      <UiSwitch :model-value="on" :label="t('switch')" hide-label :disabled="busy || !snap" @update:model-value="toggle" />
    </SettingsRow>
    <SettingsRow v-if="status" role="status" aria-live="polite">
      <template #title>
        <span class="tr__state" :class="status.tone && `tr__state--${status.tone}`">{{ status.text }}</span>
      </template>
    </SettingsRow>
  </SettingsGroup>
</template>

<style scoped>
.tr__state { font-size: var(--fs-small); color: var(--text-2); }
.tr__state--accent { color: var(--accent); }
.tr__state--warn { color: var(--warning); }
</style>

<i18n lang="json">
{
  "ru": {
    "titleMac": "Рекомендации на этом Mac",
    "titlePc": "Рекомендации на этом ПК",
    "switch": "Помогать учить рекомендации",
    "switchDesc": "Только на питании от сети, когда компьютер не занят",
    "state": {
      "training": "Учится — это займёт пару минут",
      "battery": "Ждёт питания от сети",
      "busy": "Ждёт, пока компьютер освободится",
      "signedOut": "Нужен вход",
      "waiting": "Ждёт своей очереди"
    },
    "last": {
      "active": "{when}: рекомендации стали точнее",
      "kept": "{when}: прежние рекомендации оказались точнее",
      "failed": "{when}: не получилось, попробует позже"
    },
    "today": "Сегодня в {time}",
    "onDate": "{date} в {time}",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "en": {
    "titleMac": "Recommendations on this Mac",
    "titlePc": "Recommendations on this PC",
    "switch": "Help train recommendations",
    "switchDesc": "Only on mains power, when the computer isn't busy",
    "state": {
      "training": "Training — takes a couple of minutes",
      "battery": "Waiting for mains power",
      "busy": "Waiting until the computer is free",
      "signedOut": "Sign-in required",
      "waiting": "Waiting for its turn"
    },
    "last": {
      "active": "{when}: recommendations got more accurate",
      "kept": "{when}: the previous recommendations were more accurate",
      "failed": "{when}: didn't work, will try later"
    },
    "today": "Today at {time}",
    "onDate": "{date} at {time}",
    "failed": "That didn't work — try again"
  }
}
</i18n>
