<script setup lang="ts">
import type { PersonCard } from '~/api/types'

// Ссылка «Добавить в друзья»: кто зовёт — и одно действие. Открыл — стал другом (ссылку дал сам человек)
const route = useRoute()
const bridge = useBridge()
const friends = useFriendsStore()
const toast = useToast()
const ui = useUiStore()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })
useHead({ title: () => tg('friends.add') })

const code = computed(() => String(route.params.code))
const { data, state, load } = useLoad<PersonCard>(() => bridge.friends.invitePreview(code.value), () => code.value)
const busy = ref(false)

async function accept() {
  if (!data.value) return
  busy.value = true
  try {
    data.value = await bridge.friends.useInvite(code.value)
    void friends.load()
    toast.show({ text: tg('friends.nowFriends', { name: data.value.person.name }), icon: 'ph:users-three' })
    await ui.navigate(`/friends/${data.value.person.id}`)
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
    <MusicPhoneNav fallback="/friends" />
    <div v-if="state === 'ready' && data" class="inv__card">
      <FriendsAvatar :person="data.person" :size="112" />
      <h1 class="inv__name">{{ data.person.name }}</h1>
      <template v-if="data.relation === 'friend'">
        <p class="inv__text">{{ t('already') }}</p>
        <UiButton variant="primary" size="lg" :to="`/friends/${data.person.id}`">{{ t('open') }}</UiButton>
      </template>
      <template v-else-if="data.relation === 'me'">
        <p class="inv__text">{{ t('mine') }}</p>
        <UiButton size="lg" to="/friends">{{ tg('friends.title') }}</UiButton>
      </template>
      <template v-else>
        <p class="inv__text">{{ t('invites') }}</p>
        <UiButton variant="primary" size="lg" icon="ph:user-plus" :loading="busy" @click="accept">{{ tg('friends.add') }}</UiButton>
      </template>
    </div>
    <UiEmptyState v-else-if="state === 'missing'" icon="ph:link-break" :title="t('expired')" :text="t('expiredText')">
      <UiButton to="/friends">{{ tg('friends.title') }}</UiButton>
    </UiEmptyState>
    <MusicPageState v-else-if="state !== 'ready'" :state="state" @retry="load" />
  </div>
</template>

<style scoped>
.inv { display: flex; flex-direction: column; min-height: 60vh; }
.inv__card { display: flex; flex-direction: column; align-items: center; gap: 12px; margin: auto; padding: 32px 24px; text-align: center; }
.inv__name { margin: 8px 0 0; font-size: var(--fs-h2); font-weight: 750; letter-spacing: var(--tr-title); }
.inv__text { max-width: 320px; margin: 0 0 12px; color: var(--text-2); text-wrap: pretty; }
</style>

<i18n lang="json">
{
  "ru": {
    "invites": "Зовёт вас в друзья: будет видно, что слушает каждый, и можно слушать вместе",
    "already": "Вы уже друзья",
    "open": "Открыть страницу",
    "mine": "Это ваша ссылка — отправьте её друзьям",
    "expired": "Ссылка больше не работает",
    "expiredText": "Попросите у друга новую"
  },
  "uk": {
    "invites": "Кличе вас у друзі: буде видно, що слухає кожен, і можна слухати разом",
    "already": "Ви вже друзі",
    "open": "Відкрити сторінку",
    "mine": "Це ваше посилання — надішліть його друзям",
    "expired": "Посилання більше не працює",
    "expiredText": "Попросіть у друга нове"
  },
  "en": {
    "invites": "Invites you to be friends: you’ll see what each other plays and can listen together",
    "already": "You’re already friends",
    "open": "Open page",
    "mine": "This is your link — send it to friends",
    "expired": "This link no longer works",
    "expiredText": "Ask your friend for a new one"
  }
}
</i18n>
