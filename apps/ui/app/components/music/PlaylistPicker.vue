<script setup lang="ts">
import type { PlaylistSummary } from '~/api/types'

// «Добавить в плейлист»: поиск по своим плейлистам, «Новый плейлист» и список. Одно окно на
// приложение — открывается из меню трека, выделения и очереди (usePlaylistUi().pick)
const pui = usePlaylistUi()
const library = useLibraryStore()
const actions = usePlaylistActions()
const { t } = useI18n()

const q = ref('')
const creating = ref(false)
const newTitle = ref('')
const busy = ref<string | null>(null)

watch(() => pui.pickOpen, (open) => {
  if (!open) return
  q.value = ''
  creating.value = false
  newTitle.value = ''
  void library.loadPlaylists().catch(() => {})
})

const tracks = computed(() => pui.pickTracks ?? [])
const heading = computed(() => (tracks.value.length === 1 ? tracks.value[0]!.title : t('pl.selected', { n: tracks.value.length })))
const list = computed(() => {
  const needle = q.value.trim().toLocaleLowerCase()
  const own = actions.own.value
  return needle ? own.filter(p => p.title.toLocaleLowerCase().includes(needle)) : own
})

async function choose(p: PlaylistSummary) {
  if (busy.value) return
  busy.value = p.id
  await actions.addTo(p, tracks.value)
  busy.value = null
  pui.pickOpen = false
}

function startCreate() {
  creating.value = true
  newTitle.value = q.value.trim()
}

async function create() {
  const title = newTitle.value.trim()
  if (!title || busy.value) return
  busy.value = 'new'
  try {
    await actions.createWith(title, tracks.value, { open: false })
    pui.pickOpen = false
  }
  catch {
    useToast().error(t('pl.failed'))
  }
  finally {
    busy.value = null
  }
}
</script>

<template>
  <UiModal v-model:open="pui.pickOpen" :title="t('pl.add')" :description="heading" detents="expand" tall :width="400">
    <div class="pk">
      <p class="pk__what ellipsis">{{ heading }}</p>
      <UiSearchField v-if="actions.own.value.length > 5" v-model="q" :placeholder="t('pl.pickSearch')" class="pk__search" />
      <form v-if="creating" class="pk__new" @submit.prevent="create">
        <input v-model="newTitle" class="pk__input" :placeholder="t('pl.newName')" :aria-label="t('pl.new')" maxlength="200" enterkeyhint="done" autofocus>
        <UiButton type="submit" variant="primary" :loading="busy === 'new'" :disabled="!newTitle.trim()">{{ t('pl.create') }}</UiButton>
      </form>
      <button v-else type="button" class="pk__row" @click="startCreate">
        <span class="pk__art pk__art--new"><Icon name="ph:plus" /></span>
        <span class="pk__name">{{ t('pl.new') }}</span>
      </button>
      <div class="pk__hair" />
      <button v-for="p in list" :key="p.id" type="button" class="pk__row" :disabled="!!busy" @click="choose(p)">
        <MusicPlaylistCover :id="p.id" :title="p.title" :covers="p.covers" :image="p.image" :kind="p.kind" :show-title="false" class="pk__art" />
        <span class="pk__text">
          <span class="pk__name ellipsis">{{ p.title }}</span>
          <span class="pk__sub ellipsis">{{ actions.subOf(p) }}</span>
        </span>
        <span v-if="busy === p.id" class="pk__spin" aria-hidden="true" />
      </button>
      <p v-if="!list.length" class="pk__none">{{ q.trim() ? t('pl.noMatch') : t('pl.noPlaylists') }}</p>
    </div>
  </UiModal>
</template>

<style scoped>
.pk { display: flex; flex-direction: column; padding: 0 12px 16px; }
.pk__what { margin: 0 8px 12px; font-size: var(--fs-small); color: var(--text-3); }
.pk__search { margin: 0 8px 8px; }
.pk__row { display: flex; align-items: center; gap: 12px; min-height: 60px; padding: 8px; border: 0; border-radius: var(--r-3); background: none; color: var(--text-1); font: inherit; text-align: left; cursor: pointer; transition: background-color var(--dur-1); -webkit-tap-highlight-color: transparent; }
.pk__row:hover, .pk__row:focus-visible { background: var(--fill-1); outline: none; }
.pk__row:active { background: var(--fill-2); }
.pk__row:disabled { cursor: default; }
.pk__art { width: 44px; height: 44px; flex: none; border-radius: var(--r-2); }
.pk__art--new { display: grid; place-items: center; background: var(--fill-2); color: var(--text-1); font-size: 20px; }
.pk__text { display: flex; flex: 1; flex-direction: column; gap: 2px; min-width: 0; }
.pk__name { font-weight: 550; }
.pk__sub { font-size: var(--fs-small); color: var(--text-3); }
.pk__hair { height: 1px; margin: 6px 8px; background: var(--hair); }
.pk__new { display: flex; gap: 8px; padding: 8px; }
.pk__input { flex: 1; min-width: 0; height: 40px; padding: 0 14px; border: 0; border-radius: var(--r-pill); background: var(--fill-1); box-shadow: inset 0 0 0 1px var(--line); color: var(--text-1); font: inherit; }
.pk__input::placeholder { color: var(--text-4); }
.pk__input:focus-visible { outline: none; box-shadow: inset 0 0 0 1.5px var(--focus); }
.pk__none { margin: 16px 8px; font-size: var(--fs-small); color: var(--text-3); text-align: center; }
.pk__spin { width: 18px; height: 18px; flex: none; border: 2px solid var(--fill-3); border-top-color: var(--text-1); border-radius: 50%; animation: spin 0.8s linear infinite; }
@media (max-width: 699px) {
  .pk { padding: 0 8px 12px; }
  .pk__input { height: 44px; font-size: 16px; }
}
</style>
