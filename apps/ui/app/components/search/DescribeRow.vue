<script setup lang="ts">
// «Собрать плейлист: …» — строка в поиске, когда запрос похож на описание («спокойный инди под дождь»):
// ведёт на «Плейлист по описанию» с этим текстом
const props = defineProps<{ text: string }>()
const { t } = useScopedI18n('components.search.DescribeRow')
const to = computed(() => ({ path: '/describe', query: { text: props.text } }))
</script>

<template>
  <NuxtLink :to="to" class="drow">
    <span class="drow__icon"><Icon name="ph:note-pencil" /></span>
    <span class="drow__text">
      <span class="drow__title">{{ t('build') }}</span>
      <span class="drow__q ellipsis">«{{ text }}»</span>
    </span>
    <Icon name="ph:caret-right" class="drow__chev" aria-hidden="true" />
  </NuxtLink>
</template>

<style scoped>
.drow { display: flex; align-items: center; gap: 14px; min-height: 60px; padding: 8px 12px 8px 8px; border-radius: var(--r-3); color: var(--text-1); transition: background-color var(--dur-1); }
.drow:hover { background: var(--fill-1); }
.drow:focus-visible { outline: 2px solid var(--focus); outline-offset: 0; }
.drow__icon { display: grid; place-items: center; width: 44px; height: 44px; flex: none; border-radius: var(--r-2); background: var(--fill-2); font-size: 20px; color: var(--text-1); }
.drow__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.drow__title { font-size: var(--fs-body); font-weight: 600; }
.drow__q { font-size: var(--fs-small); color: var(--text-2); }
.drow__chev { flex: none; font-size: 14px; color: var(--text-3); }
@media (max-width: 699px) {
  .drow { margin-inline: -8px; }
}
</style>
