<script setup lang="ts">
// Политика конфиденциальности — статическая страница интерфейса: открыта без входа (ссылка со страницы
// входа, из «О приложении» и из карточки App Store: https://<сервер>/privacy). Текст один на все
// серверы DalyWave: данные хранит сервер, который выбрал человек, приложение ничего не отправляет
// третьим сторонам. Язык — как у интерфейса; ?lang=en — английский (ссылка для App Store), ?lang=uk — украинский
import type { Locale } from '~/utils/format'

definePageMeta({ layout: 'bare' })

const route = useRoute()
const i18n = useNuxtApp().$i18n
const lang = computed<Locale>(() => {
  const q = route.query.lang
  if (q === 'ru' || q === 'en' || q === 'uk') return q
  const l = i18n.locale.value
  return l === 'en' || l === 'uk' ? l : 'ru'
})
const host = typeof location !== 'undefined' ? location.host : ''

interface Section { title: string, body: string[] }
const TEXT: Record<Locale, { title: string, updated: string, lead: string, sections: Section[], back: string }> = {
  ru: {
    title: 'Конфиденциальность',
    updated: 'Обновлено 13 ноября 2026',
    lead: 'DalyWave — музыкальный сервис, который человек или его администратор ставит на свой сервер. Приложения DalyWave для iPhone, iPad, Apple TV, Mac и Windows — клиенты такого сервера: они показывают и играют музыку с сервера, адрес которого вы выбрали сами.',
    sections: [
      {
        title: 'Где хранятся данные',
        body: [
          'Всё, что описано ниже, хранится только на сервере DalyWave, к которому вы подключились, — у его владельца. Разработчик приложения к чужим серверам доступа не имеет и данных с них не получает.',
          'Если вы пользуетесь чужим сервером (например, сервером друга), уточните у его владельца, как он хранит данные и делает резервные копии.',
        ],
      },
      {
        title: 'Что хранит сервер',
        body: [
          'Аккаунт: имя, логин, хеш пароля или привязку к службе входа (OpenID Connect) и адрес почты, если служба входа его сообщила; аватар.',
          'Медиатеку и действия: лайки, плейлисты, подписки, историю прослушиваний и реакции в волне — по ним работают рекомендации.',
          'Устройства и сессии: название устройства, платформу, версию приложения, время последней активности, IP-адрес последнего входа; на iPhone и iPad — токен push-уведомлений, если вы их разрешили.',
          'Журнал действий администратора и служебные журналы сервера.',
        ],
      },
      {
        title: 'Чего приложение не делает',
        body: [
          'Не собирает аналитику и рекламные идентификаторы, не отслеживает вас в других приложениях и на сайтах, не передаёт данные третьим сторонам.',
          'Микрофон нужен только экрану «Распознать», пока он открыт: звук не записывается и не уходит на сервер DalyWave — песню по цифровому отпечатку звука узнаёт служба Shazam от Apple.',
          'Локальная сеть нужна только, чтобы найти сервер DalyWave и подключить Apple TV с iPhone. Список ваших серверов (без паролей и ключей) приложение может хранить в iCloud, чтобы Apple TV с тем же Apple ID предложил подключиться к ним.',
        ],
      },
      {
        title: 'Внешние службы',
        body: [
          'Сервер может обращаться к открытым каталогам музыки (MusicBrainz, Wikidata, Википедия, LRCLIB) за обложками, сведениями об исполнителях и текстами песен — без данных о вас. Владелец сервера может включить и другие источники; это его настройка.',
          'Вход через внешнюю службу (OpenID Connect) — по правилам этой службы. Push-уведомления доставляет Apple (APNs) или браузер. Статус «Слушает DalyWave» в Discord на компьютере появляется, только если вы его включили.',
        ],
      },
      {
        title: 'Как удалить данные',
        body: [
          'Историю прослушиваний можно очистить в разделе «История».',
          'Аккаунт удаляется в «Настройках» → «Удалить аккаунт»: вместе с ним с сервера уходят медиатека, плейлисты, история, устройства и сессии. Единственного администратора сервера удалить нельзя — сначала нужно назначить другого.',
          'Выход из аккаунта или смена сервера стирают ключ входа на этом устройстве. Удаление приложения стирает его данные на устройстве.',
        ],
      },
      {
        title: 'Дети',
        body: ['DalyWave не предназначен для детей младше 13 лет и не собирает о них данные сознательно.'],
      },
      {
        title: 'Связь',
        body: ['Вопросы о данных на конкретном сервере — его владельцу. Вопросы о приложении — разработчику: Zakhar Stupak, github.com/Dalynkaa.'],
      },
    ],
    back: 'Назад',
  },
  uk: {
    title: 'Конфіденційність',
    updated: 'Оновлено 13 листопада 2026',
    lead: 'DalyWave — музичний сервіс, який людина або її адміністратор встановлює на свій сервер. Застосунки DalyWave для iPhone, iPad, Apple TV, Mac і Windows — клієнти такого сервера: вони показують і відтворюють музику із сервера, адресу якого ви вибрали самі.',
    sections: [
      {
        title: 'Де зберігаються дані',
        body: [
          'Усе, що описано нижче, зберігається лише на сервері DalyWave, до якого ви підключилися, — у його власника. Розробник застосунку не має доступу до чужих серверів і не отримує з них даних.',
          'Якщо ви користуєтеся чужим сервером (наприклад, сервером друга), уточніть у його власника, як він зберігає дані та робить резервні копії.',
        ],
      },
      {
        title: 'Що зберігає сервер',
        body: [
          'Обліковий запис: ім’я, логін, хеш пароля або прив’язку до служби входу (OpenID Connect) та адресу електронної пошти, якщо служба входу її повідомила; аватар.',
          'Медіатеку та дії: лайки, плейлисти, підписки, історію прослуховувань і реакції у хвилі — на їх основі працюють рекомендації.',
          'Пристрої та сесії: назву пристрою, платформу, версію застосунку, час останньої активності, IP-адресу останнього входу; на iPhone та iPad — токен push-сповіщень, якщо ви їх дозволили.',
          'Журнал дій адміністратора та службові журнали сервера.',
        ],
      },
      {
        title: 'Чого застосунок не робить',
        body: [
          'Не збирає аналітику й рекламні ідентифікатори, не відстежує вас в інших застосунках і на сайтах, не передає дані третім сторонам.',
          'Мікрофон потрібен лише екрану «Розпізнати», поки він відкритий: звук не записується і не надсилається на сервер DalyWave — пісню за цифровим відбитком звуку розпізнає служба Shazam від Apple.',
          'Локальна мережа потрібна лише для того, щоб знайти сервер DalyWave і підключити Apple TV з iPhone. Список ваших серверів (без паролів і ключів) застосунок може зберігати в iCloud, щоб Apple TV з тим самим Apple ID запропонував підключитися до них.',
        ],
      },
      {
        title: 'Зовнішні служби',
        body: [
          'Сервер може звертатися до відкритих музичних каталогів (MusicBrainz, Wikidata, Вікіпедія, LRCLIB) за обкладинками, відомостями про виконавців і текстами пісень — без даних про вас. Власник сервера може ввімкнути й інші джерела; це його налаштування.',
          'Вхід через зовнішню службу (OpenID Connect) — за правилами цієї служби. Push-сповіщення доставляє Apple (APNs) або браузер. Статус «Слухає DalyWave» у Discord на комп’ютері з’являється, лише якщо ви його ввімкнули.',
        ],
      },
      {
        title: 'Як видалити дані',
        body: [
          'Історію прослуховувань можна очистити в розділі «Історія».',
          'Обліковий запис видаляється в «Налаштуваннях» → «Видалити обліковий запис»: разом із ним із сервера видаляються медіатека, плейлисти, історія, пристрої та сесії. Єдиного адміністратора сервера видалити не можна — спершу потрібно призначити іншого.',
          'Вихід з облікового запису або зміна сервера стирають ключ входу на цьому пристрої. Видалення застосунку стирає його дані на пристрої.',
        ],
      },
      {
        title: 'Діти',
        body: ['DalyWave не призначений для дітей віком до 13 років і свідомо не збирає даних про них.'],
      },
      {
        title: 'Зв’язок',
        body: ['Питання про дані на конкретному сервері — до його власника. Питання про застосунок — до розробника: Zakhar Stupak, github.com/Dalynkaa.'],
      },
    ],
    back: 'Назад',
  },
  en: {
    title: 'Privacy',
    updated: 'Updated November 13, 2026',
    lead: 'DalyWave is a music service that a person or their administrator runs on their own server. The DalyWave apps for iPhone, iPad, Apple TV, Mac and Windows are clients of such a server: they show and play music from the server whose address you chose.',
    sections: [
      {
        title: 'Where data is stored',
        body: [
          'Everything described below is stored only on the DalyWave server you connected to, by its owner. The app developer has no access to other people’s servers and receives no data from them.',
          'If you use someone else’s server (for example, a friend’s), ask its owner how they store data and make backups.',
        ],
      },
      {
        title: 'What the server stores',
        body: [
          'Account: name, username, a password hash or a link to a sign-in service (OpenID Connect), and an email address if the sign-in service provided one; avatar.',
          'Library and activity: likes, playlists, follows, listening history and wave reactions — recommendations are built from them.',
          'Devices and sessions: device name, platform, app version, last activity time, IP address of the last sign-in; on iPhone and iPad, a push notification token if you allowed notifications.',
          'The administrator action log and the server’s service logs.',
        ],
      },
      {
        title: 'What the app does not do',
        body: [
          'It does not collect analytics or advertising identifiers, does not track you across other apps and websites, and does not share data with third parties.',
          'The microphone is used only by the “Recognize” screen while it is open: audio is not recorded or sent to the DalyWave server — the song is identified from an audio fingerprint by Apple’s Shazam service.',
          'The local network is used only to find a DalyWave server and to connect an Apple TV from an iPhone. The app may keep the list of your servers (without passwords or keys) in iCloud so that an Apple TV signed in with the same Apple ID can offer to connect to them.',
        ],
      },
      {
        title: 'Outside services',
        body: [
          'The server may query open music catalogs (MusicBrainz, Wikidata, Wikipedia, LRCLIB) for artwork, artist information and lyrics — without any data about you. The server owner may enable other sources; that is their setting.',
          'Signing in with an outside service (OpenID Connect) follows that service’s terms. Push notifications are delivered by Apple (APNs) or the browser. The “Listening to DalyWave” status in Discord on a computer appears only if you turned it on.',
        ],
      },
      {
        title: 'How to delete data',
        body: [
          'Listening history can be cleared in “History”.',
          'The account is deleted in Settings → “Delete account”: the library, playlists, history, devices and sessions are removed from the server with it. The only administrator of a server cannot be deleted — appoint another one first.',
          'Signing out or switching servers erases the sign-in key on this device. Deleting the app erases its data on the device.',
        ],
      },
      {
        title: 'Children',
        body: ['DalyWave is not intended for children under 13 and does not knowingly collect data about them.'],
      },
      {
        title: 'Contact',
        body: ['Questions about data on a particular server go to its owner. Questions about the app go to the developer: Zakhar Stupak, github.com/Dalynkaa.'],
      },
    ],
    back: 'Back',
  },
}
const text = computed(() => TEXT[lang.value])
useHead({ title: () => text.value.title, htmlAttrs: { lang: () => lang.value } })

