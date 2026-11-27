<script setup lang="ts">
import type { EnvSetting, SettingsSection } from '~/api/types'

// Настройки раздела, заданные переменными окружения сервера: видны, но меняются только на
// сервере — с пометкой. Секреты — без значения, только «задан». Список один на страницу: грузится
// раз и делится между вкладками
const props = defineProps<{ section: SettingsSection }>()
const { t } = useScopedI18n('components.admin.EnvList')
const bridge = useBridge()
const all = useState<EnvSetting[] | null>('admin-env', () => null)
const failed = ref(false)
onMounted(async () => {
  if (all.value) return
  try {
    all.value = (await bridge.admin.envSettings()).items
  }
  catch {
    failed.value = true
  }
})
// Сначала заданные; незаданные — свёрнуты: их полезно знать, но не разглядывать
const items = computed(() => (all.value ?? []).filter(i => i.section === props.section))
const set = computed(() => items.value.filter(i => i.set))
const unset = computed(() => items.value.filter(i => !i.set))
const showUnset = ref(false)
</script>

<template>
  <SettingsGroup v-if="items.length" :title="t('title')" :footnote="t('note')">
    <SettingsRow v-for="i in set" :key="i.name" :title="i.name" class="env__row">
      <span class="env__value" :class="{ 'is-secret': i.secret }">
        <template v-if="i.secret"><Icon name="ph:lock-simple" />{{ t('hidden') }}</template>
        <template v-else>{{ i.value }}</template>
      </span>
    </SettingsRow>
    <SettingsRow v-if="unset.length" as="button" :title="showUnset ? t('hideUnset') : t('showUnset', { n: unset.length })" @click="showUnset = !showUnset">
      <Icon :name="showUnset ? 'ph:caret-up' : 'ph:caret-down'" class="env__caret" />
    </SettingsRow>
    <template v-if="showUnset">
      <SettingsRow v-for="i in unset" :key="i.name" :title="i.name" indent class="env__row">
        <span class="env__value is-unset">{{ t('unset') }}</span>
      </SettingsRow>
    </template>
  </SettingsGroup>
  <p v-else-if="failed" class="ap-hint">{{ t('failed') }}</p>
</template>

<style scoped>
.env__row :deep(.sr__title) { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12.5px; }
.env__value { display: inline-flex; align-items: center; gap: 6px; max-width: 360px; overflow: hidden; font-size: var(--fs-small); color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.env__value.is-secret, .env__value.is-unset { color: var(--text-3); }
.env__caret { color: var(--text-3); }
@media (max-width: 699px) {
  .env__value { max-width: 160px; }
}
</style>
