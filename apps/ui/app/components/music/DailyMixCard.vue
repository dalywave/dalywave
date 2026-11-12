<script setup lang="ts">
import type { PlaylistSummary } from '~/api/types'
import type { MenuEntry } from '~/types/ui'

// «Микс дня»: настоящие обложки грани вкуса под цветной вуалью, крупный номер микса и артисты под ним.
// «На повторе» и «Старые хиты на повторе» — та же карточка, вместо номера — своё название на полосе
const props = defineProps<{
  playlist: PlaylistSummary
  /** Номер микса с 1 — от него цвет вуали */
  index: number
}>()

const TINTS = ['#ff4f8b', '#8ed8f5', '#b6abff', '#ffb26b', '#7fdcae', '#ff8fd6']
const { tg: t } = useGlobalT()
const { t: tl } = useI18n({ useScope: 'local' })
const { playPlaylist, isPlayingContext, loading } = usePlayActions()
const tint = computed(() => TINTS[(props.index - 1) % TINTS.length]!)
const covers = computed(() => props.playlist.covers ?? [])
const sub = computed(() => props.playlist.description || props.playlist.title)
const special = computed(() => (props.playlist.generator === 'on_repeat' || props.playlist.generator === 'repeat_rewind' ? props.playlist.generator : null))
// Правый щелчок (под пальцем — долгое нажатие) — то же меню плейлиста, что в медиатеке
const pla = usePlaylistActions()
const menu = ref<MenuEntry[]>([])
const onMenu = () => (menu.value = pla.menuFor(props.playlist))
</script>

<template>
  <UiContextMenu :items="menu" @open="onMenu">
  <div class="card mix" :style="{ '--tint': tint }">
    <NuxtLink :to="`/playlist/${playlist.id}`" class="card__link" :aria-label="playlist.title" />
    <div class="card__art mix__art">
      <MusicCoverArt v-if="covers.length >= 4" :mosaic="covers" size="small" />
      <MusicCoverArt v-else :image="covers[0] ?? null" size="medium" />
      <span class="mix__veil" aria-hidden="true" />
      <span class="mix__label" aria-hidden="true">
        <span v-if="!special" class="mix__num tnum">{{ index }}</span>
        <span class="mix__kicker">{{ special ? tl(`kind.${special}`) : tl('kicker') }}</span>
      </span>
      <MusicPlayButton class="card__play" :playing="isPlayingContext('playlist', playlist.id)" :loading="loading === `playlist:${playlist.id}`" :size="44" :label="`${t('common.play')}: ${playlist.title}`" @click="playPlaylist(playlist.id)" />
    </div>
    <div class="card__body">
      <div class="card__title">{{ playlist.title }}</div>
      <div class="card__sub">{{ sub }}</div>
    </div>
  </div>
  </UiContextMenu>
</template>

<style scoped>
.mix__art { overflow: hidden; container-type: inline-size; }
/* Полоса цвета микса внизу обложки, как у «Миксов дня» больших сервисов: обложки остаются настоящими */
.mix__veil {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 30%;
  background: var(--tint);
  pointer-events: none;
}
.mix__label { position: absolute; left: 7%; right: 7%; bottom: 0; display: flex; align-items: center; gap: 8px; height: 30%; color: #0b0b10; pointer-events: none; }
.mix__kicker { display: inline-flex; align-items: center; gap: 4px; min-width: 0; font-size: clamp(9px, 6.2cqw, 15px); font-weight: 750; letter-spacing: 0.06em; text-transform: uppercase; white-space: nowrap; }
.mix__num { font-size: clamp(18px, 17cqw, 44px); font-weight: 800; letter-spacing: -0.04em; line-height: 1; }
</style>

<i18n lang="json">
{
  "ru": { "kicker": "Микс дня", "kind": { "on_repeat": "На повторе", "repeat_rewind": "Старые хиты" } },
  "en": { "kicker": "Daily mix", "kind": { "on_repeat": "On repeat", "repeat_rewind": "Repeat rewind" } }
}
</i18n>
