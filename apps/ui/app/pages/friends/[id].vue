<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Страница друга: аватар и имя, что слушает сейчас (или последнее), совпадение вкусов и общие
// артисты, топ артистов за месяц и плейлисты, открытые друзьям. Не друг — имя и «Добавить в друзья»
const route = useRoute()
const bridge = useBridge()
const friends = useFriendsStore()
const player = usePlayerStore()
const pui = usePlaylistUi()
const lang = useLang()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

const id = computed(() => String(route.params.id))
const { data, state, load } = useLoad(() => bridge.friends.profile(id.value), () => id.value)
useHead({ title: () => data.value?.person.name ?? tg('friends.title') })

// Список друзей изменился (приняли, удалили) — перечитать страницу
watch(() => friends.list, () => {
  if (data.value) void bridge.friends.profile(id.value).then(p => (data.value = p)).catch(() => {})
})
// Друг сменил трек — живое событие уже в списке друзей
const activity = computed(() => {
  const live = friends.friends.find(f => f.person.id === id.value)
  return live ? live.activity : data.value?.activity ?? null
})

const p = computed(() => data.value?.person ?? null)
const isFriend = computed(() => data.value?.relation === 'friend')
const since = computed(() => {
  const at = data.value?.since
  if (!at) return ''
  return t('since', { date: formatShortDate(at, lang.value) })
})
const kicker = computed(() => (isFriend.value ? t('friend') : t('person')))
const initial = computed(() => (p.value?.name ?? '?').trim().charAt(0).toUpperCase())

const when = computed(() => {
  void friends.now
  const a = activity.value
  if (!a) return ''
  return a.now ? t('nowPlaying') : t('lastPlayed', { when: formatRelative(a.at, lang.value) })
})

function playActivity() {
  const tr = activity.value?.track
  if (!tr) return
  if (player.current?.track.id === tr.id) return player.toggle()
  return player.playTracks([tr], 0, { type: 'queue', id: null, title: p.value?.name ?? '' })
}

const match = computed(() => data.value?.tasteMatch ?? null)
const matchParts = computed(() => {
  const m = match.value
  if (!m) return []
  return [
    m.artists != null ? t('byArtists', { n: Math.round(m.artists * 100) }) : null,
    m.sound != null ? t('bySound', { n: Math.round(m.sound * 100) }) : null,
  ].filter(Boolean) as string[]
})

async function unfriend() {
  if (!p.value) return
  const ok = await pui.confirm({ title: t('removeTitle', { name: p.value.name }), text: t('removeText'), action: t('remove'), danger: true })
  if (ok) await friends.remove(p.value)
}
const more = computed<MenuEntry[]>(() => (p.value && isFriend.value
  ? [{ label: t('remove'), icon: 'ph:user-minus', danger: true, onSelect: () => void unfriend() }]
  : []))
const busy = computed(() => !!p.value && friends.busy.has(p.value.id))
</script>

