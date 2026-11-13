<script setup lang="ts">
// Оболочка приложения.
// Большой экран: боковая панель | содержимое (+ сцена плеера поверх) | правая панель; внизу — панель плеера.
// Планшет: боковая панель сжимается в полосу значков, правая панель ложится поверх.
// Телефон: документ прокручивается сам, внизу вкладки и мини-плеер, плеер и очередь — шторками.
// Планшет под пальцем (iPad), как Spotify на iPad: то же, что на телефоне, но содержимое — в колонке
// большого экрана, а внизу плавающие стеклянные вкладки и рядом карточка плеера; плеер — на весь экран.
// Горизонтально с «Плеером сбоку» — карточки нет, «Сейчас играет» колонкой справа, содержимое уже.
// Раскрытый складной iPhone, как приложения Apple на нём: слева боковое меню (разделы, плейлисты,
// артисты) во всю высоту, справа содержимое планшета под пальцем, мини-плеер — внизу колонки содержимого.
// Сложенный — телефон; переход на лету: страница не пересоздаётся, музыка не прерывается
const ui = useUiStore()
const bridge = useBridge()
const route = useRoute()

useShortcuts()

// Живые события сервера: загрузки треков, изменения медиатеки с других устройств
onMounted(() => bridge.live.connect())
// Управление между устройствами: это окно — устройство в списке «Где играть»
onMounted(() => useConnectStore().start())
// «Слушать вместе»: живые события сессии и её состояние после перезагрузки
onMounted(() => useJamStore().init())
// Настройки слушателя (фильтр нецензурного, качество, порядок плейлистов) — общие для устройств
onMounted(() => void useAccountSettings().load())
// Уведомления: центр, живые `notification.*`, push этого устройства (без запроса разрешения)
onMounted(() => useNotificationsStore().init())
// Друзья: что слушают, запросы и приватная сессия (её видно в плеере и на аватаре)
onMounted(() => useFriendsStore().init())

// Окно приложения: во весь экран «светофора» нет, без фокуса заголовок приглушён
const { win, titlebar, titlebarStyle } = useTitlebar()

// Обновление приложения качается в фоне само и ждёт «Перезапустить» в настройках («О приложении»)
// и в трее — без всплывающих предложений: ни музыку, ни работу оно не прерывает

const main = ref<HTMLElement | null>(null)
const scrolled = ref(false)

function onScroll() {
  scrolled.value = (main.value?.scrollTop ?? 0) > 8
}

// Своя прокрутка у содержимого: при переходе — наверх, при «назад» — туда, где были
const positions = new Map<string, number>()
let popped = false
onMounted(() => window.addEventListener('popstate', () => (popped = true)))
watch(() => route.fullPath, (to, from) => {
  if (from && main.value) positions.set(from, main.value.scrollTop)
  const restore = popped ? positions.get(to) ?? 0 : 0
  popped = false
  void nextTick(() => {
    if (!main.value || ui.isTouchUi) return
    main.value.scrollTop = restore
    onScroll()
  })
})

// Плеер закрывается при переходе на другую страницу; под пальцем — уезжает вниз
watch(() => route.path, () => {
  if (!ui.isTouchUi) ui.playerOpen = false
  else if (ui.playerOpen) void ui.collapsePlayer()
})

// Смена страницы: новая проявляется коротким подъёмом. Старая уходит сразу — без ожидания
// и без <Transition>, которому нужен один корень у каждой страницы
const page = ref<HTMLElement | null>(null)
watch(() => route.path, () => {
  if (ui.reducedMotion) return
  void nextTick(() => {
    // На телефоне — только прозрачность: transform на <main> сломал бы закреплённые шапки страниц
    if (ui.isTouchUi) phoneMain.value?.animate?.([{ opacity: 0 }, { opacity: 1 }], { duration: 220, easing: 'ease-out' })
    else page.value?.animate?.([{ opacity: 0, transform: 'translateY(6px)' }, { opacity: 1, transform: 'none' }], { duration: 280, easing: 'cubic-bezier(0.16, 1, 0.3, 1)' })
  })
})
const phoneMain = ref<HTMLElement | null>(null)
</script>

<template>
  <div
    class="app"
    :class="{
      'is-panel': ui.panelOpen && !ui.isTouchUi,
      'is-touch': ui.isTouch,
      'is-fold': ui.isFoldable,
      'is-side-bar': ui.isSideBar,
      'is-side': ui.sidePlayer,
      'is-rail': ui.isRail,
      'is-player': ui.playerOpen,
      [`is-titlebar-${titlebar}`]: !!titlebar,
    }"
    :style="titlebarStyle"
  >
    <ShellTitleBar v-if="titlebar" :focused="win.focused" :maximized="win.maximized" />
    <template v-if="!ui.isTouchUi">
      <ShellSidebar class="app__side" :rail="ui.isRail" />
      <div class="app__center">
        <div class="app__stage">
          <main ref="main" class="app__main thin-scrollbar" @scroll.passive="onScroll">
            <ShellTopBar :scrolled="scrolled" />
            <div ref="page" class="page">
              <slot />
            </div>
          </main>
          <MusicFullPlayer variant="stage" />
        </div>
        <ShellRightPanel class="app__panel" />
      </div>
      <MusicNowPlayingBar class="app__bar" />
    </template>
    <template v-else>
      <!-- Раскрытый складной iPhone: слева боковое меню, как у приложений Apple на нём -->
      <ShellSidebar v-if="ui.isFoldable" touch class="app__fold-side" />
      <main ref="phoneMain" class="app__main app__main--phone">
        <!-- Планшет: колонка большого экрана — страницы раскладываются по ней через @container main.
             Телефон — та же обёртка без своей коробки: складной iPhone переходит из одной раскладки
             в другую, не пересоздавая страницу (прокрутка и всё, что на ней набрано, остаются) -->
        <div :class="ui.isTouch ? 'page page--touch' : 'page-phone'"><slot /></div>
      </main>
      <MusicMiniPlayer v-if="ui.isFoldable" fold />
      <template v-else-if="ui.isTouch">
        <ShellDock />
        <MusicSidePlayer />
      </template>
      <template v-else>
        <MusicMiniPlayer />
        <ShellTabBar />
      </template>
      <MusicFullPlayer :variant="ui.isTouch ? 'tablet' : 'phone'" />
      <ShellPhoneSheets />
    </template>
    <!-- «Распознать»: экран прослушивания и лист результата (приложение для iPhone и Mac) -->
    <RecognizeLayer />
  </div>
