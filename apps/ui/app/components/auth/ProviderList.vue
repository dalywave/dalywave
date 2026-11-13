<script setup lang="ts">
import type { OidcProvider } from '~/api/types'

// Кнопки входа через провайдеров OIDC: значок и название — из настроек сервера. Одна кнопка без
// формы пароля — главная (белая), иначе — второстепенные, одна под другой
const props = withDefaults(defineProps<{
  providers: OidcProvider[]
  primary?: boolean
  next?: string
  app?: boolean
  invite?: string
  /** Подпись вместо «Войти через …» (например, «Попробовать снова») */
  label?: string
}>(), { primary: false, next: '/', app: false, invite: undefined, label: undefined })

const bridge = useBridge()
const { t } = useI18n({ useScope: 'local' })
const failed = ref(new Set<string>())

function href(p: OidcProvider) {
  return bridge.auth.providerUrl(p.id, { next: props.next, app: props.app, invite: props.invite })
}
</script>

<template>
  <div class="pl">
    <UiButton
      v-for="p in providers"
      :key="p.id"
      :variant="primary ? 'primary' : 'secondary'"
      size="lg"
      block
      :href="href(p)"
    >
      <span class="pl__label">
        <img v-if="p.icon && !failed.has(p.id)" class="pl__icon" :src="p.icon" alt="" @error="failed.add(p.id)">
        {{ label ?? t('with', { title: p.title }) }}
      </span>
    </UiButton>
  </div>
</template>

<style scoped>
.pl { display: flex; flex-direction: column; gap: 10px; width: 100%; }
.pl__label { display: inline-flex; align-items: center; justify-content: center; gap: 10px; min-width: 0; }
.pl__icon { flex: none; width: 20px; height: 20px; border-radius: 5px; object-fit: contain; }
</style>

<i18n lang="json">
{
  "ru": { "with": "Войти через {title}" },
  "en": { "with": "Sign in with {title}" }
}
</i18n>
