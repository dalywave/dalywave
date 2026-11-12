<script setup lang="ts">
import type { Notification } from '~/api/types'

// Список центра уведомлений: мягкое предложение включить push на этом устройстве (один раз, пока
// система не спрашивала), уведомления новыми первыми, подгрузка при прокрутке. Общий для поповера
// в верхней строке компьютера и страницы /notifications на телефоне и iPad
withDefaults(defineProps<{ compact?: boolean }>(), { compact: false })
const emit = defineEmits<{ navigate: [] }>()

const store = useNotificationsStore()
const { tg } = useGlobalT()

function open(n: Notification, link: string | null) {
  store.open(n, link)
  if (notificationRoute(link)) emit('navigate')
}

async function enable() {
  await store.enable()
}

// Подгрузка, когда низ списка близко
const sentinel = ref<HTMLElement | null>(null)
let io: IntersectionObserver | null = null
watch(sentinel, (el) => {
  io?.disconnect()
  if (!el) return
  io = new IntersectionObserver(([e]) => {
    if (e?.isIntersecting) void store.loadMore()
  }, { rootMargin: '200px' })
  io.observe(el)
})
onBeforeUnmount(() => io?.disconnect())
</script>

<template>
  <div class="nl" :class="{ 'nl--compact': compact }">
    <div v-if="store.shouldAsk" class="ask">
      <div class="ask__text">
        <span class="ask__title">{{ tg('notify.askTitle') }}</span>
        <span class="ask__sub">{{ tg('notify.askText') }}</span>
      </div>
      <div class="ask__btns">
        <UiButton size="sm" variant="ghost" @click="store.dismissAsk()">{{ tg('notify.askLater') }}</UiButton>
        <UiButton size="sm" variant="primary" :loading="store.busy" @click="enable">{{ tg('notify.askOn') }}</UiButton>
      </div>
    </div>

    <div v-if="store.state === 'loading' && !store.items.length" class="skel" aria-busy="true">
      <div v-for="i in (compact ? 4 : 7)" :key="i" class="skel__row">
        <UiSkeleton width="44px" height="44px" />
        <div class="skel__lines"><UiSkeleton variant="line" width="52%" /><UiSkeleton variant="line" width="78%" /></div>
      </div>
    </div>
    <UiEmptyState v-else-if="store.state === 'error' && !store.items.length" icon="ph:cloud-warning" :title="tg('common.loadFailed')" tone="error" compact>
      <UiButton size="sm" icon="ph:arrow-clockwise" @click="store.load()">{{ tg('common.retry') }}</UiButton>
    </UiEmptyState>
    <UiEmptyState v-else-if="!store.items.length" icon="ph:bell-simple" :title="tg('notify.empty')" :text="tg('notify.emptyText')" compact />
    <div v-else class="nl__list" role="list">
      <NotifyRow v-for="n in store.items" :key="n.id" role="listitem" :n="n" @open="link => open(n, link)" />
      <div ref="sentinel" class="nl__sentinel" aria-hidden="true" />
      <div v-if="store.loadingMore" class="skel__row" aria-busy="true">
        <UiSkeleton width="44px" height="44px" />
        <div class="skel__lines"><UiSkeleton variant="line" width="46%" /><UiSkeleton variant="line" width="70%" /></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.nl { display: flex; flex-direction: column; min-width: 0; }
.nl__list { display: flex; flex-direction: column; }
.nl__sentinel { height: 1px; }

/* Предложение включить: подпись слева, кнопки справа; без карточки — волосок снизу */
.ask { display: flex; align-items: center; gap: 16px; padding: 12px 12px 14px; margin-bottom: 4px; box-shadow: 0 1px 0 var(--hair); }
.ask__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.ask__title { font-size: var(--fs-body); font-weight: 600; color: var(--text-1); }
.ask__sub { font-size: var(--fs-small); line-height: 1.4; color: var(--text-3); }
.ask__btns { display: flex; flex: none; gap: 4px; }
.nl--compact .ask { flex-direction: column; align-items: stretch; gap: 10px; }
.nl--compact .ask__btns { justify-content: flex-end; }

.skel { display: flex; flex-direction: column; gap: 4px; }
.skel__row { display: flex; align-items: center; gap: 12px; padding: 12px; }
.skel__lines { display: flex; flex: 1; flex-direction: column; gap: 8px; }

@container main (max-width: 520px) {
  .ask { flex-direction: column; align-items: stretch; gap: 10px; }
  .ask__btns { justify-content: flex-end; }
}
@media (max-width: 699px) {
  .ask { flex-direction: column; align-items: stretch; gap: 12px; padding: 12px 4px 16px; }
  .ask__btns { justify-content: stretch; }
  .ask__btns > :deep(*) { flex: 1; }
}
</style>
