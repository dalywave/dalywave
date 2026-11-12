<script setup lang="ts">
// Центр уведомлений страницей: телефон и iPad (колокольчик на главной), ссылка из собранного
// уведомления («3 новых релиза») и из push. На компьютере тот же список открывается и из верхней строки
const store = useNotificationsStore()
const ui = useUiStore()
const { tg } = useGlobalT()
useHead({ title: () => tg('notify.title') })

onMounted(() => {
  store.init()
  void store.load()
})

// Телефон: название уехало под верхнюю строку — она показывает его сама
const titleEl = ref<HTMLElement | null>(null)
const condensed = ref(false)
let tio: IntersectionObserver | null = null
watch(titleEl, (el) => {
  tio?.disconnect()
  if (!el) return
  tio = new IntersectionObserver(([e]) => {
    if (e) condensed.value = !e.isIntersecting && e.boundingClientRect.top < 100
  }, { rootMargin: '-100px 0px 0px 0px' })
  tio.observe(el)
})
onBeforeUnmount(() => tio?.disconnect())
</script>

<template>
  <div class="notes">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="tg('notify.title')" :condensed="condensed">
      <UiIconButton v-if="store.unread" icon="ph:checks" :label="tg('notify.readAll')" size="lg" :tooltip="false" @click="store.markAllRead()" />
    </MusicPhoneNav>
    <header class="notes__head">
      <h1 ref="titleEl" class="notes__title">{{ tg('notify.title') }}</h1>
      <UiButton v-if="store.unread && !ui.isTouchUi" size="sm" variant="secondary" icon="ph:checks" @click="store.markAllRead()">{{ tg('notify.readAll') }}</UiButton>
    </header>
    <NotifyList class="notes__list" />
  </div>
</template>

<style scoped>
.notes { display: flex; flex-direction: column; gap: 12px; width: 100%; max-width: 720px; margin: 0 auto; padding-top: 16px; }
.notes__head { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 44px; }
.notes__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.notes__list { margin: 0 -12px; }

@media (max-width: 699px) {
  .notes { gap: 8px; padding: 0 16px; }
  .notes > .pn { margin-right: -16px; margin-bottom: -8px; margin-left: -16px; }
  .notes__title { font-size: 28px; }
  .notes__list { margin: 0 -12px; }
}
</style>