<template>
  <div class="friend">
    <template v-if="state !== 'ready'">
      <MusicPhoneNav fallback="/friends" />
      <MusicPageState :state="state" @retry="load" />
    </template>
    <template v-else-if="data && p">
      <MusicPageHeader :kicker="kicker" :title="p.name" round back="/friends">
        <template #art>
          <span class="friend__art">
            <img v-if="p.avatarUrl" :src="p.avatarUrl" alt="" class="friend__img">
            <span v-else class="friend__initial" aria-hidden="true">{{ initial }}</span>
          </span>
        </template>
        <template v-if="since" #meta>
          <span>{{ since }}</span>
        </template>
        <template #actions>
          <template v-if="isFriend">
            <UiButton v-if="activity?.jamCode" variant="primary" size="lg" icon="ph:users-three" :to="`/jam/${activity.jamCode}`">{{ tg('friends.listenTogether') }}</UiButton>
          </template>
          <UiButton v-else-if="data.relation === 'none'" variant="primary" size="lg" icon="ph:user-plus" :loading="busy" @click="friends.request(p)">{{ tg('friends.add') }}</UiButton>
          <template v-else-if="data.relation === 'incoming'">
            <UiButton variant="primary" size="lg" icon="ph:check" :loading="busy" @click="friends.accept(p)">{{ t('accept') }}</UiButton>
            <UiButton size="lg" :disabled="busy" @click="friends.decline(p)">{{ t('decline') }}</UiButton>
          </template>
          <UiButton v-else-if="data.relation === 'outgoing'" size="lg" icon="ph:clock" :loading="busy" @click="friends.cancel(p)">{{ t('sent') }}</UiButton>
        </template>
        <template v-if="more.length" #tools>
          <UiMenu :items="more" align="start">
            <UiIconButton icon="ph:dots-three" :label="tg('common.more')" size="lg" :tooltip="false" />
          </UiMenu>
        </template>
      </MusicPageHeader>

      <template v-if="isFriend">
        <!-- Что слушает сейчас или слушал последним -->
        <section v-if="activity" class="now" :class="{ 'is-now': activity.now }">
          <button type="button" class="now__art" :aria-label="`${tg('common.play')}: ${activity.track.title}`" @click="playActivity">
            <MusicCoverArt :image="activity.track.album?.image" size="medium" :alt="''" radius="10px" />
            <span class="now__play" aria-hidden="true"><Icon :name="player.current?.track.id === activity.track.id && player.playing ? 'ph:pause-fill' : 'ph:play-fill'" /></span>
          </button>
          <div class="now__text">
            <span class="now__label">
              <span v-if="activity.now" class="now__eq" aria-hidden="true"><i /><i /><i /></span>{{ when }}
            </span>
            <NuxtLink :to="`/track/${activity.track.id}`" class="now__title ellipsis">{{ activity.track.title }}</NuxtLink>
            <span class="now__artist ellipsis">{{ activity.track.artists.map(a => a.name).join(', ') }}</span>
            <NuxtLink v-if="activity.context" :to="`/${activity.context.type}/${activity.context.id}`" class="now__ctx ellipsis">{{ activity.context.title }}</NuxtLink>
          </div>
        </section>

        <!-- Совпадение вкусов: доля общих артистов и близость звучания любимого -->
        <section class="match">
          <div class="match__num tnum">{{ match ? `${match.percent}%` : '—' }}</div>
          <div class="match__text">
            <h2 class="match__title">{{ t('match') }}</h2>
            <p class="match__hint">{{ match ? matchParts.join(' · ') : t('matchUnknown') }}</p>
            <div v-if="match" class="match__bar" role="img" :aria-label="`${match.percent}%`"><span :style="{ width: `${Math.max(2, match.percent)}%` }" /></div>
          </div>
        </section>

        <MusicShelf v-if="data.commonArtists.length" :title="t('common')" :cols="6" phone-width="120px">
          <MusicArtistCard v-for="a in data.commonArtists" :key="a.id" :artist="a" />
        </MusicShelf>

        <MusicShelf v-if="data.topArtists.length" :title="t('top')" :cols="6" phone-width="120px">
          <MusicArtistCard v-for="a in data.topArtists" :key="a.artist.id" :artist="a.artist" :meta="formatCount(a.plays, 'play', lang)" />
        </MusicShelf>

        <MusicShelf v-if="data.playlists.length" :title="t('playlists')" :cols="5">
          <MusicMixCard v-for="pl in data.playlists" :key="pl.id" :playlist="pl" :meta="formatCount(pl.tracksCount, 'track', lang)" />
        </MusicShelf>

        <UiEmptyState v-if="!activity && !data.topArtists.length && !data.playlists.length" icon="ph:music-notes-simple" :title="t('quietTitle')" :text="t('quietText')" compact />
      </template>
      <UiEmptyState v-else icon="ph:lock-simple" :title="t('closedTitle')" :text="t('closedText')" compact />
    </template>
  </div>
</template>

<style scoped>
.friend { display: flex; flex-direction: column; gap: 36px; }
.friend__art { display: grid; place-items: center; width: 100%; aspect-ratio: 1; overflow: hidden; border-radius: 50%; background: var(--fill-3); box-shadow: var(--shadow-cover); }
.friend__img { width: 100%; height: 100%; object-fit: cover; }
.friend__initial { font-size: clamp(48px, 8vw, 96px); font-weight: 750; color: var(--text-1); }

