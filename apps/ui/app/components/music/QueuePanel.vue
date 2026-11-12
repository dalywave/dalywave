<script setup lang="ts">
import type { MenuEntry } from '~/types/ui'

// Очередь: что играет, что добавлено вручную (можно перетаскивать) и что дальше по волне или списку.
// Строки без значков загрузки: в очереди важно только, что сыграет дальше
const player = usePlayerStore()
const library = useLibraryStore()
const bridge = useBridge()
const toast = useToast()
const { menuFor } = useTrackActions()
const lang = useLang()
const { t } = useI18n()

const MAX_ROWS = 100
const contextTitle = computed(() => {
  const c = player.context
  if (!c) return ''
  if (c.type === 'wave') return player.wave.mood === 'any' ? t('wave.title') : `${t('wave.title')} · ${t(`wave.mood.${player.wave.mood}`)}`
  return c.title
})
const upcomingShown = computed(() => player.upcoming.slice(0, MAX_ROWS))
const hiddenCount = computed(() => Math.max(0, player.upcoming.length - MAX_ROWS))
const isEmpty = computed(() => !player.current && !player.manual.length && !player.upcoming.length)

// Волна не сохраняется сама — то, что в очереди сейчас, можно забрать в новый плейлист
const saving = ref(false)
async function saveAsPlaylist() {
  if (saving.value) return
  const ids = [player.current, ...player.manual, ...player.upcoming].flatMap(e => (e ? [e.track.id] : []))
  if (!ids.length) return
  saving.value = true
  const title = `${contextTitle.value || t('wave.title')} — ${new Intl.DateTimeFormat(lang.value, { day: 'numeric', month: 'long' }).format(new Date())}`
  try {
    const p = await bridge.playlists.create({ title, trackIds: [...new Set(ids)] })
    void library.loadPlaylists(true).catch(() => {})
    toast.show({ text: t('queue.saved', { title }), icon: 'ph:playlist', action: { label: t('queue.open'), onClick: () => void navigateTo(`/playlist/${p.id}`) } })
  }
  catch {
    toast.error(t('queue.saveFailed'))
  }
  finally {
    saving.value = false
  }
}

// Правый клик по строке — то же меню, что у «⋯»
const menuItems = ref<MenuEntry[]>([])
function onContext(target: EventTarget | null) {
  const uid = (target as HTMLElement | null)?.closest<HTMLElement>('[data-uid]')?.dataset.uid
  if (uid === 'current' && player.current) {
    menuItems.value = menuFor(player.current.track, { nowPlaying: true })
    return
  }
  const e = [...player.manual, ...player.upcoming].find(x => x.uid === uid)
  menuItems.value = e ? menuFor(e.track, { inQueue: e.uid }) : []
}

// Перетаскивание в «добавлено вручную»
const dragFrom = ref<number | null>(null)
const dropAt = ref<number | null>(null)

function onDragStart(i: number, e: DragEvent) {
  dragFrom.value = i
  e.dataTransfer?.setData('text/plain', String(i))
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move'
}

function onDrop(i: number) {
  if (dragFrom.value != null && dragFrom.value !== i) player.moveManual(dragFrom.value, i)
  dragFrom.value = null
  dropAt.value = null
}
</script>

