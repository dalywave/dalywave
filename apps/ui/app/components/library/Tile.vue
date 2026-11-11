<script setup lang="ts">
import type { ImageSet } from '~/api/types'

// Цветная плитка в верхнем ряду медиатеки: «Любимые треки», «Моя волна», «Недавно добавленные».
// У каждой своя палитра из цветов настроений, значок, название и одна строка подписи
const props = defineProps<{
  tone: 'liked' | 'wave' | 'recent'
  icon: string
  title: string
  meta?: string
  to?: string
  /** Обложки последних добавленных — веером справа (только на широком экране) */
  covers?: ImageSet[]
  active?: boolean
}>()
defineEmits<{ click: [] }>()

const bridge = useBridge()
const NuxtLink = resolveComponent('NuxtLink')
const tag = computed(() => (props.to ? NuxtLink : 'button'))
</script>

<template>
  <component :is="tag" :to="to" :type="to ? undefined : 'button'" class="tile" :class="[`tile--${tone}`, { 'is-active': active }]">
    <svg v-if="tone === 'wave'" class="tile__wave" viewBox="0 0 200 100" preserveAspectRatio="none" aria-hidden="true">
      <path d="M-10 62 C 30 30, 60 30, 100 56 S 170 84, 210 44" fill="none" stroke="rgba(255,255,255,0.7)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
      <path d="M-10 70 C 40 46, 70 40, 110 62 S 175 80, 210 54" fill="none" stroke="rgba(174,240,255,0.35)" stroke-width="6" vector-effect="non-scaling-stroke" />
    </svg>
    <Icon v-if="tone === 'liked'" name="ph:heart-fill" class="tile__heart" aria-hidden="true" />
    <span v-if="covers?.length" class="tile__fan" aria-hidden="true">
      <img v-for="(c, i) in covers.slice(0, 3)" :key="i" :src="bridge.resolveUrl(c.small)" alt="" loading="lazy" decoding="async">
    </span>
    <span class="tile__icon"><Icon :name="icon" /></span>
    <span class="tile__text">
      <span class="tile__title">{{ title }}</span>
      <span v-if="meta" class="tile__meta">{{ meta }}</span>
    </span>
  </component>
</template>

<style scoped>
.tile {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  min-height: 128px;
  padding: 16px 18px;
  overflow: hidden;
  isolation: isolate;
  border: 0;
  border-radius: var(--r-4);
  color: #fff;
  font: inherit;
  text-align: left;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08), var(--shadow-1);
  transition: transform var(--dur-3) var(--ease-out), box-shadow var(--dur-2);
  -webkit-tap-highlight-color: transparent;
}
.tile:hover { transform: translateY(-2px); }
.tile:active { transform: scale(0.98); }
.tile:focus-visible { outline: none; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08), 0 0 0 2px var(--focus); }
.tile.is-active { box-shadow: inset 0 0 0 1.5px rgba(255, 255, 255, 0.55), var(--shadow-1); }

.tile--liked { background: radial-gradient(90% 120% at 100% 0%, color-mix(in oklab, var(--brand-3) 85%, transparent), transparent 60%), radial-gradient(90% 120% at 0% 100%, var(--brand-1), transparent 70%), var(--brand-2); }
.tile--wave { background: radial-gradient(80% 90% at 85% 10%, color-mix(in oklab, var(--mood-calm-3) 40%, transparent), transparent 60%), radial-gradient(90% 100% at 0% 100%, var(--mood-calm-2), transparent 75%), var(--mood-calm-1); }
.tile--recent { background: radial-gradient(80% 100% at 100% 0%, color-mix(in oklab, var(--mood-energetic-3) 55%, transparent), transparent 60%), radial-gradient(90% 110% at 0% 100%, var(--mood-energetic-2), transparent 75%), var(--mood-energetic-1); }

.tile__icon { position: relative; z-index: 1; display: grid; place-items: center; width: 40px; height: 40px; border-radius: 50%; background: rgba(255, 255, 255, 0.18); box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.2); font-size: 20px; backdrop-filter: blur(8px); }
.tile__text { position: relative; z-index: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.tile__title { font-size: var(--fs-h4); font-weight: 700; letter-spacing: -0.01em; line-height: 1.2; text-wrap: balance; }
.tile__meta { overflow: hidden; font-size: var(--fs-small); color: rgba(255, 255, 255, 0.82); text-overflow: ellipsis; white-space: nowrap; }

.tile__wave { position: absolute; left: 0; top: 0; z-index: 0; width: 100%; height: 62%; opacity: 0.9; }
/* Большое сердце за краем — как обложка «Любимых» */
.tile__heart { position: absolute; right: -14px; bottom: -26px; z-index: 0; font-size: 150px; color: rgba(255, 255, 255, 0.14); transform: rotate(-12deg); }

/* Веер из обложек последних добавленных */
.tile__fan { position: absolute; right: 18px; top: 50%; z-index: 0; width: 120px; height: 84px; transform: translateY(-50%); }
.tile__fan img { position: absolute; top: 0; width: 84px; height: 84px; border-radius: var(--r-2); object-fit: cover; box-shadow: 0 10px 24px -8px rgba(0, 0, 0, 0.7), 0 0 0 1px rgba(255, 255, 255, 0.1); transition: transform var(--dur-3) var(--ease-out); }
.tile__fan img:nth-child(1) { right: 0; transform: rotate(8deg); z-index: 3; }
.tile__fan img:nth-child(2) { right: 18px; transform: rotate(-2deg) scale(0.94); z-index: 2; filter: brightness(0.8); }
.tile__fan img:nth-child(3) { right: 36px; transform: rotate(-12deg) scale(0.88); z-index: 1; filter: brightness(0.6); }
.tile:hover .tile__fan img:nth-child(1) { transform: rotate(12deg) translateX(4px); }
.tile:hover .tile__fan img:nth-child(3) { transform: rotate(-16deg) scale(0.88) translateX(-4px); }

@media (max-width: 699px) {
  .tile { min-height: 0; aspect-ratio: 1 / 1.08; padding: 12px; gap: 8px; border-radius: var(--r-3); }
  .tile__icon { width: 32px; height: 32px; font-size: 17px; }
  .tile__title { font-size: 14px; }
  .tile__meta { font-size: var(--fs-caption); }
  .tile__fan { display: none; }
  .tile__heart { right: -18px; bottom: -18px; font-size: 84px; }
}
</style>
