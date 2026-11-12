<script setup lang="ts">
// Пустой экран подсказывает следующий шаг, а не просто «ничего нет»
withDefaults(defineProps<{
  icon: string
  title: string
  text?: string
  tone?: 'default' | 'error'
  compact?: boolean
}>(), { tone: 'default' })
</script>

<template>
  <div class="empty" :class="{ 'empty--error': tone === 'error', 'empty--compact': compact }" role="status">
    <div class="empty__icon"><Icon :name="icon" /></div>
    <div class="empty__title">{{ title }}</div>
    <p v-if="text" class="empty__text">{{ text }}</p>
    <div v-if="$slots.default" class="empty__actions"><slot /></div>
  </div>
</template>

<style scoped>
.empty { display: flex; flex-direction: column; align-items: center; text-align: center; gap: 8px; padding: 40px 24px; }
.empty--compact { padding: 24px 16px; }
.empty__icon {
  display: grid;
  place-items: center;
  width: 64px;
  height: 64px;
  margin-bottom: 8px;
  border-radius: 20px;
  background: var(--fill-1);
  box-shadow: inset 0 0 0 1px var(--line);
  color: var(--accent);
  font-size: 28px;
}
.empty--error .empty__icon { color: var(--danger); }
.empty__title { font-size: var(--fs-h4); font-weight: 650; letter-spacing: -0.01em; text-wrap: balance; }
.empty__text { margin: 0; max-width: 340px; color: var(--text-2); font-size: var(--fs-small); text-wrap: pretty; }
.empty__actions { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; margin-top: 12px; }
</style>