<template>
  <UiContextMenu :items="menuItems" @open="onContext">
    <div class="queue">
      <UiEmptyState v-if="isEmpty" icon="ph:queue" :title="t('queue.empty')" :text="t('queue.emptyHint')" compact>
        <UiButton variant="primary" icon="ph:play-fill" @click="player.startWave()">{{ t('wave.listen') }}</UiButton>
      </UiEmptyState>
      <template v-else>
        <section v-if="player.current" class="qsec">
          <header class="qsec__head">
            <h3 class="qsec__title">{{ t('queue.now') }}</h3>
            <span v-if="contextTitle" class="qsec__ctx ellipsis"><i class="qsec__dot" :class="{ 'qsec__dot--any': player.waveActive && player.wave.mood === 'any' }" />{{ contextTitle }}</span>
          </header>
          <MusicTrackRow :track="player.current.track" variant="queue" :reason="player.current.reason" plain data-uid="current" @play="player.toggle()" />
        </section>

        <section v-if="player.manual.length" class="qsec">
          <header class="qsec__head">
            <h3 class="qsec__title">{{ t('queue.manual') }}</h3>
            <button type="button" class="qsec__action" @click="player.clearManual()">{{ t('queue.clear') }}</button>
          </header>
          <div
            v-for="(e, i) in player.manual"
            :key="e.uid"
            class="qsec__drag"
            :data-uid="e.uid"
            :class="{ 'is-drop': dropAt === i && dragFrom !== i, 'is-dragging': dragFrom === i }"
            draggable="true"
            @dragstart="onDragStart(i, $event)"
            @dragover.prevent="dropAt = i"
            @dragleave="dropAt = dropAt === i ? null : dropAt"
            @drop.prevent="onDrop(i)"
            @dragend="dragFrom = null; dropAt = null"
          >
            <MusicTrackRow :track="e.track" variant="queue" :queue-uid="e.uid" plain @play="player.jumpTo(e.uid)" />
          </div>
        </section>

        <section v-if="player.upcoming.length || player.waveLoading" class="qsec">
          <header class="qsec__head">
            <h3 class="qsec__title">{{ player.waveActive ? t('queue.fromWave') : t('queue.fromContext') }}</h3>
            <button v-if="player.waveActive && player.upcoming.length" type="button" class="qsec__action" :disabled="saving" @click="saveAsPlaylist">
              <Icon name="ph:plus" />{{ t('queue.saveAsPlaylist') }}
            </button>
          </header>
          <MusicTrackRow v-for="e in upcomingShown" :key="e.uid" :track="e.track" variant="queue" :reason="e.reason" :queue-uid="e.uid" :data-uid="e.uid" plain @play="player.jumpTo(e.uid)" />
          <div v-if="player.waveLoading" class="qsec__loading">
            <div v-for="n in 2" :key="n" class="qsec__skel"><UiSkeleton width="40px" height="40px" /><div class="qsec__skel-lines"><UiSkeleton variant="line" width="60%" /><UiSkeleton variant="line" width="38%" /></div></div>
          </div>
          <p v-if="hiddenCount" class="qsec__more">{{ t('queue.andMore', { n: hiddenCount }) }}</p>
        </section>
      </template>
    </div>
  </UiContextMenu>
</template>

<style scoped>
.queue { display: flex; flex-direction: column; gap: 16px; }
.qsec__head { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; padding: 12px 8px 6px; min-width: 0; }
.qsec__title { margin: 0; font-size: var(--fs-small); font-weight: 650; white-space: nowrap; }
.qsec__ctx { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-caption); color: var(--accent); }
.qsec__dot { width: 7px; height: 7px; flex: none; border-radius: 50%; background: var(--accent); }
.qsec__dot--any { background: var(--brand-gradient); }
.qsec__action { display: inline-flex; align-items: center; gap: 4px; flex: none; padding: 0; border: 0; background: none; font-size: var(--fs-caption); font-weight: 600; color: var(--text-2); white-space: nowrap; }
.qsec__action:hover { color: var(--text-1); }
.qsec__action:disabled { opacity: 0.5; }
.qsec__action:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; border-radius: 4px; }
.qsec__drag { position: relative; border-radius: var(--r-3); }
.qsec__drag.is-dragging { opacity: 0.4; }
.qsec__drag.is-drop::before { content: ''; position: absolute; left: 8px; right: 8px; top: -1px; height: 2px; border-radius: 2px; background: var(--accent); }
.qsec__loading { display: flex; flex-direction: column; }
.qsec__skel { display: flex; align-items: center; gap: 12px; padding: 8px; }
.qsec__skel-lines { display: flex; flex: 1; flex-direction: column; gap: 8px; }
.qsec__more { margin: 8px; font-size: var(--fs-caption); color: var(--text-3); }
</style>
