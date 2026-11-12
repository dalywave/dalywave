<script setup lang="ts">
// Оболочка приложения.
// Большой экран: боковая панель | содержимое (+ сцена плеера поверх) | правая панель; внизу — панель плеера.
// Планшет: боковая панель сжимается в полосу значков, правая панель ложится поверх.
// Телефон: документ прокручивается сам, внизу вкладки и мини-плеер, плеер и очередь — шторками
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

// Окно приложения: во весь экран «светофора» нет, без фокуса заголовок приглушён
const win = reactive({ maximized: false, focused: true, fullscreen: false })
let offWin: (() => void) | null = null
onMounted(() => {
  if (bridge.isDesktopApp) offWin = bridge.window.onState(s => Object.assign(win, s))
})
onBeforeUnmount(() => offWin?.())

// Обновление приложения качается в фоне само и ждёт «Перезапустить» в настройках («О приложении»)
// и в трее — без всплывающих предложений: ни музыку, ни работу оно не прерывает

const main = ref<HTMLElement | null>(null)
const scrolled = ref(false)
const titlebar = computed(() => (bridge.isDesktopApp && bridge.chrome && bridge.chrome.controls !== 'none' ? bridge.chrome.controls : null))
const titlebarStyle = computed(() => (bridge.chrome ? { '--titlebar-size': `${bridge.chrome.height}px` } : undefined))

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
    if (!main.value || ui.isPhone) return
    main.value.scrollTop = restore
    onScroll()
  })
})

// Плеер закрывается при переходе на другую страницу; на телефоне — уезжает вниз
watch(() => route.path, () => {
  if (!ui.isPhone) ui.playerOpen = false
  else if (ui.playerOpen) void ui.collapsePlayer()
})

// Смена страницы: новая проявляется коротким подъёмом. Старая уходит сразу — без ожидания
// и без <Transition>, которому нужен один корень у каждой страницы
const page = ref<HTMLElement | null>(null)
watch(() => route.path, () => {
  if (ui.reducedMotion) return
  void nextTick(() => {
    // На телефоне — только прозрачность: transform на <main> сломал бы закреплённые шапки страниц
    if (ui.isPhone) phoneMain.value?.animate?.([{ opacity: 0 }, { opacity: 1 }], { duration: 220, easing: 'ease-out' })
    else page.value?.animate?.([{ opacity: 0, transform: 'translateY(6px)' }, { opacity: 1, transform: 'none' }], { duration: 280, easing: 'cubic-bezier(0.16, 1, 0.3, 1)' })
  })
})
const phoneMain = ref<HTMLElement | null>(null)
</script>

<template>
  <div
    class="app"
    :class="{
      'is-panel': ui.panelOpen && !ui.isPhone,
      'is-rail': ui.isRail,
      'is-player': ui.playerOpen,
      [`is-titlebar-${titlebar}`]: titlebar && !win.fullscreen,
    }"
    :style="titlebarStyle"
  >
    <ShellTitleBar v-if="titlebar && !win.fullscreen" :focused="win.focused" :maximized="win.maximized" />
    <template v-if="!ui.isPhone">
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
      <main ref="phoneMain" class="app__main app__main--phone">
        <slot />
      </main>
      <MusicMiniPlayer />
      <ShellTabBar />
      <MusicFullPlayer variant="phone" />
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
  grid-template-rows: minmax(0, 1fr) var(--bar-h);
  height: 100vh;
  height: 100dvh;
  padding-top: var(--titlebar-h);
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

/* Телефон: документ прокручивается сам */
@media (max-width: 699px) {
  .app { display: block; height: auto; min-height: 100dvh; padding-top: 0; }
  .app__main--phone { display: block; overflow: visible; padding: var(--safe-top) 0 calc(var(--safe-bottom) + var(--tabbar-h) + var(--mini-h) + 32px); }
}
</style>
