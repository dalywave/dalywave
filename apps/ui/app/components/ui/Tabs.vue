<script setup lang="ts" generic="T extends string">
import { TabsList, TabsRoot, TabsTrigger } from 'reka-ui'

defineProps<{
  items: { value: T, label: string, count?: number }[]
  label: string
}>()

const model = defineModel<T>({ required: true })

function update(v: string | number) {
  model.value = v as T
}
</script>

<template>
  <TabsRoot :model-value="model" class="tabs" activation-mode="automatic" @update:model-value="update">
    <TabsList class="tabs__list" :aria-label="label">
      <TabsTrigger v-for="it in items" :key="it.value" :value="it.value" class="tabs__item">
        {{ it.label }}<span v-if="it.count != null" class="tabs__count tnum">{{ it.count }}</span>
      </TabsTrigger>
    </TabsList>
  </TabsRoot>
</template>

<style scoped>
.tabs__list { display: flex; gap: 20px; border-bottom: 1px solid var(--hair); overflow-x: auto; scrollbar-width: none; }
.tabs__list::-webkit-scrollbar { display: none; }
.tabs__item {
  position: relative;
  height: 44px;
  flex: none;
  border: 0;
  background: transparent;
  font-size: var(--fs-body);
  font-weight: 600;
  color: var(--text-3);
  transition: color var(--dur-1);
  white-space: nowrap;
}
.tabs__item:hover { color: var(--text-1); }
.tabs__item[data-state='active'] { color: var(--text-1); }
.tabs__item[data-state='active']::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  border-radius: 2px;
  background: var(--text-1);
}
.tabs__count { color: var(--text-3); font-weight: 500; margin-left: 5px; }
</style>
