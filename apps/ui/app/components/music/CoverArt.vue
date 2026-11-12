<script setup lang="ts">
import type { ImageSet } from '~/api/types'

// Обложка: одна картинка, мозаика из четырёх или заглушка. ambient — мягкое свечение цвета обложки
const props = withDefaults(defineProps<{
  image?: ImageSet | null
  mosaic?: ImageSet[]
  size?: 'small' | 'medium' | 'large'
  alt?: string
  round?: boolean
  radius?: string
  ambient?: boolean
  eager?: boolean
  fallbackIcon?: string
}>(), { size: 'medium', alt: '', fallbackIcon: 'ph:music-notes-simple' })

const bridge = useBridge()
const failed = ref(false)
const src = computed(() => {
  const url = props.image?.[props.size]
  return url ? bridge.resolveUrl(url) : null
})
const tiles = computed(() => (props.mosaic?.length && props.mosaic.length >= 4 ? props.mosaic.slice(0, 4) : null))
watch(src, () => (failed.value = false))
</script>

<template>
  <div class="art" :class="{ 'art--round': round, 'art--ambient': ambient && src && !failed }" :style="radius ? { borderRadius: radius } : undefined">
    <img
      v-if="ambient && src && !failed"
      :src="src"
      alt=""
      class="art__glow"
      aria-hidden="true"
      loading="lazy"
    >
    <div v-if="tiles" class="art__mosaic">
      <img v-for="(t, i) in tiles" :key="i" :src="bridge.resolveUrl(t.small)" alt="" loading="lazy" decoding="async">
    </div>
    <img
      v-else-if="src && !failed"
      :src="src"
      :alt="alt"
      class="art__img"
      :loading="eager ? 'eager' : 'lazy'"
      decoding="async"
      @error="failed = true"
    >
    <div v-else class="art__fallback" role="img" :aria-label="alt || undefined">
      <Icon :name="fallbackIcon" />
    </div>
  </div>
</template>

<style scoped>
.art {
  position: relative;
  aspect-ratio: 1;
  border-radius: var(--r-2);
  background: var(--ink-3);
  flex: none;
  isolation: isolate;
}
.art::after { content: ''; position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 0 1px rgba(var(--hi-rgb), 0.06); pointer-events: none; }
.art__img, .art__mosaic { position: relative; width: 100%; height: 100%; border-radius: inherit; overflow: hidden; }
.art__img { object-fit: cover; }
.art__mosaic { display: grid; grid-template-columns: 1fr 1fr; }
.art__mosaic img { width: 100%; height: 100%; object-fit: cover; }
.art--round { border-radius: 50%; }
.art__fallback {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  border-radius: inherit;
  background: radial-gradient(120% 100% at 20% 0%, color-mix(in oklab, var(--accent) 22%, transparent), transparent 60%), linear-gradient(135deg, var(--ink-4), var(--ink-2));
  color: var(--text-3);
  font-size: 28%;
}
.art__fallback :deep(*) { font-size: max(16px, 2.2em); }
/* Светлая: заглушка — ровная серая плашка, как пустая обложка у Apple Music */
:root[data-theme='light'] .art__fallback { background: linear-gradient(135deg, var(--ink-3), var(--ink-0)); }
.art__glow {
  position: absolute;
  inset: 8% 6% -6%;
  z-index: -1;
  width: 88%;
  height: 98%;
  object-fit: cover;
  border-radius: inherit;
  filter: blur(28px) saturate(1.3);
  opacity: 0.55;
  transform: translateY(6%);
}
</style>
