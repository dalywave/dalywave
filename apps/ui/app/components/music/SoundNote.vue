<script setup lang="ts">
// Звук должен идти, а система его не берёт (аудиосервер завис): движок ждёт и повторяет сам,
// потом продолжает с того же места. Без подробностей — только что происходит
withDefaults(defineProps<{ long?: boolean }>(), {})
const { t } = useI18n()
</script>

<template>
  <span class="snd" :class="{ 'snd--long': long }" role="status">
    <span class="snd__spin" aria-hidden="true" />
    <span class="snd__text">{{ long ? t('player.soundRetryingLong') : t('player.soundRetrying') }}</span>
  </span>
</template>

<style scoped>
.snd { display: inline-flex; align-items: center; gap: 6px; min-width: 0; color: var(--text-2); font-size: var(--fs-caption); font-weight: 550; }
.snd__text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.snd--long .snd__text { white-space: normal; text-wrap: balance; }
.snd__spin { width: 11px; height: 11px; flex: none; border-radius: 50%; border: 1.5px solid color-mix(in oklab, currentColor 30%, transparent); border-top-color: currentColor; animation: spin 1.4s linear infinite; }
</style>
