<script setup lang="ts">
import type { Friend } from '~/api/types'

// Строка друга: аватар, имя, что играет (или играло — и давно ли), откуда. Имя ведёт на страницу друга,
// трек — на страницу трека, «откуда» — на альбом, плейлист или артиста; открыт Jam — «Слушать вместе».
// `panel` — правая панель компьютера, `list` — раздел «Друзья» и телефон (крупнее, с обложкой)
const props = withDefaults(defineProps<{ friend: Friend, variant?: 'panel' | 'list' }>(), { variant: 'panel' })

const friends = useFriendsStore()
const ui = useUiStore()
const lang = useLang()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

const a = computed(() => props.friend.activity ?? null)
const when = computed(() => {
  void friends.now
  if (!a.value) return ''
  return a.value.now ? tg('friends.now') : notificationTime(a.value.at, new Date(), lang.value)
})
const artists = computed(() => a.value?.track.artists.map(x => x.name).join(', ') ?? '')
const ctxIcon = computed(() => {
  switch (a.value?.context?.type) {
    case 'playlist': return 'ph:playlist'
    case 'artist': return 'ph:user-sound'
    default: return 'ph:vinyl-record'
  }
})
const ctxTo = computed(() => (a.value?.context ? `/${a.value.context.type}/${a.value.context.id}` : null))
const jamTo = computed(() => (a.value?.jamCode ? `/jam/${a.value.jamCode}` : null))
</script>

<template>
  <div class="fa" :class="[`fa--${variant}`, { 'is-now': a?.now, 'is-quiet': !a }]">
    <NuxtLink :to="`/friends/${friend.person.id}`" class="fa__who" :aria-label="friend.person.name">
      <FriendsAvatar :person="friend.person" :size="variant === 'list' ? 48 : 40" :live="!!a?.now" />
    </NuxtLink>
    <div class="fa__body">
      <div class="fa__line">
        <NuxtLink :to="`/friends/${friend.person.id}`" class="fa__name ellipsis">{{ friend.person.name }}</NuxtLink>
        <span v-if="when" class="fa__when tnum" :class="{ 'is-now': a?.now }">{{ when }}</span>
      </div>
      <template v-if="a">
        <NuxtLink :to="`/track/${a.track.id}`" class="fa__track ellipsis" @click="ui.collapsePlayer()">
          {{ a.track.title }}<span class="fa__dot"> · </span><span class="fa__artist">{{ artists }}</span>
        </NuxtLink>
        <NuxtLink v-if="a.context && ctxTo" :to="ctxTo" class="fa__ctx ellipsis">
          <Icon :name="ctxIcon" class="fa__ctx-icon" />{{ a.context.title }}
        </NuxtLink>
        <UiButton v-if="jamTo" :to="jamTo" size="sm" icon="ph:users-three" class="fa__jam">{{ tg('friends.listenTogether') }}</UiButton>
      </template>
      <span v-else class="fa__none">{{ t('nothing') }}</span>
    </div>
    <NuxtLink v-if="variant === 'list' && a" :to="`/track/${a.track.id}`" class="fa__art" tabindex="-1" aria-hidden="true">
      <MusicCoverArt :image="a.track.album?.image" size="small" :alt="''" radius="8px" />
    </NuxtLink>
  </div>
</template>

<style scoped>
.fa { position: relative; display: flex; align-items: flex-start; gap: 12px; min-height: 56px; padding: 10px 8px; border-radius: var(--r-3); }
.fa--panel:hover { background: var(--fill-1); }
.fa__who { flex: none; border-radius: 50%; }
.fa__who:focus-visible, .fa a:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: var(--r-1); }
.fa__body { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.fa__line { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
.fa__name { flex: 1; min-width: 0; font-weight: 650; color: var(--text-1); }
.fa__name:hover { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.fa__when { flex: none; font-size: var(--fs-caption); color: var(--text-3); }
.fa__when.is-now { color: var(--accent); font-weight: 600; }
.fa__track { font-size: var(--fs-small); color: var(--text-1); }
.fa__track:hover .fa__artist, .fa__track:hover { color: var(--text-1); }
.fa__dot, .fa__artist { color: var(--text-2); }
.fa__ctx { display: flex; align-items: center; gap: 5px; font-size: var(--fs-caption); color: var(--text-3); }
.fa__ctx:hover { color: var(--text-1); }
.fa__ctx-icon { flex: none; font-size: 13px; }
.fa__none { font-size: var(--fs-small); color: var(--text-3); }
.fa__jam { align-self: flex-start; margin-top: 6px; }
.fa__art { width: 48px; height: 48px; flex: none; align-self: center; }
.fa.is-quiet .fa__name { color: var(--text-2); }

.fa--list { gap: 14px; padding: 10px 0; min-height: 68px; }
.fa--list .fa__body { align-self: center; }
.fa--list .fa__name { font-size: var(--fs-body); }

@media (pointer: coarse) {
  .fa__ctx, .fa__track { min-height: 22px; }
}
</style>

<i18n lang="json">
{
  "ru": { "nothing": "Пока тихо" },
  "uk": { "nothing": "Поки тихо" },
  "en": { "nothing": "Nothing yet" }
}
</i18n>
