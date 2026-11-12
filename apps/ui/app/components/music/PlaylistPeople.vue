<script setup lang="ts">
import type { PersonRef, PlaylistSummary } from '~/api/types'

// «Совместный плейлист»: владелец включает совместный, зовёт друзей (или даёт ссылку с QR-кодом),
// убирает участников и решает, видят ли плейлист друзья. Участник видит, кто ещё в плейлисте, и может
// выйти. Одно окно на приложение — из меню плейлиста и из шапки (usePlaylistUi().openPeople)
const pui = usePlaylistUi()
const bridge = useBridge()
const auth = useAuthStore()
const friends = useFriendsStore()
const library = useLibraryStore()
const toast = useToast()
const ui = useUiStore()
const route = useRoute()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

const pl = ref<PlaylistSummary | null>(null)
const busy = ref<string | null>(null)
const link = ref('')
const linkBusy = ref(false)

const owner = computed(() => pl.value?.access === 'owner')
const me = computed<PersonRef | null>(() => (auth.user ? { id: auth.user.id, name: auth.user.name, avatarUrl: auth.user.avatarUrl ?? null } : null))
const members = computed(() => pl.value?.members ?? [])
/** Владелец, затем участники; «вы» — подписью */
const people = computed(() => {
  const own = owner.value ? me.value : pl.value?.owner ?? null
  return [...(own ? [{ person: own, owner: true }] : []), ...members.value.map(person => ({ person, owner: false }))]
})
/** Друзья, которых ещё нет в плейлисте */
const invitable = computed(() => friends.friends.map(f => f.person).filter(p => !members.value.some(m => m.id === p.id)))

watch(() => pui.peopleOpen, async (open) => {
  if (!open || !pui.peopleFor) return
  pl.value = pui.peopleFor
  link.value = ''
  friends.init()
  void friends.load()
  try {
    pl.value = (await bridge.playlists.get(pui.peopleFor.id)).playlist
  }
  catch {
    // Покажем то, что было в меню
  }
  if (owner.value && pl.value?.collaborative) void loadLink()
})

function changed(next: PlaylistSummary | null) {
  if (next) pl.value = { ...pl.value!, ...next }
  void library.loadPlaylists(true).catch(() => {})
  pui.peopleChanged++
}

async function run(key: string, fn: () => Promise<void>) {
  busy.value = key
  try {
    await fn()
  }
  catch {
    toast.error(tg('friends.failed'))
  }
  finally {
    busy.value = null
  }
}

async function loadLink(reset = false) {
  if (!pl.value) return
  linkBusy.value = true
  try {
    link.value = (await bridge.playlists.invite(pl.value.id, reset)).url
    if (!pl.value.collaborative) changed({ ...pl.value, collaborative: true })
  }
  catch {
    toast.error(tg('friends.failed'))
  }
  finally {
    linkBusy.value = false
  }
}

function setCollaborative(on: boolean) {
  const p = pl.value
  if (!p) return
  return run('collab', async () => {
    changed(await bridge.playlists.setCollaborative(p.id, on))
    if (on) await loadLink()
    else link.value = ''
  })
}

function setVisible(on: boolean) {
  const p = pl.value
  if (!p) return
  return run('visible', async () => changed(await bridge.playlists.setVisibility(p.id, on)))
}

function invite(person: PersonRef) {
  const p = pl.value
  if (!p) return
  return run(person.id, async () => {
    changed(await bridge.playlists.addMembers(p.id, [person.id]))
    if (!link.value) void loadLink()
    toast.show({ text: t('invited', { name: person.name }), icon: 'ph:user-plus' })
  })
}

function removeMember(person: PersonRef) {
  const p = pl.value
  if (!p) return
  return run(person.id, async () => {
    await bridge.playlists.removeMember(p.id, person.id)
    changed({ ...p, members: (p.members ?? []).filter(m => m.id !== person.id) })
  })
}

async function leave() {
  const p = pl.value
  const self = me.value
  if (!p || !self) return
  await run('leave', async () => {
    await bridge.playlists.removeMember(p.id, self.id)
    pui.peopleOpen = false
    changed(null)
    toast.show({ text: t('left', { title: p.title }), icon: 'ph:sign-out' })
    if (route.path === `/playlist/${p.id}`) await ui.navigate('/library')
  })
}
</script>

