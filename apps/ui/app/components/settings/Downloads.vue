<script setup lang="ts">
import { formatBytes, formatCount } from '~/utils/format'

// Настройки › «Загрузки» (только приложение): сколько места занято — закреплённое, умный запас и кэш, —
// предел места, «только по Wi-Fi» (iPhone и iPad), умный запас, «Без сети» и «Удалить всё».
// На сайте раздела нет: музыка на устройство качается только в приложении
const bridge = useBridge()
const offline = useOfflineStore()
const lang = useLang()
const route = useRoute()
const { t } = useI18n({ useScope: 'local' })

const GB = 1024 ** 3
const phone = bridge.platform === 'ios' || bridge.platform === 'ipados'
/** Предел ползунка: телефону хватит 64 ГБ, компьютеру — 200 ГБ */
const MAX_GB = phone ? 64 : 200

const s = computed(() => offline.status)
const usage = computed(() => s.value?.usage ?? { pinnedBytes: 0, reserveBytes: 0, cacheBytes: 0, totalBytes: 0, files: 0 })
const limit = computed(() => s.value?.settings.limitBytes ?? 4 * GB)

// Ползунок двигается свободно, предел сохраняется, когда его отпустили
const limitGb = ref(Math.round(limit.value / GB))
watch(limit, v => (limitGb.value = Math.max(1, Math.round(v / GB))))
let commit: ReturnType<typeof setTimeout> | undefined
watch(limitGb, (v) => {
  clearTimeout(commit)
  if (v * GB === limit.value) return
  commit = setTimeout(() => void offline.setSettings({ limitBytes: v * GB }), 500)
})

const parts = computed(() => {
  const l = Math.max(limit.value, usage.value.totalBytes, 1)
  return [
    { key: 'pinned', bytes: usage.value.pinnedBytes, share: usage.value.pinnedBytes / l },
    { key: 'reserve', bytes: usage.value.reserveBytes, share: usage.value.reserveBytes / l },
    { key: 'cache', bytes: usage.value.cacheBytes, share: usage.value.cacheBytes / l },
  ]
})

/** Что с загрузками сейчас — одной строкой */
const state = computed(() => {
  const d = s.value?.downloads
  if (!s.value || !d) return ''
  if (s.value.settings.forceOffline) return t('state.forced')
  if (!s.value.online) return t('state.offline')
  if (d.waitingForWifi) return t('state.wifi')
  if (d.active || d.queued) return t('state.downloading', { n: formatCount(d.active + d.queued, 'track', lang.value) })
  if (d.overLimit) return t('state.overLimit', { n: formatCount(d.overLimit, 'track', lang.value) })
  if (d.failed) return t('state.failed', { n: formatCount(d.failed, 'track', lang.value) })
  return t('state.done')
})
const stateTone = computed(() => {
  const d = s.value?.downloads
  if (!s.value || !d) return ''
  if (!s.value.online || d.waitingForWifi || d.overLimit || d.failed) return 'warn'
  if (d.active || d.queued) return 'busy'
  return 'ok'
})

const reserveText = computed(() => {
  const r = s.value?.reserve
  if (!r || !r.tracks) return t('reserveDesc')
  return t('reserveCount', { ready: r.ready, total: formatCount(r.tracks, 'track', lang.value) })
})

const reserveSize = computed(() => {
  const r = s.value?.reserve
  return `${formatCount(r?.ready ?? 0, 'track', lang.value)} · ${formatBytes(usage.value.reserveBytes, lang.value)}`
})

// «Удалить всё» — вторым нажатием: первое меняет подпись на «Точно удалить?»
const confirming = ref(false)
let confirmTimer: ReturnType<typeof setTimeout> | undefined
function removeAll() {
  if (!confirming.value) {
    confirming.value = true
    clearTimeout(confirmTimer)
    confirmTimer = setTimeout(() => (confirming.value = false), 4000)
    return
  }
  confirming.value = false
  void offline.clear()
}

onMounted(() => {
  offline.start()
  void offline.refresh()
  if (route.hash === '#downloads') setTimeout(() => document.getElementById('downloads')?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 120)
})
</script>

