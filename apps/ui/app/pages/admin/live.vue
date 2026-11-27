<script setup lang="ts">
// Сейчас: кто в сети и что слушает — живьём. Сводка сверху, ниже карточки людей: слушающие первыми.
// В приватной сессии вместо трека — «Приватная сессия»
definePageMeta({ layout: 'admin' })

const lang = useLang()
const { t } = useScopedI18n('pages.admin.live')
useHead({ title: () => t('title') })
const { data, failed, load } = useAdminLiveFeed()
const now = useTicker()
const bridge = useBridge()

const stats = computed(() => {
  const s = data.value?.summary
  if (!s) return []
  return [
    { key: 'online', value: s.online },
    { key: 'listening', value: s.listening },
    { key: 'plays', value: s.plays24h },
    { key: 'newUsers', value: s.newUsers7d },
  ]
})
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
      <div class="ap-head__side">
        <AdminStatus v-if="bridge.live.state === 'open'" tone="ok" pulse>{{ t('live') }}</AdminStatus>
      </div>
    </header>

    <div v-if="stats.length" class="ap-stats" style="--n: 4">
      <div v-for="s in stats" :key="s.key" class="ap-stat">
        <span class="ap-stat__value">{{ formatNumber(s.value, lang) }}</span>
        <span class="ap-stat__label">{{ t(`stats.${s.key}`) }}</span>
      </div>
    </div>

    <AdminSection :title="t('people')">
      <AdminState :loading="!data && !failed" :error="failed && !data" :empty="!!data && !data.people.length" empty-icon="ph:moon-stars" :empty-title="t('empty')" :empty-text="t('emptyText')" :rows="3" @retry="load">
        <div class="lv__grid">
          <AdminLivePerson v-for="p in data?.people" :key="p.user.id" :person="p" :now="now" />
        </div>
      </AdminState>
    </AdminSection>
  </div>
</template>

<style scoped>
.lv__grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 12px; }
@media (max-width: 699px) {
  .lv__grid { grid-template-columns: minmax(0, 1fr); }
}
</style>
