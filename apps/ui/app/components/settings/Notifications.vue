<script setup lang="ts">
import type { NotificationKind, NotificationSettings, NotificationSettingsPatch, QuietHours } from '~/api/types'

// Настройки → «Уведомления»: push на этом устройстве (системный запрос разрешения — только по
// переключателю), какие виды присылать (админские сервер отдаёт только администраторам), тихие часы.
// Часовой пояс уходит с каждой правкой: тихие часы считаются по времени человека
const store = useNotificationsStore()
const bridge = useBridge()
const toast = useToast()
const { tg, te } = useGlobalT()

const settings = ref<NotificationSettings | null>(null)

onMounted(async () => {
  void store.loadPush()
  try {
    settings.value = await bridge.notifications.settings()
  }
  catch {
    // Без связи группа видов не показывается
  }
})

const offset = () => -new Date().getTimezoneOffset()

async function patch(p: NotificationSettingsPatch) {
  const was = settings.value
  if (!was) return
  settings.value = {
    ...was,
    kinds: p.kinds ? was.kinds.map(k => p.kinds!.find(x => x.kind === k.kind) ?? k) : was.kinds,
    quiet: p.quiet ?? was.quiet,
  }
  try {
    settings.value = await bridge.notifications.updateSettings({ ...p, utcOffsetMin: offset() })
  }
  catch {
    settings.value = was
    toast.error(tg('settingsSync.failed'))
  }
}

function setKind(kind: NotificationKind, enabled: boolean) {
  void patch({ kinds: [{ kind, enabled }] })
}

function setQuiet(q: Partial<QuietHours>) {
  if (!settings.value) return
  void patch({ quiet: { ...settings.value.quiet, ...q } })
}

const quietOn = computed({
  get: () => settings.value?.quiet.enabled ?? false,
  set: (v: boolean) => setQuiet({ enabled: v }),
})

function onTime(which: 'from' | 'to', e: Event) {
  const m = timeToMinutes((e.target as HTMLInputElement).value)
  if (m != null) setQuiet({ [which]: m })
}

// Устройство: переключатель — подписка; запрещено системой — подсказка, где включить
const device = computed({
  get: () => store.enabled,
  set: (v: boolean) => void (v ? store.enable() : store.disable()),
})
const denied = computed(() => store.permission === 'denied')
const showDevice = computed(() => store.available || (denied.value && !!store.push))
const deniedText = computed(() => (bridge.appPush ? tg('notify.deniedApp') : tg('notify.deniedWeb')))
const kindDesc = (k: string) => (te(`notify.kindDesc.${k}`) ? tg(`notify.kindDesc.${k}`) : undefined)
</script>

<template>
  <SettingsGroup :title="tg('notify.title')">
    <SettingsRow v-if="showDevice" :title="tg('notify.device')" :description="denied ? deniedText : undefined">
      <UiSwitch v-model="device" :label="tg('notify.device')" hide-label :disabled="denied || store.busy" />
    </SettingsRow>
    <template v-if="settings">
      <SettingsRow v-for="k in settings.kinds" :key="k.kind" :title="tg(`notify.kind.${k.kind}`)" :description="kindDesc(k.kind)">
        <UiSwitch :model-value="k.enabled" :label="tg(`notify.kind.${k.kind}`)" hide-label @update:model-value="v => setKind(k.kind, v)" />
      </SettingsRow>
      <SettingsRow :title="tg('notify.quiet')" :description="tg('notify.quietDesc')">
        <UiSwitch v-model="quietOn" :label="tg('notify.quiet')" hide-label />
      </SettingsRow>
      <SettingsRow v-if="settings.quiet.enabled" indent>
        <div class="quiet">
          <label class="quiet__field">
            <span>{{ tg('notify.from') }}</span>
            <input type="time" step="900" class="quiet__time tnum" :value="minutesToTime(settings.quiet.from)" @change="e => onTime('from', e)">
          </label>
          <label class="quiet__field">
            <span>{{ tg('notify.to') }}</span>
            <input type="time" step="900" class="quiet__time tnum" :value="minutesToTime(settings.quiet.to)" @change="e => onTime('to', e)">
          </label>
        </div>
      </SettingsRow>
      <SettingsRow as="button" :title="tg('notify.test')" @click="store.test()" />
    </template>
  </SettingsGroup>
</template>

<style scoped>
.quiet { display: flex; align-items: center; gap: 16px; flex-wrap: wrap; }
.quiet__field { display: inline-flex; align-items: center; gap: 8px; font-size: var(--fs-small); color: var(--text-3); }
.quiet__time {
  height: 36px;
  padding: 0 12px;
  border: 0;
  border-radius: var(--r-3);
  background: var(--fill-2);
  color: var(--text-1);
  font: inherit;
  font-size: var(--fs-body);
  color-scheme: inherit;
}
.quiet__time:hover { background: var(--fill-3); }
.quiet__time:focus-visible { outline: none; box-shadow: inset 0 0 0 2px var(--focus); }
.quiet__time::-webkit-calendar-picker-indicator { display: none; }

@media (pointer: coarse) {
  .quiet__time { height: 44px; }
}
</style>