.now { display: flex; align-items: center; gap: 18px; max-width: 640px; }
.now__art { position: relative; width: 96px; height: 96px; flex: none; padding: 0; border: 0; border-radius: 10px; background: none; }
.now__art:focus-visible { outline: 2px solid var(--focus); outline-offset: 3px; }
.now__play { position: absolute; inset: 0; display: grid; place-items: center; border-radius: 10px; background: rgba(4, 5, 7, 0.42); color: #fff; font-size: 30px; opacity: 0; transition: opacity var(--dur-2); }
.now__art:hover .now__play, .now__art:focus-visible .now__play { opacity: 1; }
.now__text { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
.now__label { display: inline-flex; align-items: center; gap: 7px; font-size: var(--fs-caption); font-weight: 650; letter-spacing: var(--tr-caps); text-transform: uppercase; color: var(--text-3); }
.now.is-now .now__label { color: var(--accent); }
.now__eq { display: inline-flex; align-items: flex-end; gap: 2px; width: 12px; height: 11px; }
.now__eq i { flex: 1; height: 100%; border-radius: 1px; background: currentColor; transform-origin: bottom; animation: eq-bounce 1s var(--ease-in-out) infinite; }
.now__eq i:nth-child(2) { animation-delay: -0.35s; animation-duration: 0.8s; }
.now__eq i:nth-child(3) { animation-delay: -0.6s; animation-duration: 1.15s; }
.now__title { font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.now__title:hover, .now__ctx:hover { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.now__artist { color: var(--text-2); }
.now__ctx { font-size: var(--fs-small); color: var(--text-3); }

.match { display: flex; align-items: center; gap: 20px; max-width: 640px; }
.match__num { flex: none; min-width: 3ch; font-size: clamp(44px, 6vw, 64px); font-weight: 750; letter-spacing: var(--tr-display); line-height: 1; }
.match__text { display: flex; flex: 1; flex-direction: column; gap: 4px; min-width: 0; }
.match__title { margin: 0; font-size: var(--fs-h4); font-weight: 700; }
.match__hint { margin: 0; font-size: var(--fs-small); color: var(--text-2); }
.match__bar { height: 4px; margin-top: 6px; overflow: hidden; border-radius: 2px; background: var(--fill-2); }
.match__bar span { display: block; height: 100%; border-radius: 2px; background: var(--text-1); }

@media (max-width: 699px) {
  .friend { gap: 28px; }
  .now, .match { padding: 0 16px; }
  .now__art { width: 72px; height: 72px; }
  .now__play { opacity: 1; background: rgba(4, 5, 7, 0.28); font-size: 24px; }
  .friend > :deep(.empty) { padding-left: 16px; padding-right: 16px; }
}
@media (pointer: coarse) {
  .now__play { opacity: 1; background: rgba(4, 5, 7, 0.28); }
}
</style>

<i18n lang="json">
{
  "ru": {
    "person": "Профиль",
    "friend": "Друг",
    "since": "В друзьях с {date}",
    "nowPlaying": "Сейчас играет",
    "lastPlayed": "Последнее · {when}",
    "match": "Совпадение вкусов",
    "matchUnknown": "Пока мало прослушиваний, чтобы сравнить",
    "byArtists": "общие артисты {n}%",
    "bySound": "звучание {n}%",
    "common": "Ваши общие артисты",
    "top": "Топ за месяц",
    "playlists": "Плейлисты",
    "quietTitle": "Пока тихо",
    "quietText": "Здесь появится, что слушает ваш друг",
    "closedTitle": "Видно только друзьям",
    "closedText": "Что слушает человек и его плейлисты появятся, когда вы станете друзьями",
    "accept": "Принять",
    "decline": "Отклонить",
    "sent": "Запрос отправлен",
    "remove": "Удалить из друзей",
    "removeTitle": "Удалить {name} из друзей?",
    "removeText": "Вы перестанете видеть, что слушает каждый из вас"
  },
  "en": {
    "person": "Profile",
    "friend": "Friend",
    "since": "Friends since {date}",
    "nowPlaying": "Playing now",
    "lastPlayed": "Last played · {when}",
    "match": "Taste match",
    "matchUnknown": "Not enough listening yet to compare",
    "byArtists": "shared artists {n}%",
    "bySound": "sound {n}%",
    "common": "Artists you both like",
    "top": "Top this month",
    "playlists": "Playlists",
    "quietTitle": "Nothing yet",
    "quietText": "What your friend plays will show up here",
    "closedTitle": "Only friends can see this",
    "closedText": "Become friends to see what they play and their playlists",
    "accept": "Accept",
    "decline": "Decline",
    "sent": "Request sent",
    "remove": "Remove friend",
    "removeTitle": "Remove {name} from friends?",
    "removeText": "You’ll both stop seeing what the other plays"
  }
}
</i18n>
