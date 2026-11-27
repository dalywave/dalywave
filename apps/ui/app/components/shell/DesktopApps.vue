<script setup lang="ts">
import type { AppDownload } from '~/api/types'

// «Приложения для компьютера» с сайта: меню аккаунта и «Настройки → О приложении». Mac и Windows
// рядом, система этого компьютера — первой и с основной кнопкой. Ссылка ведёт на сервер, он
// отвечает редиректом на файл в хранилище под именем DalyWave-<версия>.dmg. Ниже — плагин Stream Deck
// (один файл для Mac и Windows): после скачивания — подсказка, как подключить. На телефоне — лист снизу
const open = defineModel<boolean>('open', { default: false })
const bridge = useBridge()
const ui = useUiStore()
const { t, locale } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const apps = ref<AppDownload[] | null>(null)
const failed = ref(false)
watch(open, async (v) => {
  if (!v) return
  failed.value = false
  try {
    apps.value = (await bridge.desktopApps()).apps
  }
  catch {
    failed.value = true
  }
}, { immediate: true })

type Os = 'mac' | 'windows'
function thisComputer(): Os | null {
  if (!import.meta.client || bridge.platform !== 'web') return null
  const hint = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform ?? navigator.platform ?? ''
  const ua = navigator.userAgent
  if (/mac/i.test(hint) || /Macintosh/.test(ua)) return 'mac'
  if (/win/i.test(hint) || /Windows/.test(ua)) return 'windows'
  return null
}
const current = thisComputer()

const cards = computed(() => {
  const find = (p: string) => apps.value?.find(a => a.platform === p) ?? null
  const mac = find('mac')
  const win = find('windows')
  // Требования — из выпуска (сервер сверяет их со сборкой), до первого выпуска — нынешние
  const list = [
    { os: 'mac' as const, icon: 'ph:apple-logo', name: 'Mac', req: t('macReq', { v: mac?.minOsVersion ?? '13' }), app: mac, intel: find('mac-intel') },
    { os: 'windows' as const, icon: 'ph:windows-logo', name: 'Windows', req: t('winReq', { v: win?.minOsVersion ?? '10' }), app: win, intel: null },
  ]
  return current === 'windows' ? list.reverse() : list
})

const deck = computed(() => apps.value?.find(a => a.platform === 'streamdeck') ?? null)
/** Плагин скачан: подсказать, что дальше */
const deckTaken = ref(false)

const meta = (a: AppDownload) => `${a.version} · ${formatBytes(a.sizeBytes, locale.value === 'en' ? 'en' : locale.value === 'uk' ? 'uk' : 'ru')}`
</script>

<template>
  <UiModal v-model:open="open" :title="tg('nav.desktopApps')" :width="560">
    <div class="da" :class="{ 'da--phone': ui.isPhone }">
      <section v-for="c in cards" :key="c.os" class="da__app" :class="{ 'is-current': c.os === current }">
        <Icon :name="c.icon" class="da__logo" aria-hidden="true" />
        <div class="da__text">
          <h3 class="da__name">{{ c.name }}</h3>
          <p class="da__req">{{ c.req }}</p>
        </div>
        <div class="da__get">
          <template v-if="c.app">
            <UiButton
              :href="c.app.url"
              :variant="c.os === current ? 'primary' : 'secondary'"
              :size="ui.isPhone ? 'lg' : 'md'"
              icon="ph:download-simple"
              block
              :aria-label="`${t('download')} ${c.name}`"
            >
              {{ t('download') }}
            </UiButton>
            <p class="da__meta tnum">{{ meta(c.app) }}</p>
            <a v-if="c.intel" :href="c.intel.url" class="da__alt">{{ t('intel') }}</a>
          </template>
          <UiButton v-else-if="!apps && !failed" :size="ui.isPhone ? 'lg' : 'md'" block loading />
          <p v-else class="da__soon">{{ failed ? t('failed') : t('soon') }}</p>
        </div>
      </section>
      <section class="da__deck">
        <Icon name="ph:squares-four" class="da__logo" aria-hidden="true" />
        <div class="da__text">
          <h3 class="da__name">Stream Deck</h3>
          <p class="da__req">{{ t('deckWhat') }}</p>
          <p class="da__req">{{ t('deckReq', { v: deck?.minOsVersion ?? '7.6' }) }}</p>
        </div>
        <div class="da__get">
          <template v-if="deck">
            <UiButton
              :href="deck.url"
              variant="secondary"
              :size="ui.isPhone ? 'lg' : 'md'"
              icon="ph:download-simple"
              block
              :aria-label="`${t('download')} Stream Deck`"
              @click="deckTaken = true"
            >
              {{ t('download') }}
            </UiButton>
            <p class="da__meta tnum">{{ meta(deck) }}</p>
          </template>
          <UiButton v-else-if="!apps && !failed" :size="ui.isPhone ? 'lg' : 'md'" block loading />
          <p v-else class="da__soon">{{ failed ? t('failed') : t('soon') }}</p>
        </div>
        <p v-if="deckTaken" class="da__next" role="status">{{ t('deckNext') }}</p>
      </section>
    </div>
  </UiModal>
