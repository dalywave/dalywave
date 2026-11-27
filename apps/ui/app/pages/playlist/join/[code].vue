<script setup lang="ts">
import type { PlaylistInvite } from '~/api/types'

// Ссылка в совместный плейлист: чей плейлист и что в нём — и «Присоединиться». Вошёл — плейлист
// в медиатеке на всех устройствах, можно добавлять свои треки
const route = useRoute()
const bridge = useBridge()
const library = useLibraryStore()
const toast = useToast()
const ui = useUiStore()
const lang = useLang()
const { tg } = useGlobalT()
const { t } = useScopedI18n('pages.playlist.join._code')
useHead({ title: () => data.value?.playlist.title ?? t('title') })

const code = computed(() => String(route.params.code))
const { data, state, load } = useLoad<PlaylistInvite>(() => bridge.playlists.joinPreview(code.value), () => code.value)
const busy = ref(false)

async function join() {
  if (!data.value) return
  busy.value = true
  try {
    const p = await bridge.playlists.join(code.value)
    void library.loadPlaylists(true).catch(() => {})
    toast.show({ text: t('joined', { title: p.title }), icon: 'ph:users-three' })
    await ui.navigate(`/playlist/${p.id}`)
  }
  catch {
    toast.error(tg('friends.failed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="inv">
    <MusicPhoneNav fallback="/library" />
    <div v-if="state === 'ready' && data" class="inv__card">
      <div class="inv__cover">
        <MusicPlaylistCover :id="data.playlist.id" :title="data.playlist.title" :covers="data.playlist.covers" :image="data.playlist.image" :kind="data.playlist.kind" />
      </div>
      <div class="inv__owner">
        <FriendsAvatar :person="data.owner" :size="24" />
        <span>{{ t('from', { name: data.owner.name }) }}</span>
      </div>
      <h1 class="inv__name">{{ data.playlist.title }}</h1>
      <p class="inv__meta">{{ formatCount(data.playlist.tracksCount, 'track', lang) }}<template v-if="data.playlist.members?.length"> · {{ t('members', { n: data.playlist.members.length + 1 }) }}</template></p>
      <UiButton v-if="data.joined" variant="primary" size="lg" :to="`/playlist/${data.playlist.id}`">{{ t('open') }}</UiButton>
      <template v-else>
        <p class="inv__text">{{ t('text') }}</p>
        <UiButton variant="primary" size="lg" icon="ph:users-three" :loading="busy" @click="join">{{ t('join') }}</UiButton>
      </template>
    </div>
    <UiEmptyState v-else-if="state === 'missing'" icon="ph:link-break" :title="t('expired')" :text="t('expiredText')">
      <UiButton to="/library">{{ tg('nav.library') }}</UiButton>
    </UiEmptyState>
    <MusicPageState v-else-if="state !== 'ready'" :state="state" @retry="load" />
  </div>
</template>

<style scoped>
.inv { display: flex; flex-direction: column; min-height: 60vh; }
.inv__card { display: flex; flex-direction: column; align-items: center; gap: 10px; margin: auto; padding: 32px 24px; text-align: center; }
.inv__cover { width: min(220px, 56vw); margin-bottom: 8px; }
.inv__cover :deep(.pc) { width: 100%; border-radius: var(--r-4); box-shadow: var(--shadow-cover); }
.inv__owner { display: inline-flex; align-items: center; gap: 8px; font-size: var(--fs-small); font-weight: 600; color: var(--text-2); }
.inv__name { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); text-wrap: balance; }
.inv__meta { margin: 0; font-size: var(--fs-small); color: var(--text-3); }
.inv__text { max-width: 320px; margin: 4px 0 12px; color: var(--text-2); text-wrap: pretty; }
</style>
