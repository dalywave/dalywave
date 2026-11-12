<script setup lang="ts">
import type { NuxtError } from '#app'

// Сбой, после которого страница не может показаться: спокойный экран без оболочки и одна кнопка.
// Неизвестные адреса сюда не попадают — их ловит pages/[...slug].vue внутри оболочки
const props = defineProps<{ error: NuxtError }>()
const { t } = useI18n()

const missing = computed(() => props.error.statusCode === 404)
useHead({ title: () => (missing.value ? t('common.notFound') : t('common.crashed')) })

function home() {
  void clearError({ redirect: '/' })
}
function reload() {
  window.location.reload()
}
</script>

<template>
  <div class="err">
    <BrandGlyph class="err__glyph" />
    <h1 class="err__title">{{ missing ? t('common.notFound') : t('common.crashed') }}</h1>
    <p class="err__text">{{ missing ? t('common.notFoundText') : t('common.crashedText') }}</p>
    <div class="err__actions">
      <UiButton v-if="!missing" variant="primary" icon="ph:arrow-clockwise" @click="reload">{{ t('common.reload') }}</UiButton>
      <UiButton :variant="missing ? 'primary' : 'secondary'" icon="ph:house" @click="home">{{ t('nav.home') }}</UiButton>
    </div>
  </div>
</template>

<style scoped>
.err {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 100vh;
  min-height: 100dvh;
  padding: calc(var(--safe-top) + 32px) 24px calc(var(--safe-bottom) + 32px);
  text-align: center;
}
.err__glyph { width: 44px; height: 44px; margin-bottom: 16px; color: var(--text-3); }
.err__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); text-wrap: balance; }
.err__text { max-width: 340px; margin: 0; color: var(--text-2); text-wrap: pretty; }
.err__actions { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; margin-top: 20px; }
</style>