<template>
  <UiModal v-model:open="pui.peopleOpen" :title="t('title')" :description="pl?.title" :width="440" detents="expand">
    <div v-if="pl" class="pp">
      <!-- Владелец: совместный ли и видят ли друзья -->
      <SettingsGroup v-if="owner" class="pp__group">
        <SettingsRow :title="t('collab')" :description="t('collabDesc')">
          <UiSwitch :model-value="pl.collaborative" :label="t('collab')" hide-label :disabled="busy === 'collab'" @update:model-value="setCollaborative" />
        </SettingsRow>
        <SettingsRow :title="t('visible')" :description="t('visibleDesc')">
          <UiSwitch :model-value="pl.friendsCanSee" :label="t('visible')" hide-label :disabled="busy === 'visible'" @update:model-value="setVisible" />
        </SettingsRow>
      </SettingsGroup>

      <section v-if="pl.collaborative || !owner" class="pp__sec">
        <h3 class="pp__h">{{ t('people') }}</h3>
        <div v-for="x in people" :key="x.person.id" class="pp__row">
          <FriendsAvatar :person="x.person" :size="40" />
          <span class="pp__text">
            <span class="pp__name ellipsis">{{ x.person.id === me?.id ? t('you') : x.person.name }}</span>
            <span v-if="x.owner" class="pp__hint">{{ t('owner') }}</span>
          </span>
          <UiIconButton v-if="owner && !x.owner" icon="ph:user-minus" :label="t('remove', { name: x.person.name })" :size="ui.isTouchUi ? 'lg' : 'sm'" :tooltip="ui.isTouchUi ? false : undefined" :disabled="busy === x.person.id" @click="removeMember(x.person)" />
        </div>
      </section>

      <template v-if="owner">
        <section class="pp__sec">
          <h3 class="pp__h">{{ t('invite') }}</h3>
          <div v-for="f in invitable" :key="f.id" class="pp__row">
            <FriendsAvatar :person="f" :size="40" />
            <span class="pp__text"><span class="pp__name ellipsis">{{ f.name }}</span></span>
            <UiButton size="sm" variant="secondary" icon="ph:plus" :loading="busy === f.id" @click="invite(f)">{{ t('add') }}</UiButton>
          </div>
          <p v-if="friends.list && !invitable.length" class="pp__empty">
            {{ friends.friends.length ? t('allIn') : t('noFriends') }}
            <NuxtLink v-if="!friends.friends.length" to="/friends" class="pp__link" @click="pui.peopleOpen = false">{{ tg('friends.title') }}</NuxtLink>
          </p>
        </section>

        <section v-if="pl.collaborative" class="pp__sec pp__sec--link">
          <h3 class="pp__h">{{ t('byLink') }}</h3>
          <FriendsInviteLink v-if="link" :url="link" :hint="t('linkHint')" :share-title="pl.title" :busy="linkBusy" @reset="loadLink(true)" />
          <UiSkeleton v-else width="208px" height="208px" class="pp__skel" />
        </section>
      </template>

      <UiButton v-else-if="pl.access === 'member'" variant="danger" block icon="ph:sign-out" :loading="busy === 'leave'" class="pp__leave" @click="leave">{{ t('leave') }}</UiButton>
    </div>
  </UiModal>
</template>

<style scoped>
.pp { display: flex; flex-direction: column; gap: 24px; padding: 0 20px 20px; }
.pp__group { margin: 0; }
.pp__sec { display: flex; flex-direction: column; }
.pp__sec--link { align-items: stretch; }
.pp__h { margin: 0 0 4px; font-size: var(--fs-small); font-weight: 650; color: var(--text-2); }
.pp__row { position: relative; display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 6px 0; }
.pp__row + .pp__row::before { content: ''; position: absolute; top: 0; left: 52px; right: 0; height: 0.5px; background: var(--hair); }
.pp__text { display: flex; flex: 1; flex-direction: column; gap: 1px; min-width: 0; }
.pp__name { font-weight: 600; }
.pp__hint { font-size: var(--fs-caption); color: var(--text-3); }
.pp__empty { margin: 0; padding: 8px 0; font-size: var(--fs-small); color: var(--text-3); }
.pp__link { margin-left: 4px; color: var(--text-1); text-decoration: underline; text-underline-offset: 3px; }
.pp__skel { align-self: center; }
.pp__leave { margin-top: 4px; }
@media (pointer: coarse) {
  .pp__row { min-height: 60px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Совместный плейлист",
    "collab": "Совместный",
    "collabDesc": "Друзья добавляют, убирают и переставляют треки",
    "visible": "Видно друзьям",
    "visibleDesc": "На вашей странице у друзей",
    "people": "Участники",
    "you": "Вы",
    "owner": "владелец",
    "remove": "Убрать {name}",
    "invite": "Позвать друзей",
    "add": "Позвать",
    "invited": "{name} теперь в плейлисте",
    "allIn": "Все друзья уже здесь",
    "noFriends": "Чтобы звать в плейлист, добавьте друзей:",
    "byLink": "По ссылке",
    "linkHint": "Кто откроет ссылку, сможет добавлять треки",
    "leave": "Выйти из плейлиста",
    "left": "Вы вышли из «{title}»"
  },
  "en": {
    "title": "Shared playlist",
    "collab": "Shared",
    "collabDesc": "Friends add, remove and reorder tracks",
    "visible": "Visible to friends",
    "visibleDesc": "On your page for friends",
    "people": "People",
    "you": "You",
    "owner": "owner",
    "remove": "Remove {name}",
    "invite": "Invite friends",
    "add": "Invite",
    "invited": "{name} is in the playlist now",
    "allIn": "All your friends are already here",
    "noFriends": "To invite people, add friends first:",
    "byLink": "With a link",
    "linkHint": "Anyone who opens the link can add tracks",
    "leave": "Leave playlist",
    "left": "You left “{title}”"
  }
}
</i18n>
