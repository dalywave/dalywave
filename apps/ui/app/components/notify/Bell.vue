<script setup lang="ts">
// Колокольчик центра уведомлений с числом непрочитанных. Компьютер — в верхней строке, открывает
// панель со списком; телефон и iPad — ссылка на страницу /notifications (крупная цель под палец)
withDefaults(defineProps<{ size?: 'sm' | 'lg' }>(), { size: 'sm' })

const store = useNotificationsStore()
const ui = useUiStore()
const { tg } = useGlobalT()

const open = ref(false)
const count = computed(() => (store.unread > 99 ? '99+' : String(store.unread)))
const label = computed(() => (store.unread ? `${tg('notify.title')}: ${tg('notify.unread', { n: store.unread })}` : tg('notify.title')))

// Открыли панель — свежий список
watch(open, (v) => {
  if (v) void store.load()
})
</script>

<template>
  <NuxtLink v-if="ui.isTouchUi" to="/notifications" class="bell" :class="`bell--${size}`" :aria-label="label">
    <Icon name="ph:bell-simple" class="bell__icon" />
    <span v-if="store.unread" class="bell__count tnum" aria-hidden="true">{{ count }}</span>
  </NuxtLink>
  <UiPopover v-else v-model:open="open" side="bottom" align="end" :width="380" no-autofocus>
    <template #trigger>
      <button type="button" class="bell" :class="[`bell--${size}`, { 'is-open': open }]" :aria-label="label">
        <Icon name="ph:bell-simple" class="bell__icon" />
        <span v-if="store.unread" class="bell__count tnum" aria-hidden="true">{{ count }}</span>
      </button>
    </template>
    <template #default="{ close }">
      <header class="bp__head">
        <span class="bp__title">{{ tg('notify.title') }}</span>
        <UiButton v-if="store.unread" size="sm" variant="ghost" @click="store.markAllRead()">{{ tg('notify.readAll') }}</UiButton>
      </header>
      <NotifyList compact @navigate="close()" />
    </template>
  </UiPopover>
</template>

<style scoped>
.bell {
  position: relative;
  display: grid;
  place-items: center;
  flex: none;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: var(--fill-1);
  color: var(--text-2);
  transition: color var(--dur-1), background-color var(--dur-1);
}
.bell:hover, .bell.is-open { color: var(--text-1); background: var(--fill-2); }
.bell:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.bell__icon { font-size: 18px; }
.bell--lg { width: 44px; height: 44px; background: transparent; color: var(--text-1); }
.bell--lg .bell__icon { font-size: 24px; }
.bell__count {
  position: absolute;
  top: -3px;
  left: calc(50% + 3px);
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: var(--r-pill);
  background: var(--accent);
  box-shadow: 0 0 0 2px var(--ink-1);
  color: var(--on-light);
  font-size: 11px;
  font-weight: 700;
  line-height: 18px;
  text-align: center;
}
.bell--lg .bell__count { top: 4px; left: calc(50% + 4px); }

.bp__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 40px; padding: 4px 4px 6px 12px; }
.bp__title { font-size: var(--fs-body); font-weight: 650; color: var(--text-1); }
</style>
