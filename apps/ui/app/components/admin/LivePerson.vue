<script setup lang="ts">
import type { LivePerson } from '~/api/types'

// Человек в «Сейчас»: аватар и имя (ссылка на его страницу), устройства на связи и что играет —
// обложка, трек, исполнитель, источник и полоса позиции, которая идёт сама. Приватная сессия —
// без трека. `compact` — строка для «Обзора», без полосы
const props = defineProps<{
  person: LivePerson
  /** Текущее время для позиции (общий тикер страницы) */
  now: number
  compact?: boolean
  /** Без имени и аватара: на странице самого человека */
  bare?: boolean
}>()
const { t } = useI18n({ useScope: 'global' })
const P = usePeopleLabels()

const ICON: Record<string, string> = {
  laptop: 'ph:laptop',
  desktop: 'ph:desktop',
  phone: 'ph:device-mobile',
  tablet: 'ph:device-tablet',
  web: 'ph:browser',
}
const n = computed(() => props.person.now)
const track = computed(() => n.value?.track ?? null)
const device = computed(() => props.person.devices.find(d => d.active) ?? null)
const pos = computed(() => (n.value ? livePosition(n.value, props.now) : 0))
const pct = computed(() => (n.value?.durationMs ? Math.min(100, (pos.value / n.value.durationMs) * 100) : 0))
const source = computed(() => P.context(n.value?.context))
</script>

<template>
  <article class="lp" :class="{ 'lp--compact': compact, 'lp--bare': bare, 'is-paused': n && !n.playing }">
    <NuxtLink v-if="!bare" :to="`/admin/users/${person.user.id}`" class="lp__who">
      <span class="lp__avatar">
        <img v-if="person.user.avatarUrl" :src="person.user.avatarUrl" alt="">
        <span v-else>{{ person.user.name.charAt(0).toUpperCase() }}</span>
        <i class="lp__online" />
      </span>
      <span class="lp__name">{{ person.user.name }}</span>
    </NuxtLink>

    <div class="lp__now">
      <template v-if="n?.private">
        <span class="lp__cover lp__cover--private"><Icon name="ph:eye-slash" /></span>
        <span class="lp__track">
          <b>{{ t('admin.people.private') }}</b>
          <small>{{ device?.name }}</small>
        </span>
      </template>
      <template v-else-if="n && track">
        <MusicCoverArt class="lp__cover" :image="track.album?.image" size="small" radius="var(--r-2)" />
        <span class="lp__track">
          <b>{{ track.title }}</b>
          <small>{{ artistNames(track.artists) }}</small>
          <small v-if="!compact && source" class="lp__source">{{ source }}</small>
        </span>
        <span class="lp__state">
          <Icon v-if="!n.playing" name="ph:pause-fill" :title="t('admin.people.paused')" />
          <span v-else class="lp__eq" aria-hidden="true"><i /><i /><i /></span>
        </span>
      </template>
      <span v-else class="lp__idle">{{ t('admin.people.idle') }}</span>
    </div>

    <div v-if="n && !n.private && !compact" class="lp__bar">
      <span class="lp__time tnum">{{ formatDuration(pos) }}</span>
      <span class="lp__track-line"><i :style="{ width: `${pct}%` }" /></span>
      <span class="lp__time tnum">{{ formatDuration(n.durationMs) }}</span>
    </div>

    <ul v-if="!compact" class="lp__devices">
      <li v-for="d in person.devices" :key="d.id" :class="{ 'is-active': d.active }">
        <Icon :name="ICON[d.kind] ?? 'ph:browser'" />{{ d.name }}<template v-if="d.appVersion"> · {{ d.appVersion }}</template>
      </li>
    </ul>
  </article>
</template>

<style scoped>
.lp { display: flex; flex-direction: column; gap: 12px; min-width: 0; padding: 16px; border-radius: var(--r-4); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--hair); }
.lp--bare { padding: 0; background: none; box-shadow: none; }
.lp--compact { display: grid; grid-template-columns: minmax(120px, 0.6fr) minmax(0, 1fr); align-items: center; gap: 16px; padding: 8px 0; border-radius: 0; background: none; box-shadow: none; border-top: 1px solid var(--hair); }
.lp--compact:first-child { border-top: 0; }
.lp__who { display: flex; align-items: center; gap: 10px; min-width: 0; font-weight: 650; color: var(--text-1); }
.lp__who:hover .lp__name { text-decoration: underline; text-underline-offset: 3px; }
.lp__who:focus-visible { outline: none; border-radius: var(--r-2); box-shadow: 0 0 0 2px var(--focus); }
.lp__avatar { position: relative; display: grid; place-items: center; width: 32px; height: 32px; flex: none; border-radius: 50%; background: var(--fill-2); font-size: var(--fs-small); font-weight: 700; color: var(--text-2); }
.lp__avatar img { width: 100%; height: 100%; border-radius: 50%; object-fit: cover; }
.lp__online { position: absolute; right: -1px; bottom: -1px; width: 10px; height: 10px; border-radius: 50%; background: var(--success); box-shadow: 0 0 0 2px var(--ink-1); }
.lp__name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.lp__now { display: flex; align-items: center; gap: 12px; min-width: 0; min-height: 48px; }
.lp--compact .lp__now { min-height: 40px; }
.lp__cover { width: 48px; height: 48px; flex: none; }
.lp--compact .lp__cover { width: 40px; height: 40px; }
.lp__cover--private { display: grid; place-items: center; border-radius: var(--r-2); background: var(--fill-2); font-size: 20px; color: var(--text-3); }
.lp__track { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.lp__track b { overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.lp__track small { overflow: hidden; font-size: var(--fs-small); color: var(--text-2); text-overflow: ellipsis; white-space: nowrap; }
.lp__track .lp__source { font-size: var(--fs-caption); color: var(--text-3); }
.lp__state { display: grid; place-items: center; width: 24px; flex: none; color: var(--text-3); }
.lp__eq { display: flex; align-items: flex-end; gap: 2px; height: 14px; }
.lp__eq i { width: 3px; border-radius: 1px; background: var(--accent); animation: lp-eq 1s var(--ease-in-out) infinite; }
.lp__eq i:nth-child(2) { animation-delay: -0.35s; }
.lp__eq i:nth-child(3) { animation-delay: -0.7s; }
@keyframes lp-eq { 0%, 100% { height: 30%; } 50% { height: 100%; } }
@media (prefers-reduced-motion: reduce) { .lp__eq i { height: 70%; animation: none; } }
.lp__idle { font-size: var(--fs-small); color: var(--text-3); }
.lp__bar { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 10px; }
.lp__time { font-size: 11px; color: var(--text-3); }
.lp__track-line { position: relative; height: 3px; overflow: hidden; border-radius: 2px; background: var(--fill-2); }
.lp__track-line i { position: absolute; inset: 0 auto 0 0; border-radius: inherit; background: var(--text-1); transition: width 1s linear; }
.is-paused .lp__track-line i { background: var(--text-3); }
.lp__devices { display: flex; flex-wrap: wrap; gap: 6px 14px; margin: 0; padding: 0; list-style: none; font-size: var(--fs-caption); color: var(--text-3); }
.lp__devices li { display: inline-flex; align-items: center; gap: 6px; min-width: 0; }
.lp__devices li.is-active { color: var(--text-1); font-weight: 600; }
.lp__devices :deep(.iconify) { flex: none; font-size: 15px; }
@media (max-width: 699px) {
  .lp--compact { grid-template-columns: minmax(0, 1fr); gap: 6px; }
}
</style>
