<script setup lang="ts">
import type { Friend, PersonCard } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// Друзья: запросы, что слушают друзья (играющие — первыми), поиск людей сервиса по имени и ссылка
// «Добавить в друзья» с QR-кодом. На компьютере то же «что слушают» — и в правой панели
const bridge = useBridge()
const friends = useFriendsStore()
const ui = useUiStore()
const toast = useToast()
const pui = usePlaylistUi()
const { tg } = useGlobalT()
const { t } = useScopedI18n('pages.friends.index')
useHead({ title: () => tg('friends.title') })

onMounted(() => {
  friends.init()
  void friends.load()
  void searchPeople('')
})

// ---------- Поиск людей ----------

const q = ref('')
const people = ref<PersonCard[] | null>(null)
let seq = 0
async function searchPeople(text: string) {
  const my = ++seq
  try {
    const res = await bridge.friends.people(text.trim())
    if (my === seq) people.value = res.items
  }
  catch {
    if (my === seq) people.value = []
  }
}
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, (v) => {
  clearTimeout(timer)
  timer = setTimeout(() => void searchPeople(v), 200)
})
// Список перечитался (принял, отправил) — отметки в поиске тоже
watch(() => friends.list, () => void searchPeople(q.value))

const searching = computed(() => !!q.value.trim())
/** Без запроса — знакомые по сервису, кого ещё нет в друзьях */
const suggestions = computed(() => (people.value ?? []).filter(p => p.relation === 'none').slice(0, 8))

// ---------- Ссылка «Добавить в друзья» ----------

const inviteOpen = ref(false)
const inviteUrl = ref('')
const inviteBusy = ref(false)
async function openInvite(reset = false) {
  inviteOpen.value = true
  inviteBusy.value = true
  try {
    inviteUrl.value = (await bridge.friends.invite(reset)).url
  }
  catch {
    toast.error(tg('friends.failed'))
  }
  finally {
    inviteBusy.value = false
  }
}

// ---------- Меню друга ----------

async function unfriend(f: Friend) {
  const ok = await pui.confirm({ title: t('removeTitle', { name: f.person.name }), text: t('removeText'), action: t('remove'), danger: true })
  if (ok) await friends.remove(f.person)
}
function menuFor(f: Friend): MenuEntry[] {
  return [
    { kind: 'heading', label: f.person.name },
    { label: t('open'), icon: 'ph:user', onSelect: () => void ui.navigate(`/friends/${f.person.id}`) },
    ...(f.activity?.jamCode ? [{ label: tg('friends.listenTogether'), icon: 'ph:users-three', onSelect: () => void ui.navigate(`/jam/${f.activity!.jamCode}`) }] : []),
    { kind: 'separator' },
    { label: t('remove'), icon: 'ph:user-minus', danger: true, onSelect: () => void unfriend(f) },
  ]
}

// Телефон: название уехало под верхнюю строку — она показывает его сама
const titleEl = ref<HTMLElement | null>(null)
const condensed = ref(false)
let tio: IntersectionObserver | null = null
watch(titleEl, (el) => {
  tio?.disconnect()
  if (!el) return
  tio = new IntersectionObserver(([e]) => {
    if (e) condensed.value = !e.isIntersecting && e.boundingClientRect.top < 100
  }, { rootMargin: '-100px 0px 0px 0px' })
  tio.observe(el)
})
onBeforeUnmount(() => tio?.disconnect())
</script>