</template>

<style scoped>
/* Две колонки через волосок, плагин Stream Deck — строкой под ними; на телефоне — одно под другим */
/* Поля — как у заголовка окна: 20px */
.da { display: grid; grid-template-columns: 1fr 1fr; padding: 8px 20px 20px; }
.da__app { display: flex; flex-direction: column; gap: 14px; min-width: 0; padding: 4px 24px 0 0; }
.da__app + .da__app { padding: 4px 0 0 24px; box-shadow: inset 1px 0 0 var(--hair); }
.da__logo { font-size: 30px; color: var(--text-2); }
.is-current .da__logo { color: var(--text-1); }
.da__text { display: flex; flex-direction: column; gap: 2px; }
.da__name { margin: 0; font-size: var(--fs-h4); font-weight: 650; letter-spacing: -0.01em; }
.da__req { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.da__get { display: flex; flex-direction: column; gap: 8px; }
.da__meta, .da__soon { margin: 0; font-size: var(--fs-caption); color: var(--text-3); text-align: center; }
.da__soon { display: grid; place-items: center; height: 40px; border-radius: var(--r-pill); background: var(--fill-1); font-size: var(--fs-small); color: var(--text-3); }
.da__alt { align-self: center; font-size: var(--fs-caption); color: var(--text-2); text-decoration: underline; text-decoration-color: var(--line-strong); text-underline-offset: 3px; }
.da__alt:hover { color: var(--text-1); }
.da__deck { grid-column: 1 / -1; display: grid; grid-template-columns: auto 1fr 168px; align-items: center; gap: 4px 16px; margin-top: 22px; padding-top: 18px; box-shadow: inset 0 1px 0 var(--hair); }
.da__deck .da__logo { font-size: 26px; }
.da__deck .da__meta { margin-top: -2px; }
.da__next { grid-column: 2 / -1; margin: 8px 0 0; font-size: var(--fs-small); color: var(--text-2); text-wrap: pretty; }

.da--phone { grid-template-columns: 1fr; padding: 0 20px 8px; }
.da--phone .da__app, .da--phone .da__app + .da__app { display: grid; grid-template-columns: auto 1fr; align-items: center; gap: 4px 14px; padding: 16px 0; }
.da--phone .da__app + .da__app { box-shadow: inset 0 1px 0 var(--hair); }
.da--phone .da__logo { font-size: 28px; }
.da--phone .da__get { grid-column: 1 / -1; margin-top: 12px; }
.da--phone .da__soon { height: 50px; }
.da--phone .da__deck { grid-template-columns: auto 1fr; gap: 4px 14px; margin-top: 0; padding: 16px 0; }
.da--phone .da__deck .da__logo { font-size: 28px; }
.da--phone .da__next { grid-column: 1 / -1; margin-top: 4px; }
</style>

<i18n lang="json">
{
  "ru": {
    "macReq": "Apple silicon, macOS {v} и новее",
    "winReq": "Windows {v} и новее, 64 бит",
    "download": "Скачать",
    "intel": "Для Mac с Intel",
    "soon": "Скоро",
    "failed": "Не получилось загрузить",
    "deckWhat": "Плагин для Stream Deck и Stream Deck +",
    "deckReq": "Для Mac и Windows, Stream Deck {v} и новее",
    "deckNext": "Откройте файл — Stream Deck установит плагин и профиль «DalyWave». Нажмите любую кнопку DalyWave и войдите в открывшемся окне"
  },
  "uk": {
    "macReq": "Apple silicon, macOS {v} і новіші",
    "winReq": "Windows {v} і новіші, 64 біт",
    "download": "Завантажити",
    "intel": "Для Mac з Intel",
    "soon": "Незабаром",
    "failed": "Не вдалося завантажити",
    "deckWhat": "Плагін для Stream Deck і Stream Deck +",
    "deckReq": "Для Mac і Windows, Stream Deck {v} і новіші",
    "deckNext": "Відкрийте файл — Stream Deck встановить плагін і профіль «DalyWave». Натисніть будь-яку кнопку DalyWave й увійдіть у вікні, що відкриється"
  },
  "en": {
    "macReq": "Apple silicon, macOS {v} or later",
    "winReq": "Windows {v} or later, 64-bit",
    "download": "Download",
    "intel": "For Intel-based Macs",
    "soon": "Coming soon",
    "failed": "Couldn't load",
    "deckWhat": "Plugin for Stream Deck and Stream Deck +",
    "deckReq": "For Mac and Windows, Stream Deck {v} or later",
    "deckNext": "Open the file and Stream Deck will install the plugin and the DalyWave profile. Press any DalyWave key and sign in in the window that opens"
  }
}
</i18n>
