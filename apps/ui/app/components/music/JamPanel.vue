<script setup lang="ts">
import { copyText } from '~/utils/web-api'
// «Слушать вместе» у хозяина: QR-код и ссылка для гостей, кто слушает, треки на одобрении и
// настройки. Правая панель на компьютере, лист на телефоне (`sheet`)
withDefaults(defineProps<{ variant?: 'panel' | 'sheet', back?: boolean }>(), { variant: 'panel' })
const emit = defineEmits<{ back: [] }>()

const jam = useJamStore()
const bridge = useBridge()
const toast = useToast()
const ui = useUiStore()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })

onMounted(() => {
  jam.init()
  void jam.load()
})

const url = computed(() => jam.jam?.url ?? '')
/** Ссылка без схемы — короче и читается */
const shortUrl = computed(() => url.value.replace(/^https?:\/\//, ''))
const online = computed(() => jam.guests.filter(g => g.online).length)

async function copy() {
  try {
    if (bridge.kind === 'tauri' && bridge.isDesktopApp) {
      const { invoke } = await import('@tauri-apps/api/core')
      await invoke('plugin:clipboard-manager|write_text', { text: url.value })
    }
    else await copyText(url.value)
    bridge.haptic('light')
    toast.show({ text: t('copied'), icon: 'ph:link' })
  }
  catch {
    toast.error(t('copyFailed'))
  }
}

const canShare = computed(() => import.meta.client && typeof navigator.share === 'function' && (ui.isPhone || bridge.platform === 'ios' || bridge.platform === 'ipados'))
async function share() {
  try {
    await navigator.share({ title: t('shareTitle'), url: url.value })
  }
  catch (e) {
    if (!(e instanceof DOMException && e.name === 'AbortError')) void copy()
  }
}

const guestsLabel = computed(() => {
  const n = jam.guests.length
  if (lang.value === 'en') return n === 1 ? '1 guest' : `${n} guests`
  return `${n}${NBSP}${plural(n, ['гость', 'гостя', 'гостей'])}`
})

const initial = (name: string) => name.trim().charAt(0).toUpperCase() || '?'
</script>

<template>
  <div class="jam" :class="`jam--${variant}`">
    <!-- Правая панель: «Слушать вместе» открыто из «Устройств» — назад туда -->
    <button v-if="back" type="button" class="jam__back" @click="emit('back')">
      <Icon name="ph:caret-left" />{{ t('title') }}
    </button>
    <!-- Сессии нет: одно действие -->
    <div v-if="!jam.jam" class="jam__intro">
      <span class="jam__badge"><Icon name="ph:users-three" /></span>
      <h3 class="jam__title">{{ t('title') }}</h3>
      <p class="jam__text">{{ t('intro') }}</p>
      <UiButton variant="primary" icon="ph:play-fill" :loading="jam.busy || jam.loading" @click="jam.start()">{{ t('start') }}</UiButton>
    </div>

    <template v-else>
      <section class="jam__invite">
        <MusicJamQr :value="url" :label="t('qr')" class="jam__qr" />
        <p class="jam__hint">{{ t('scan') }}</p>
        <div class="jam__link">
          <span class="jam__url ellipsis" :title="url">{{ shortUrl }}</span>
          <UiButton v-if="canShare" size="sm" icon="ph:export" @click="share">{{ t('share') }}</UiButton>
          <UiButton v-else size="sm" icon="ph:copy" @click="copy">{{ t('copy') }}</UiButton>
        </div>
      </section>

      <section v-if="jam.pending.length" class="jam__sec">
        <h4 class="jam__head">{{ t('pending') }}</h4>
        <div v-for="r in jam.pending" :key="r.id" class="jrow">
          <MusicCoverArt :image="r.track.image" size="small" class="jrow__art" radius="8px" />
          <span class="jrow__text">
            <span class="jrow__name ellipsis">{{ r.track.title }}</span>
            <span class="jrow__meta ellipsis">{{ r.guestName }} · {{ r.track.artists.join(', ') }}</span>
          </span>
          <UiIconButton icon="ph:x" :label="t('decline')" size="sm" :disabled="jam.busy" @click="jam.decline(r.id)" />
          <UiIconButton icon="ph:check-bold" :label="t('approve')" size="sm" tone="solid" :disabled="jam.busy" @click="jam.approve(r.id)" />
        </div>
      </section>

      <section class="jam__sec">
        <h4 class="jam__head">
          {{ t('listening') }}
          <span v-if="jam.guests.length" class="jam__count">{{ guestsLabel }}<template v-if="online && online < jam.guests.length"> · {{ t('online', { n: online }) }}</template></span>
        </h4>
        <p v-if="!jam.guests.length" class="jam__empty">{{ t('noGuests') }}</p>
        <div v-for="g in jam.guests" :key="g.id" class="jrow" :class="{ 'is-away': !g.online }">
          <span class="jrow__avatar" aria-hidden="true">{{ initial(g.name) }}</span>
          <span class="jrow__text">
            <span class="jrow__name ellipsis">{{ g.name }}</span>
            <span class="jrow__meta">{{ g.online ? t('here') : t('away') }}</span>
          </span>
          <UiIconButton icon="ph:user-minus" :label="t('remove', { name: g.name })" size="sm" :disabled="jam.busy" @click="jam.removeGuest(g.id)" />
        </div>
      </section>

      <SettingsGroup class="jam__settings">
        <SettingsRow :title="t('canAdd')">
          <UiSwitch :model-value="jam.jam.settings.guestsCanAdd" :label="t('canAdd')" hide-label @update:model-value="v => jam.update({ guestsCanAdd: v })" />
        </SettingsRow>
        <SettingsRow :title="t('approval')" :disabled="!jam.jam.settings.guestsCanAdd">
          <UiSwitch :model-value="jam.jam.settings.approval" :label="t('approval')" hide-label :disabled="!jam.jam.settings.guestsCanAdd" @update:model-value="v => jam.update({ approval: v })" />
        </SettingsRow>
        <SettingsRow :title="t('canListen')">
          <UiSwitch :model-value="jam.jam.settings.guestsCanListen" :label="t('canListen')" hide-label @update:model-value="v => jam.update({ guestsCanListen: v })" />
        </SettingsRow>
      </SettingsGroup>

      <UiButton variant="danger" block icon="ph:stop-circle" class="jam__end" :loading="jam.busy" @click="jam.end()">{{ t('end') }}</UiButton>
    </template>
  </div>
</template>

<style scoped>
.jam { display: flex; flex-direction: column; gap: 20px; padding: 16px 8px 8px; }
.jam--sheet { padding: 4px 20px 16px; }
.jam__back { display: inline-flex; align-items: center; gap: 6px; align-self: flex-start; margin: -8px 0 -8px; padding: 6px 8px 6px 2px; border: 0; border-radius: var(--r-2); background: none; font: inherit; font-weight: 650; color: var(--text-1); }
.jam__back:hover { color: var(--text-2); }
.jam__back:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }

.jam__intro { display: flex; flex-direction: column; align-items: center; gap: 10px; padding: 24px 8px; text-align: center; }
.jam__badge { display: grid; place-items: center; width: 56px; height: 56px; border-radius: 50%; background: var(--fill-2); font-size: 28px; color: var(--text-1); }
.jam__title { margin: 6px 0 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.jam__text { max-width: 280px; margin: 0 0 8px; color: var(--text-2); text-wrap: pretty; }

.jam__invite { display: flex; flex-direction: column; align-items: center; gap: 12px; }
.jam__qr { width: min(208px, 64%); }
.jam--sheet .jam__qr { width: min(232px, 64vw); }
.jam__hint { margin: 0; font-size: var(--fs-small); color: var(--text-2); text-align: center; text-wrap: pretty; }
.jam__link { display: flex; align-items: center; gap: 10px; width: 100%; min-width: 0; padding: 6px 6px 6px 14px; border-radius: var(--r-pill); background: var(--fill-1); }
.jam__url { flex: 1; min-width: 0; font-size: var(--fs-small); color: var(--text-2); font-variant-numeric: tabular-nums; }

.jam__sec { display: flex; flex-direction: column; }
.jam__head { display: flex; align-items: baseline; gap: 8px; margin: 0; padding: 0 8px 6px; font-size: var(--fs-small); font-weight: 650; }
.jam--sheet .jam__head { padding: 0 0 6px; }
.jam__count { font-size: var(--fs-caption); font-weight: 500; color: var(--text-3); }
.jam__empty { margin: 0; padding: 6px 8px; font-size: var(--fs-small); color: var(--text-3); }
.jam--sheet .jam__empty { padding: 6px 0; }

.jrow { position: relative; display: flex; align-items: center; gap: 12px; min-height: 52px; padding: 6px 4px 6px 8px; }
.jam--sheet .jrow { padding: 6px 0; min-height: 56px; }
.jrow + .jrow::before { content: ''; position: absolute; top: 0; left: 60px; right: 0; height: 0.5px; background: var(--hair); }
.jrow__art { width: 40px; height: 40px; flex: none; }
.jrow__avatar { display: grid; place-items: center; width: 40px; height: 40px; flex: none; border-radius: 50%; background: var(--fill-3); font-weight: 700; color: var(--text-1); }
.jrow__text { display: flex; flex: 1; flex-direction: column; gap: 1px; min-width: 0; }
.jrow__name { font-weight: 600; }
.jrow__meta { font-size: var(--fs-caption); color: var(--text-3); }
.jrow.is-away .jrow__avatar, .jrow.is-away .jrow__name { opacity: 0.55; }

.jam__settings { margin: 0; }
.jam--panel .jam__settings { padding: 0 8px; }
.jam__end { margin-top: 4px; }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Слушать вместе",
    "intro": "Друзья откроют ссылку на телефоне и добавят свои треки в очередь — без аккаунта",
    "start": "Начать",
    "qr": "QR-код ссылки для гостей",
    "scan": "Наведите камеру телефона или отправьте ссылку",
    "copy": "Скопировать",
    "copied": "Ссылка скопирована",
    "copyFailed": "Не получилось скопировать ссылку",
    "share": "Отправить",
    "shareTitle": "Слушаем вместе в DalyWave",
    "pending": "Ждут одобрения",
    "approve": "Добавить в очередь",
    "decline": "Отклонить",
    "listening": "Слушают",
    "online": "на связи {n}",
    "noGuests": "Пока никого",
    "here": "на связи",
    "away": "не на связи",
    "remove": "Убрать {name}",
    "canAdd": "Гости добавляют треки",
    "approval": "Одобрять треки гостей",
    "canListen": "Гости слушают у себя",
    "end": "Закончить"
  },
  "en": {
    "title": "Listen together",
    "intro": "Friends open the link on their phones and add tracks to the queue — no account needed",
    "start": "Start",
    "qr": "QR code with the guest link",
    "scan": "Point a phone camera here or send the link",
    "copy": "Copy",
    "copied": "Link copied",
    "copyFailed": "Couldn’t copy the link",
    "share": "Send",
    "shareTitle": "Listening together on DalyWave",
    "pending": "Waiting for approval",
    "approve": "Add to queue",
    "decline": "Decline",
    "listening": "Listening",
    "online": "{n} online",
    "noGuests": "Nobody yet",
    "here": "online",
    "away": "away",
    "remove": "Remove {name}",
    "canAdd": "Guests add tracks",
    "approval": "Approve guest tracks",
    "canListen": "Guests listen on their devices",
    "end": "End session"
  }
}
</i18n>