<template>
  <SettingsGroup v-if="offline.available" id="downloads" class="dlset" :title="t('title')">
    <SettingsRow class="use" stack>
      <template #title>
        <span class="use__total tnum">{{ t('used', { used: formatBytes(usage.totalBytes, lang), limit: formatBytes(limit, lang) }) }}</span>
      </template>
      <div class="use__body">
        <div class="use__bar" role="img" :aria-label="t('used', { used: formatBytes(usage.totalBytes, lang), limit: formatBytes(limit, lang) })">
          <span v-for="p in parts" :key="p.key" :class="`use__seg use__seg--${p.key}`" :style="{ width: `${(p.share * 100).toFixed(2)}%` }" />
        </div>
        <ul class="use__legend">
          <li v-for="p in parts" :key="p.key" class="use__item">
            <i :class="`use__dot use__dot--${p.key}`" />
            <span>{{ t(`part.${p.key}`) }}</span>
            <span class="use__bytes tnum">{{ formatBytes(p.bytes, lang) }}</span>
          </li>
        </ul>
      </div>
    </SettingsRow>

    <SettingsRow class="live" :class="`live--${stateTone}`" role="status" aria-live="polite">
      <template #title>
        <span class="live__state">{{ state }}</span>
      </template>
    </SettingsRow>

    <SettingsRow :title="t('limit')" stack>
      <div class="lim">
        <UiSlider v-model="limitGb" :label="t('limit')" :min="1" :max="MAX_GB" :step="1" :value-text="formatBytes(limitGb * GB, lang)" class="lim__slider" />
        <span class="lim__value tnum">{{ formatBytes(limitGb * GB, lang) }}</span>
      </div>
    </SettingsRow>

    <SettingsRow v-if="phone" :title="t('wifi')" :description="t('wifiDesc')">
      <UiSwitch :model-value="s?.settings.wifiOnly ?? true" :label="t('wifi')" hide-label @update:model-value="v => offline.setSettings({ wifiOnly: v })" />
    </SettingsRow>

    <SettingsRow :title="t('reserve')" :description="reserveText">
      <UiSwitch :model-value="s?.settings.reserveEnabled ?? true" :label="t('reserve')" hide-label @update:model-value="v => offline.setSettings({ reserveEnabled: v })" />
    </SettingsRow>

    <!-- Состав запаса — отдельной страницей, в «Скачанном» его нет -->
    <SettingsRow v-if="s?.settings.reserveEnabled" as="button" :title="t('reserveList')" @click="navigateTo('/downloads/reserve')">
      <span class="rsv tnum">{{ reserveSize }}</span>
      <Icon name="ph:caret-right" class="rsv__chev" aria-hidden="true" />
    </SettingsRow>

    <SettingsRow :title="t('forceOffline')" :description="t('forceOfflineDesc')">
      <UiSwitch :model-value="s?.settings.forceOffline ?? false" :label="t('forceOffline')" hide-label @update:model-value="v => offline.setSettings({ forceOffline: v })" />
    </SettingsRow>

    <SettingsRow as="button" danger :title="confirming ? t('removeConfirm') : t('remove')" :disabled="!usage.totalBytes && !s?.pins.length" @click="removeAll" />
  </SettingsGroup>
</template>

<style scoped>
.dlset { scroll-margin-top: calc(var(--topbar-h) + 16px); }

.use :deep(.sr__title) { min-width: 0; }
.use__total { font-weight: 600; }
.use__body { display: flex; flex: 1; flex-direction: column; gap: 10px; min-width: 0; padding-bottom: 4px; }
.use__bar { display: flex; height: 8px; overflow: hidden; border-radius: var(--r-pill); background: var(--fill-3); }
.use__seg { height: 100%; transition: width var(--dur-3) var(--ease-out); }
.use__seg + .use__seg { box-shadow: -1px 0 0 var(--ink-1); }
.use__seg--pinned, .use__dot--pinned { background: var(--accent); }
.use__seg--reserve, .use__dot--reserve { background: color-mix(in oklab, var(--accent) 45%, var(--ink-3)); }
.use__seg--cache, .use__dot--cache { background: var(--text-3); }
.use__legend { display: flex; flex-wrap: wrap; gap: 6px 20px; margin: 0; padding: 0; list-style: none; font-size: var(--fs-small); color: var(--text-2); }
.use__item { display: inline-flex; align-items: center; gap: 6px; }
.use__dot { width: 8px; height: 8px; border-radius: 50%; }
.use__bytes { color: var(--text-1); font-weight: 550; }

