<script setup lang="ts">
import type { UpcomingRelease } from '~/api/types'

// Анонсированный релиз: обложка, название, «EP · 12 декабря» и «Сохранить заранее». В день выхода
// сервер сам положит его в медиатеку и пришлёт уведомление
const props = defineProps<{
  release: UpcomingRelease
  /** Подпись — исполнитель (в медиатеке), а не вид релиза (на странице артиста) */
  showArtist?: boolean
}>()

const bridge = useBridge()
const toast = useToast()
const lang = useLang()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

const presaved = ref(props.release.presaved)
const busy = ref(false)
watch(() => props.release.presaved, v => (presaved.value = v))

const album = computed(() => props.release.album)
const day = computed(() => (album.value.releaseDate ? formatReleaseDay(album.value.releaseDate, lang.value) : ''))
const meta = computed(() => [props.showArtist ? album.value.artist.name : tg(`record.${album.value.recordType}`), day.value].filter(Boolean).join(' · '))

async function toggle() {
  if (busy.value) return
  busy.value = true
  const was = presaved.value
  presaved.value = !was
  bridge.haptic(was ? 'light' : 'success')
  try {
    if (was) {
      await bridge.library.unpresave(album.value.id)
    }
    else {
      const res = await bridge.library.presave(album.value.id)
      toast.show(res.savedNow
        ? { text: t('savedNow', { title: album.value.title }), icon: 'ph:check-circle-fill' }
        : { text: t('presaved', { day: day.value }), icon: 'ph:calendar-check' })
    }
  }
  catch {
    presaved.value = was
    toast.error(t('failed'))
  }
  finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="up">
    <div class="up__row">
      <NuxtLink :to="`/album/${album.id}`" class="up__link">
        <MusicCoverArt :image="album.image" size="small" :alt="album.title" class="up__art" fallback-icon="ph:calendar-blank" />
        <span class="up__text">
          <span class="up__title">{{ album.title }}</span>
          <span class="up__meta">{{ meta }}</span>
        </span>
      </NuxtLink>
      <UiButton
        size="sm"
        :variant="presaved ? 'ghost' : 'secondary'"
        :icon="presaved ? 'ph:check' : 'ph:calendar-plus'"
        :aria-pressed="presaved"
        :class="{ 'is-on': presaved }"
        :aria-label="presaved ? t('saved') : t('save')"
        @click="toggle"
      >
        {{ presaved ? t('saved') : t('save') }}
      </UiButton>
    </div>
  </div>
</template>

<style scoped>
.up { container-type: inline-size; }
.up__row { display: flex; align-items: center; gap: 12px; min-height: 64px; padding: 6px 0; }
.up + .up .up__row { box-shadow: inset 0 0.5px 0 var(--hair); }
.up__link { display: flex; align-items: center; gap: 12px; flex: 1; min-width: 0; color: inherit; text-decoration: none; }
.up__art { width: 52px; }
.up__text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.up__title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.up__meta { color: var(--text-2); font-size: var(--fs-small); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.is-on { color: var(--accent); }
@media (hover: hover) {
  .up__link:hover .up__title { text-decoration: underline; text-underline-offset: 3px; }
}
/* Узко (телефон): кнопка — значком, дата видна целиком */
@container (max-width: 480px) {
  .up :deep(.btn__label) { display: none; }
  .up :deep(.btn) { min-width: 44px; min-height: 44px; padding: 0; justify-content: center; }
}
@media (pointer: coarse) {
  .up :deep(.btn) { min-height: 44px; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "save": "Сохранить заранее",
    "saved": "Сохранено",
    "presaved": "Сохраним в медиатеку {day}",
    "savedNow": "«{title}» — в медиатеке",
    "failed": "Не получилось — попробуйте ещё раз"
  },
  "uk": {
    "save": "Зберегти заздалегідь",
    "saved": "Збережено",
    "presaved": "Збережемо в медіатеку {day}",
    "savedNow": "«{title}» — у медіатеці",
    "failed": "Не вдалося — спробуйте ще раз"
  },
  "en": {
    "save": "Pre-save",
    "saved": "Pre-saved",
    "presaved": "Will be added to your library on {day}",
    "savedNow": "“{title}” is in your library",
    "failed": "Something went wrong — try again"
  }
}
</i18n>
