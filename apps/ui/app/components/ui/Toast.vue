<script setup lang="ts">
// Неподвижное уведомление для галереи; живые показывает UiToaster
defineProps<{
  text: string
  icon?: string
  tone?: 'default' | 'error'
  action?: string
}>()
</script>

<template>
  <div class="toast" :class="{ 'toast--error': tone === 'error' }" role="status">
    <Icon :name="icon ?? 'ph:check-circle-fill'" class="toast__icon" />
    <span class="toast__text">{{ text }}</span>
    <button v-if="action" type="button" class="toast__action">{{ action }}</button>
  </div>
</template>