.live__state { font-size: var(--fs-small); color: var(--text-2); }
.live--ok .live__state { color: var(--text-2); }
.live--busy .live__state { color: var(--accent); }
.live--warn .live__state { color: var(--warning); }

.lim { display: flex; flex: 1; align-items: center; gap: 16px; min-width: 0; }
.lim__slider { flex: 1; min-width: 120px; }
.lim__value { width: 72px; flex: none; text-align: right; font-size: var(--fs-small); font-weight: 600; color: var(--text-1); }
.tnum { font-variant-numeric: tabular-nums; }
.rsv { font-size: var(--fs-small); color: var(--text-2); white-space: nowrap; }
.rsv__chev { flex: none; font-size: 16px; color: var(--text-3); }
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Загрузки",
    "used": "Занято {used} из {limit}",
    "part": { "pinned": "Закреплённое", "reserve": "Умный запас", "cache": "Кэш" },
    "state": {
      "forced": "Режим «Без сети»: играет только скачанное",
      "offline": "Нет сети — загрузки продолжатся, когда она появится",
      "wifi": "Ждёт Wi-Fi",
      "downloading": "Скачивается {n}",
      "overLimit": "Не помещается {n} — увеличьте предел",
      "failed": "Не скачалось {n} — попробуем ещё раз позже",
      "done": "Всё скачано"
    },
    "limit": "Сколько места занимать",
    "wifi": "Только по Wi-Fi",
    "wifiDesc": "По сотовой сети музыка не качается",
    "reserve": "Умный запас",
    "reserveDesc": "Любимое вперемешку с новым — волна играет и без сети",
    "reserveCount": "Волна без сети: на устройстве {ready} из {total}",
    "reserveList": "Запас для волны",
    "forceOffline": "Без сети",
    "forceOfflineDesc": "Только скачанное: приложение не ходит на сервер",
    "remove": "Удалить всё скачанное",
    "removeConfirm": "Точно удалить? Нажмите ещё раз"
  },
  "uk": {
    "title": "Завантаження",
    "used": "Зайнято {used} з {limit}",
    "part": { "pinned": "Закріплене", "reserve": "Розумний запас", "cache": "Кеш" },
    "state": {
      "forced": "Режим «Без мережі»: грає тільки завантажене",
      "offline": "Немає мережі — завантаження продовжаться, коли вона з’явиться",
      "wifi": "Чекає Wi-Fi",
      "downloading": "Завантажується {n}",
      "overLimit": "Не вміщається {n} — збільште ліміт",
      "failed": "Не завантажилося {n} — спробуємо ще раз пізніше",
      "done": "Усе завантажено"
    },
    "limit": "Скільки місця займати",
    "wifi": "Тільки через Wi-Fi",
    "wifiDesc": "Через мобільну мережу музика не завантажується",
    "reserve": "Розумний запас",
    "reserveDesc": "Улюблене впереміш із новим — хвиля грає й без мережі",
    "reserveCount": "Хвиля без мережі: на пристрої {ready} з {total}",
    "reserveList": "Запас для хвилі",
    "forceOffline": "Без мережі",
    "forceOfflineDesc": "Тільки завантажене: застосунок не звертається до сервера",
    "remove": "Видалити все завантажене",
    "removeConfirm": "Точно видалити? Натисніть ще раз"
  },
  "en": {
    "title": "Downloads",
    "used": "{used} of {limit} used",
    "part": { "pinned": "Pinned", "reserve": "Smart reserve", "cache": "Cache" },
    "state": {
      "forced": "Offline mode: only downloads play",
      "offline": "No network — downloads resume when it's back",
      "wifi": "Waiting for Wi-Fi",
      "downloading": "Downloading {n}",
      "overLimit": "{n} don't fit — raise the limit",
      "failed": "{n} didn't download — we'll retry later",
      "done": "Everything is downloaded"
    },
    "limit": "Storage to use",
    "wifi": "Wi-Fi only",
    "wifiDesc": "No music downloads over cellular",
    "reserve": "Smart reserve",
    "reserveDesc": "Favourites mixed with new music — the wave plays offline",
    "reserveCount": "Offline wave: {ready} of {total} on this device",
    "reserveList": "Wave reserve",
    "forceOffline": "Offline mode",
    "forceOfflineDesc": "Downloads only: the app stays off the server",
    "remove": "Remove all downloads",
    "removeConfirm": "Sure? Tap again"
  }
}
</i18n>