function back() {
  if (window.history.length > 1) window.history.back()
  else void navigateTo('/')
}
</script>

<template>
  <main class="pv" :lang="lang">
    <button type="button" class="pv__back" @click="back">
      <Icon name="ph:caret-left" aria-hidden="true" />{{ text.back }}
    </button>
    <BrandGlyph class="pv__glyph" />
    <h1 class="pv__title">{{ text.title }}</h1>
    <p class="pv__meta">{{ text.updated }}<template v-if="host"> · {{ host }}</template></p>
    <p class="pv__lead">{{ text.lead }}</p>
    <section v-for="s in text.sections" :key="s.title" class="pv__sec">
      <h2 class="pv__h2">{{ s.title }}</h2>
      <p v-for="(p, i) in s.body" :key="i" class="pv__p">{{ p }}</p>
    </section>
    <nav class="pv__langs" aria-label="Language">
      <NuxtLink :to="{ query: { lang: 'ru' } }" :aria-current="lang === 'ru' ? 'page' : undefined">Русский</NuxtLink>
      <NuxtLink :to="{ query: { lang: 'uk' } }" :aria-current="lang === 'uk' ? 'page' : undefined">Українська</NuxtLink>
      <NuxtLink :to="{ query: { lang: 'en' } }" :aria-current="lang === 'en' ? 'page' : undefined">English</NuxtLink>
    </nav>
  </main>
