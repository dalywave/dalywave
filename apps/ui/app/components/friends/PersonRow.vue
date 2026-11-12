<script setup lang="ts">
import type { PersonRef, Relation } from '~/api/types'

// Человек в поиске и в запросах: аватар, имя и одно действие по тому, кем приходится —
// «Добавить», «Принять / Отклонить», «Запрос отправлен» (нажатие — отменить), «В друзьях»
const props = defineProps<{ person: PersonRef, relation: Relation, hint?: string }>()

const friends = useFriendsStore()
const ui = useUiStore()
const { tg } = useGlobalT()
const { t } = useI18n({ useScope: 'local' })

const busy = computed(() => friends.busy.has(props.person.id))
const to = computed(() => (props.relation === 'friend' ? `/friends/${props.person.id}` : undefined))
</script>

<template>
  <div class="pr">
    <NuxtLink v-if="to" :to="to" class="pr__who">
      <FriendsAvatar :person="person" :size="44" />
      <span class="pr__text">
        <span class="pr__name ellipsis">{{ person.name }}</span>
        <span v-if="hint" class="pr__hint ellipsis">{{ hint }}</span>
      </span>
    </NuxtLink>
    <span v-else class="pr__who">
      <FriendsAvatar :person="person" :size="44" />
      <span class="pr__text">
        <span class="pr__name ellipsis">{{ person.name }}</span>
        <span v-if="hint" class="pr__hint ellipsis">{{ hint }}</span>
      </span>
    </span>
    <span class="pr__actions">
      <template v-if="relation === 'incoming'">
        <!-- Под пальцем — крестик: имени нужнее место -->
        <UiIconButton v-if="ui.isTouchUi" icon="ph:x" :label="t('decline')" size="lg" :tooltip="false" :disabled="busy" class="pr__decline" @click="friends.decline(person)" />
        <UiButton v-else size="sm" variant="secondary" :disabled="busy" @click="friends.decline(person)">{{ t('decline') }}</UiButton>
        <UiButton size="sm" variant="primary" :loading="busy" @click="friends.accept(person)">{{ t('accept') }}</UiButton>
      </template>
      <UiButton v-else-if="relation === 'none'" size="sm" variant="primary" icon="ph:user-plus" :loading="busy" @click="friends.request(person)">{{ t('add') }}</UiButton>
      <UiButton v-else-if="relation === 'outgoing'" size="sm" variant="ghost" :loading="busy" :title="t('cancel')" @click="friends.cancel(person)">{{ t('sent') }}</UiButton>
      <span v-else-if="relation === 'friend'" class="pr__state"><Icon name="ph:check" />{{ tg('friends.title') }}</span>
    </span>
  </div>
</template>

<style scoped>
.pr { display: flex; align-items: center; gap: 12px; min-height: 64px; padding: 10px 0; }
.pr__who { display: flex; flex: 1; align-items: center; gap: 14px; min-width: 0; border-radius: var(--r-2); }
a.pr__who:hover .pr__name { text-decoration: underline; text-underline-offset: 3px; text-decoration-color: var(--text-3); }
.pr__who:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.pr__text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.pr__name { font-weight: 650; }
.pr__hint { font-size: var(--fs-small); color: var(--text-3); }
.pr__actions { display: flex; flex: none; align-items: center; gap: 8px; }
.pr__decline { color: var(--text-2); }
.pr__state { display: inline-flex; align-items: center; gap: 4px; font-size: var(--fs-small); color: var(--text-3); }
@media (pointer: coarse) {
  .pr__actions :deep(.btn) { min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "add": "Добавить",
    "accept": "Принять",
    "decline": "Отклонить",
    "sent": "Запрос отправлен",
    "cancel": "Отменить запрос"
  },
  "en": {
    "add": "Add",
    "accept": "Accept",
    "decline": "Decline",
    "sent": "Request sent",
    "cancel": "Cancel request"
  }
}
</i18n>
