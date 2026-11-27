<script setup lang="ts">
import type { AppReleases, VersionShare } from '~/api/types'

// Выпуски: последняя версия приложений для Mac и Windows и плагина Stream Deck, история выпусков
// и кто на какой версии (приложения, заходившие за 60 дней), отставшие — отмечены
definePageMeta({ layout: 'admin' })

const bridge = useBridge()
const lang = useLang()
const { t } = useScopedI18n('pages.admin.releases')
useHead({ title: () => t('title') })

const data = ref<AppReleases | null>(null)
const failed = ref(false)
async function load() {
  try {
    data.value = await bridge.admin.releases()
    failed.value = false
  }
  catch {
    failed.value = true
  }
}
onMounted(load)

const TARGET_ICON: Record<string, string> = { darwin: 'ph:apple-logo', windows: 'ph:windows-logo', linux: 'ph:linux-logo', streamdeck: 'ph:squares-four' }
const PLATFORM_ICON: Record<string, string> = { macos: 'ph:apple-logo', windows: 'ph:windows-logo', linux: 'ph:linux-logo', ios: 'ph:device-mobile', ipados: 'ph:device-tablet', watchos: 'ph:watch', tvos: 'ph:television-simple', streamdeck: 'ph:squares-four', android: 'ph:android-logo', web: 'ph:browser' }
const target = (k: string) => t(`target.${k}`)
const platform = (k: string) => t(`platform.${k}`)
const outdated = computed(() => data.value?.installs.filter(i => i.outdated).reduce((s, i) => s + i.devices, 0) ?? 0)
// Группы по платформе: версии внутри — новые первыми
const groups = computed(() => {
  const out: { platform: string, items: VersionShare[], devices: number }[] = []
  for (const i of data.value?.installs ?? []) {
    let g = out.find(x => x.platform === i.platform)
    if (!g) out.push(g = { platform: i.platform, items: [], devices: 0 })
    g.items.push(i)
    g.devices += i.devices
  }
  return out
})
</script>

<template>
  <div class="ap">
    <header class="ap-head">
      <h1 class="ap-title">{{ t('title') }}</h1>
    </header>

    <AdminState :loading="!data && !failed" :error="failed && !data" :rows="5" @retry="load">
      <template v-if="data">
        <div class="ap-stats" :style="{ '--n': data.latest.length + 2 }">
          <div class="ap-stat"><span class="ap-stat__value">{{ data.serverVersion }}</span><span class="ap-stat__label">{{ t('server') }}</span></div>
          <div v-for="l in data.latest" :key="l.target" class="ap-stat">
            <span class="ap-stat__value">{{ l.version }}</span>
            <span class="ap-stat__label">{{ target(l.target) }} · {{ formatRelative(l.publishedAt, lang) }}</span>
          </div>
          <div class="ap-stat" :class="{ 'is-warn': outdated }"><span class="ap-stat__value tnum">{{ outdated }}</span><span class="ap-stat__label">{{ t('outdated') }}</span></div>
        </div>

        <AdminSection :title="t('installs.title')" :hint="t('installs.hint')">
          <AdminState :empty="!groups.length" empty-icon="ph:devices" :empty-title="t('installs.empty')">
            <div class="ap-table">
              <template v-for="g in groups" :key="g.platform">
                <div class="ap-group"><span class="re__plat"><Icon :name="PLATFORM_ICON[g.platform] ?? 'ph:devices'" />{{ platform(g.platform) }}</span><span class="tnum">{{ t('devices', { n: g.devices }) }}</span></div>
                <div v-for="i in g.items" :key="`${g.platform}-${i.version}`" class="ap-tr re__row">
                  <span class="ap-cell"><b>{{ i.version ?? t('unknown') }}</b></span>
                  <span class="re__bar"><i :style="{ width: `${(i.devices / Math.max(1, g.devices)) * 100}%` }" :class="{ 'is-old': i.outdated }" /></span>
                  <span class="ap-num tnum">{{ t('devices', { n: i.devices }) }} · {{ t('people', { n: i.people }) }}</span>
                  <span class="re__state"><AdminStatus v-if="i.outdated" tone="warn">{{ t('old') }}</AdminStatus></span>
                </div>
              </template>
            </div>
          </AdminState>
        </AdminSection>

        <AdminSection :title="t('history.title')">
          <AdminState :empty="!data.items.length" empty-icon="ph:package" :empty-title="t('history.empty')" :empty-text="t('history.emptyText')">
            <div class="ap-table">
              <div class="ap-tr ap-tr--head re__hist">
                <span>{{ t('history.version') }}</span>
                <span>{{ t('history.target') }}</span>
                <span class="ap-num ap-hide-phone">{{ t('history.size') }}</span>
                <span class="ap-num">{{ t('history.when') }}</span>
              </div>
              <div v-for="r in data.items" :key="r.id" class="ap-tr re__hist">
                <span class="ap-cell"><b>{{ r.version }}</b><small v-if="r.notes">{{ r.notes }}</small></span>
                <span class="re__plat ap-muted"><Icon :name="TARGET_ICON[r.target] ?? 'ph:package'" />{{ target(r.target) }} · {{ r.arch }}<Icon v-if="r.installer" name="ph:download-simple" :title="t('history.installer')" /></span>
                <span class="ap-num ap-muted ap-hide-phone">{{ formatBytes(r.sizeBytes, lang) }}</span>
                <span class="ap-num ap-muted">{{ formatShortDate(r.publishedAt, lang) }}</span>
              </div>
            </div>
          </AdminState>
        </AdminSection>
      </template>
    </AdminState>
  </div>
</template>

<style scoped>
.re__plat { display: inline-flex; align-items: center; gap: 8px; }
.re__plat :deep(.iconify) { font-size: 16px; }
.re__row { --cols: 120px minmax(0, 1fr) 200px 120px; }
.re__bar { height: 6px; overflow: hidden; border-radius: 3px; background: var(--fill-1); }
.re__bar i { display: block; height: 100%; border-radius: inherit; background: var(--success); }
.re__bar i.is-old { background: var(--warning); }
.re__state { display: flex; justify-content: flex-end; }
.re__hist { --cols: minmax(0, 1fr) minmax(0, 1fr) 100px 100px; }
@media (max-width: 1099px) {
  .ap-stats { grid-template-columns: repeat(3, minmax(0, 1fr)); row-gap: 20px; }
}
@media (max-width: 699px) {
  .re__row { --cols: 90px minmax(0, 1fr) auto; }
  .re__row .re__bar { display: none; }
  .re__hist { --cols: minmax(0, 1fr) minmax(0, 1fr) auto; }
}
</style>