</template>

<style scoped>
.pv {
  width: min(680px, 100%);
  min-height: 100dvh;
  margin: 0 auto;
  padding: calc(var(--safe-top) + var(--titlebar-size, 0px) + 24px) 24px calc(var(--safe-bottom) + 48px);
  background: var(--stage);
  color: var(--text-1);
}
.pv__back { display: inline-flex; align-items: center; gap: 4px; min-height: 44px; margin-left: -8px; padding: 0 8px; border: 0; border-radius: var(--r-2); background: none; color: var(--text-2); font: inherit; font-size: var(--fs-small); cursor: pointer; }
.pv__back:hover { color: var(--text-1); }
.pv__back:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }
.pv__glyph { display: block; width: 36px; height: 36px; margin-top: 24px; color: var(--text-1); }
.pv__title { margin: 16px 0 0; font-size: var(--fs-h1); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1.05; }
.pv__meta { margin: 8px 0 0; color: var(--text-3); font-size: var(--fs-caption); }
.pv__lead { margin: 24px 0 0; color: var(--text-1); font-size: 17px; line-height: 1.55; }
.pv__sec { margin-top: 32px; }
.pv__h2 { margin: 0 0 8px; font-size: 17px; font-weight: 700; }
.pv__p { margin: 0 0 10px; color: var(--text-2); font-size: 15px; line-height: 1.6; }
.pv__langs { display: flex; gap: 4px; margin-top: 40px; }
.pv__langs a { display: inline-flex; align-items: center; min-height: 44px; padding: 0 12px; border-radius: var(--r-pill); color: var(--text-3); font-size: var(--fs-small); text-decoration: none; }
.pv__langs a[aria-current='page'] { background: var(--fill-1); color: var(--text-1); }
.pv__langs a:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--focus); }

@media (max-width: 699px) {
  .pv { padding-left: 20px; padding-right: 20px; }
  .pv__title { font-size: 34px; }
}
</style>
