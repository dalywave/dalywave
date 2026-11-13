<script setup lang="ts">
import type { Friend } from '~/api/types'

// Полка «Друзья сейчас» на главной (телефон и iPad): обложка того, что слушает друг, его аватар
// в углу, имя и трек. Играющие сейчас — первыми; нажатие на обложку — включить этот трек, на имя —
// страница друга. Пусто или друзей нет — полки нет
const friends = useFriendsStore()
const player = usePlayerStore()
const lang = useLang()
const { tg } = useGlobalT()

onMounted(() => friends.init())

const items = computed(() => friends.withActivity.slice(0, 12))

function when(f: Friend) {
  void friends.now
  const a = f.activity
  if (!a) return ''
  return a.now ? tg('friends.now') : notificationTime(a.at, new Date(), lang.value)
}

function play(f: Friend) {
  const tr = f.activity?.track
  if (!tr) return
  if (player.current?.track.id === tr.id) return player.toggle()
  return player.playTracks([tr], 0, { type: 'queue', id: null, title: f.person.name })
}
</script>

<template>
  <MusicShelf v-if="items.length" :title="tg('friends.activity')" to="/friends" :cols="6" phone-width="136px">
    <div v-for="f in items" :key="f.person.id" class="fs">
      <button type="button" class="fs__art" :aria-label="`${tg('common.play')}: ${f.activity!.track.title}`" @click="play(f)">
        <MusicCoverArt :image="f.activity!.track.album?.image" size="medium" :alt="''" radius="10px" />
        <span class="fs__who"><FriendsAvatar :person="f.person" :size="32" :live="f.activity!.now" /></span>
      </button>
      <NuxtLink :to="`/friends/${f.person.id}`" class="fs__name ellipsis">{{ f.person.name }}</NuxtLink>
      <span class="fs__track ellipsis"><span :class="{ 'fs__now': f.activity!.now }">{{ when(f) }}</span> · {{ f.activity!.track.title }}</span>
    </div>
  </MusicShelf>
</template>

<style scoped>
.fs { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.fs__art { position: relative; display: block; width: 100%; margin-bottom: 8px; padding: 0; border: 0; border-radius: 10px; background: none; }
.fs__art:focus-visible { outline: 2px solid var(--focus); outline-offset: 3px; }
.fs__who { position: absolute; left: 8px; bottom: 8px; display: inline-grid; border-radius: 50%; box-shadow: 0 0 0 2px rgba(4, 5, 7, 0.55); }
.fs__name { font-weight: 650; color: var(--text-1); }
.fs__track { font-size: var(--fs-small); color: var(--text-2); }
.fs__now { color: var(--accent); font-weight: 600; }
</style>
