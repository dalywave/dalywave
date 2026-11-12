<script setup lang="ts">
// Короткий ввод названия (новая папка, переименовать). Одно на приложение: вызывается через
// usePlaylistUi().prompt(...) и отвечает промисом: строка или null
const pui = usePlaylistUi()
const { tg } = useGlobalT()
const value = ref('')

watch(() => pui.promptOpen, (open) => {
  if (open) value.value = pui.promptReq?.value ?? ''
})

const clean = computed(() => value.value.trim().replace(/\s+/g, ' '))

function submit() {
  if (!clean.value) return
  pui.submitPrompt(clean.value)
}
</script>

<template>
  <UiModal v-model:open="pui.promptOpen" :title="pui.promptReq?.title ?? ''" :width="380">
    <form class="pr" @submit.prevent="submit">
      <input
        v-model="value"
        class="pr__input"
        :placeholder="pui.promptReq?.placeholder"
        :aria-label="pui.promptReq?.title"
        :maxlength="pui.promptReq?.maxLength ?? 100"
        enterkeyhint="done"
        autocomplete="off"
        autofocus
      >
      <div class="pr__actions">
        <UiButton size="lg" block @click="pui.submitPrompt(null)">{{ tg('common.cancel') }}</UiButton>
        <UiButton type="submit" size="lg" block variant="primary" :disabled="!clean">{{ pui.promptReq?.action }}</UiButton>
      </div>
    </form>
  </UiModal>
</template>

<style scoped>
.pr { display: flex; flex-direction: column; gap: 16px; padding: 0 20px 20px; }
.pr__input { height: 44px; padding: 0 16px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.pr__input::placeholder { color: var(--text-4); }
.pr__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.pr__actions { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
@media (max-width: 699px) {
  .pr { padding-bottom: 12px; }
  .pr__input { height: 48px; font-size: 16px; }
  .pr__actions { grid-template-columns: 1fr; }
  .pr__actions > :last-child { order: -1; }
}
</style>