</template>

<style scoped>
.app {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: var(--sidebar-w) minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr) calc(var(--bar-h) + var(--safe-bottom));
  height: 100vh;
  height: 100dvh;
  /* iPad: под строкой состояния; на компьютере отступов нет */
  padding-top: calc(var(--titlebar-h) + var(--safe-top));
}
.app.is-rail { --sidebar-w: 76px; }
.app.is-titlebar-mac-inset { --titlebar-inset: var(--titlebar-size, 48px); }
.app.is-titlebar-windows { --titlebar-h: var(--titlebar-size, 32px); }
.app__side { grid-row: 1; grid-column: 1; }
.app__center { position: relative; display: flex; grid-row: 1; grid-column: 2; min-width: 0; min-height: 0; }
.app__stage { position: relative; display: flex; flex: 1; min-width: 0; min-height: 0; }
.app__main { position: relative; flex: 1; min-width: 0; min-height: 0; overflow-x: hidden; overflow-y: auto; }
.app__panel { flex: none; }
.app__bar { grid-row: 2; grid-column: 1 / -1; }
/* Ширину колонки меняет не только окно, но и правая панель — содержимое страниц подстраивается
   под колонку через @container main, а не под окно */
.page { position: relative; min-height: 100%; padding: calc(var(--topbar-h) + 8px) 32px 64px; container: main / inline-size; }

@media (max-width: 1279px) {
  .app:not(.is-rail) { --sidebar-w: 232px; }
}
@media (max-width: 1023px) {
  .page { padding: calc(var(--topbar-h) + 8px) 20px 48px; }
}

/* Планшет под пальцем: документ прокручивается сам, низ — под плавающими вкладками и плеером */
.app.is-touch { display: block; height: auto; min-height: 100dvh; padding-top: 0; }
.app.is-touch .app__main--phone { display: block; overflow: visible; padding: var(--safe-top) var(--safe-right) calc(var(--safe-bottom) + var(--dock-h) + 40px) var(--safe-left); }
.page--touch { min-height: 0; padding: 20px 32px 0; }
/* Плеер сбоку: содержимое сужается до колонки слева, страницы раскладываются по ней сами */
.app.is-side .app__main--phone { padding-right: calc(var(--side-w) + var(--side-gap) + var(--safe-right)); }
.app.is-side .page--touch { padding: 20px 28px 0; }
/* Под строкой состояния содержимое не просвечивает: тихая полоса фона, как у Spotify */
.app.is-touch::before { content: ''; position: fixed; inset: 0 0 auto; z-index: 25; height: var(--safe-top); background: color-mix(in srgb, var(--ink-1) 86%, transparent); -webkit-backdrop-filter: blur(20px); backdrop-filter: blur(20px); pointer-events: none; }

/* Телефон: документ прокручивается сам. Сложенный складной iPhone держит строку состояния сбоку —
   отступы слева и справа тоже из безопасной зоны */
.page-phone { display: contents; }
@media (max-width: 699px) {
  .app { display: block; height: auto; min-height: 100dvh; padding-top: 0; }
  .app__main--phone { display: block; overflow: visible; padding: var(--safe-top) var(--safe-right) calc(var(--safe-bottom) + var(--tabbar-h) + var(--mini-h) + 32px) var(--safe-left); }
  /* Вкладки — в колонке справа (сложенный складной iPhone): снизу только мини-плеер */
  .app.is-side-bar .app__main--phone { padding-bottom: calc(var(--safe-bottom) + var(--mini-h) + 32px); }
}

/* Раскрытый складной iPhone: боковое меню закреплено слева во всю высоту, документ прокручивается
   справа от него; внизу — только мини-плеер (вкладки — в меню) */
.app.is-fold { --fold-side-w: clamp(232px, 27vw, 268px); }
.app.is-fold .app__main--phone { padding: var(--safe-top) var(--safe-right) calc(var(--safe-bottom) + var(--mini-h) + 40px) calc(var(--fold-side-w) + var(--safe-left)); }
.app.is-fold::before { left: calc(var(--fold-side-w) + var(--safe-left)); }
.app__fold-side { position: fixed; top: 0; bottom: 0; left: 0; z-index: 30; width: calc(var(--fold-side-w) + var(--safe-left)); height: auto; }
.app.is-fold .page--touch { padding: 16px 24px 0; }
</style>
