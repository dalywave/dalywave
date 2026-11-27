<script setup lang="ts">
// «Что слушают друзья» в правой панели компьютера (как Friend Activity у Spotify): играющие сейчас —
// первыми, затем по свежести; запросы в друзья — строкой сверху, всё остальное — в разделе «Друзья»
const friends = useFriendsStore()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

onMounted(() => {
  friends.init()
  void friends.load()
})

const quiet = computed(() => friends.friends.filter(f => !f.activity))
</script>

<template>
  <div class="fp">
    <NuxtLink v-if="friends.incoming.length" to="/friends" class="fp__requests">
      <span class="fp__stack" aria-hidden="true">
        <FriendsAvatar v-for="r in friends.incoming.slice(0, 3)" :key="r.person.id" :person="r.person" :size="28" />
      </span>
      <span class="fp__req-text">{{ t('requests', { n: friends.incoming.length }) }}</span>
      <Icon name="ph:caret-right" class="fp__chev" />
    </NuxtLink>

    <template v-if="friends.friends.length">
      <FriendsActivityRow v-for="f in friends.withActivity" :key="f.person.id" :friend="f" variant="panel" />
      <FriendsActivityRow v-for="f in quiet" :key="f.person.id" :friend="f" variant="panel" />
      <UiButton to="/friends" variant="ghost" size="sm" icon-right="ph:caret-right" class="fp__all">{{ tg('friends.all') }}</UiButton>
    </template>
    <UiEmptyState v-else-if="friends.list" icon="ph:users-three" :title="tg('friends.empty')" :text="t('emptyText')" compact>
      <UiButton to="/friends" icon="ph:user-plus">{{ t('find') }}</UiButton>
    </UiEmptyState>
    <div v-else-if="!friends.failed" class="fp__skel" aria-hidden="true">
      <div v-for="n in 4" :key="n" class="fp__skel-row">
        <UiSkeleton variant="round" width="40px" height="40px" />
        <div class="fp__skel-lines">
          <UiSkeleton variant="line" width="40%" height="12px" />
          <UiSkeleton variant="line" width="70%" height="12px" />
        </div>
      </div>
    </div>
    <UiEmptyState v-else icon="ph:cloud-warning" :title="t('failed')" compact tone="error">
      <UiButton icon="ph:arrow-clockwise" @click="friends.load()">{{ t('retry') }}</UiButton>
    </UiEmptyState>
  </div>
</template>

<style scoped>
.fp { display: flex; flex-direction: column; gap: 2px; padding: 8px 0; }
.fp__requests { display: flex; align-items: center; gap: 12px; min-height: 52px; margin-bottom: 6px; padding: 8px 10px; border-radius: var(--r-3); background: var(--fill-1); font-weight: 600; }
.fp__requests:hover { background: var(--fill-2); }
.fp__stack { display: flex; flex: none; }
.fp__stack > * + * { margin-left: -10px; box-shadow: 0 0 0 2px var(--ink-2); }
.fp__req-text { flex: 1; min-width: 0; font-size: var(--fs-small); }
.fp__chev { flex: none; color: var(--text-3); }
.fp__all { align-self: flex-start; margin: 8px 0 0 4px; }
.fp__skel { display: flex; flex-direction: column; gap: 18px; padding: 10px 8px; }
.fp__skel-row { display: flex; align-items: center; gap: 12px; }
.fp__skel-lines { display: flex; flex: 1; flex-direction: column; gap: 8px; }
</style>

<i18n lang="json">
{
  "ru": {
    "requests": "Запросы в друзья: {n}",
    "emptyText": "Добавьте друзей — здесь будет видно, что они слушают",
    "find": "Найти друзей",
    "failed": "Не загрузилось",
    "retry": "Повторить"
  },
  "uk": {
    "requests": "Запити на дружбу: {n}",
    "emptyText": "Додайте друзів — тут буде видно, що вони слухають",
    "find": "Знайти друзів",
    "failed": "Не завантажилося",
    "retry": "Повторити"
  },
  "en": {
    "requests": "Friend requests: {n}",
    "emptyText": "Add friends to see what they’re playing",
    "find": "Find friends",
    "failed": "Didn’t load",
    "retry": "Retry"
  }
}
</i18n>
