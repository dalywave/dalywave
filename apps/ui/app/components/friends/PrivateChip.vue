<script setup lang="ts">
// Идёт приватная сессия — видно в плеере: значок (панель плеера на компьютере) или строка «Приватная
// сессия · до 18:40» (полный плеер). Нажатие — выключить
withDefaults(defineProps<{ variant?: 'icon' | 'line' }>(), { variant: 'line' })

const priv = usePrivateSession()
const friends = useFriendsStore()
const { tg } = useGlobalT()

const label = computed(() => [tg('friends.private'), priv.until.value].filter(Boolean).join(' · '))
const items = computed(() => [
  { kind: 'heading' as const, label: label.value, hint: tg('friends.privateHint') },
  { label: tg('friends.privateStop'), icon: 'ph:eye', onSelect: () => void friends.setPrivate('off') },
])
</script>

<template>
  <UiMenu v-if="priv.on.value" :items="items" side="top" align="center">
    <UiIconButton v-if="variant === 'icon'" icon="ph:eye-slash" :label="label" size="sm" active />
    <button v-else type="button" class="pc">
      <Icon name="ph:eye-slash" class="pc__icon" />
      <span class="ellipsis">{{ label }}</span>
    </button>
  </UiMenu>
</template>

<style scoped>
.pc { display: inline-flex; align-items: center; gap: 6px; max-width: 100%; min-height: 32px; padding: 4px 12px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); font: inherit; font-size: var(--fs-caption); font-weight: 600; color: var(--text-2); }
.pc:hover { background: var(--fill-2); color: var(--text-1); }
.pc:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.pc__icon { flex: none; font-size: 14px; }
@media (pointer: coarse) {
  .pc { min-height: 44px; padding: 4px 16px; }
}
</style>