<template>
  <div class="fr">
    <MusicPhoneNav v-if="ui.isTouchUi" :title="tg('friends.title')" :condensed="condensed" fallback="/library">
      <UiIconButton icon="ph:link" :label="t('invite')" size="lg" :tooltip="false" @click="openInvite()" />
    </MusicPhoneNav>
    <header class="fr__head">
      <h1 ref="titleEl" class="fr__title">{{ tg('friends.title') }}</h1>
      <UiButton v-if="!ui.isTouchUi" size="sm" variant="secondary" icon="ph:link" @click="openInvite()">{{ t('invite') }}</UiButton>
    </header>

    <UiSearchField v-model="q" :placeholder="t('search')" class="fr__search" />

    <!-- Поиск: люди сервиса по имени -->
    <section v-if="searching" class="fr__sec">
      <FriendsPersonRow v-for="p in people ?? []" :key="p.person.id" :person="p.person" :relation="p.relation" class="fr__row" />
      <p v-if="people && !people.length" class="fr__empty">{{ t('noOne') }}</p>
    </section>

    <template v-else>
      <section v-if="friends.incoming.length" class="fr__sec">
        <h2 class="fr__h">{{ t('incoming') }}</h2>
        <FriendsPersonRow v-for="r in friends.incoming" :key="r.person.id" :person="r.person" relation="incoming" :hint="t('wants')" class="fr__row" />
      </section>

      <section v-if="friends.friends.length" class="fr__sec">
        <h2 class="fr__h">{{ friends.listening.length ? t('listeningNow', { n: friends.listening.length }) : tg('friends.title') }}</h2>
        <UiContextMenu v-for="f in friends.friends" :key="f.person.id" :items="menuFor(f)">
          <div class="fr__friend fr__row">
            <FriendsActivityRow :friend="f" variant="list" class="fr__activity" />
            <UiMenu :items="menuFor(f)" align="end">
              <UiIconButton icon="ph:dots-three" :label="tg('common.more')" :size="ui.isTouchUi ? 'lg' : 'sm'" :tooltip="false" class="fr__more" />
            </UiMenu>
          </div>
        </UiContextMenu>
      </section>

      <UiEmptyState v-else-if="friends.list" icon="ph:users-three" :title="tg('friends.empty')" :text="t('emptyText')">
        <UiButton variant="primary" icon="ph:link" @click="openInvite()">{{ t('invite') }}</UiButton>
      </UiEmptyState>
      <div v-else class="fr__skel" aria-hidden="true">
        <div v-for="n in 4" :key="n" class="fr__skel-row">
          <UiSkeleton variant="round" width="48px" height="48px" />
          <div class="fr__skel-lines">
            <UiSkeleton variant="line" width="30%" height="13px" />
            <UiSkeleton variant="line" width="60%" height="12px" />
          </div>
        </div>
      </div>

      <section v-if="suggestions.length" class="fr__sec">
        <h2 class="fr__h">{{ t('people') }}</h2>
        <FriendsPersonRow v-for="p in suggestions" :key="p.person.id" :person="p.person" :relation="p.relation" class="fr__row" />
      </section>

      <section v-if="friends.outgoing.length" class="fr__sec">
        <h2 class="fr__h">{{ t('outgoing') }}</h2>
        <FriendsPersonRow v-for="r in friends.outgoing" :key="r.person.id" :person="r.person" relation="outgoing" class="fr__row" />
      </section>

      <SettingsFriends class="fr__privacy" :title="t('privacy')" />
    </template>

    <UiModal v-model:open="inviteOpen" :title="t('invite')" :width="400">
      <div class="fr__invite">
        <FriendsInviteLink v-if="inviteUrl" :url="inviteUrl" :hint="t('inviteHint')" :share-title="t('shareTitle')" :busy="inviteBusy" @reset="openInvite(true)" />
        <UiSkeleton v-else width="208px" height="208px" />
      </div>
    </UiModal>
  </div>
</template>

<style scoped>
.fr { display: flex; flex-direction: column; gap: 20px; width: 100%; max-width: 720px; margin: 0 auto; padding-top: 16px; }
.fr__head { display: flex; align-items: center; justify-content: space-between; gap: 16px; min-height: 44px; }
.fr__title { margin: 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.fr__sec { display: flex; flex-direction: column; }
.fr__h { margin: 0 0 4px; font-size: var(--fs-h4, 17px); font-weight: 700; letter-spacing: var(--tr-title); }
.fr__row { position: relative; }
.fr__row + .fr__row::before { content: ''; position: absolute; top: 0; left: 62px; right: 0; height: 0.5px; background: var(--hair); }
.fr__friend { display: flex; align-items: center; gap: 4px; }
.fr__activity { flex: 1; min-width: 0; }
.fr__more { flex: none; color: var(--text-2); }
.fr__empty { margin: 0; padding: 16px 0; color: var(--text-3); }
.fr__privacy { margin-top: 12px; }
.fr__invite { display: grid; place-items: center; padding: 8px 4px 16px; }
.fr__skel { display: flex; flex-direction: column; gap: 20px; }
.fr__skel-row { display: flex; align-items: center; gap: 14px; }
.fr__skel-lines { display: flex; flex: 1; flex-direction: column; gap: 8px; }

@media (max-width: 699px) {
  .fr { gap: 16px; padding: 0 16px; }
  .fr > .pn { margin-right: -16px; margin-bottom: -8px; margin-left: -16px; }
  .fr__title { font-size: 28px; }
}
</style>
