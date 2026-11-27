<script setup lang="ts">
import type { ImageSet } from '~/api/types'
import type { WaveMood } from '~/bridge/types'
import { MOOD_ORB } from '~/utils/wave-options'

// «Настроения и жанры»: настроение включает волну, жанр — поиск по каталогу.
// У жанра в углу — фото артиста этого жанра из каталога (запоминается на неделю)
const router = useRouter()
const bridge = useBridge()
const player = usePlayerStore()
const { t } = useI18n({ useScope: 'local' })
const { tg } = useGlobalT()

// Плитки — ровной заливкой, как у Spotify: цвет несут шар настроения и фото артиста, не градиент
const MOODS: { mood: Exclude<WaveMood, 'any'>, tile: string }[] = [
  { mood: 'energetic', tile: '#9a3a12' },
  { mood: 'happy', tile: '#8a2468' },
  { mood: 'calm', tile: '#14557a' },
  { mood: 'sad', tile: '#363080' },
]

interface Genre { id: string, query: string, seed: string, color: string }
const GENRES: Genre[] = [
  { id: 'ruRap', query: 'русский рэп', seed: 'Mzlff', color: 'var(--brand-1)' },
  { id: 'hyperpop', query: 'hyperpop', seed: '5opka', color: 'var(--brand-2)' },
  { id: 'phonk', query: 'phonk', seed: 'Kordhell', color: '#c4123f' },
  { id: 'alt', query: 'альтернатива', seed: 'Lowborn', color: '#2a6a6e' },
  { id: 'popPunk', query: 'pop punk', seed: 'кис-кис', color: 'var(--brand-3)' },
  { id: 'electronic', query: 'electronic', seed: 'Moderat', color: '#1f5fd1' },
  { id: 'indie', query: 'indie', seed: 'Хадн Дадн', color: '#c2611c' },
  { id: 'rock', query: 'рок', seed: 'Кино', color: '#5a5f6e' },
]

const pending = ref<WaveMood | null>(null)
async function playMood(mood: WaveMood) {
  if (pending.value) return
  pending.value = mood
  try {
    await player.startWave({ mood })
  }
  finally {
    pending.value = null
  }
}
const isOn = (mood: WaveMood) => player.waveActive && player.wave.mood === mood

function openGenre(g: Genre) {
  void router.push({ path: '/search', query: { q: g.query } })
}

// Фото артистов для плиток жанров
const ART_KEY = 'dw.search.genreArt'
const WEEK = 7 * 24 * 3600 * 1000
const art = ref<Record<string, ImageSet>>({})

function readArt(): { at: number, map: Record<string, ImageSet> } | null {
  try {
    const raw = JSON.parse(localStorage.getItem(ART_KEY) ?? 'null') as { at?: number, map?: Record<string, ImageSet> } | null
    return raw?.map && typeof raw.at === 'number' ? { at: raw.at, map: raw.map } : null
  }
  catch {
    return null
  }
}

async function loadArt() {
  const cached = readArt()
  if (cached) art.value = cached.map
  if (cached && Date.now() - cached.at < WEEK && GENRES.every(g => cached.map[g.id])) return
  const fresh = !!cached && Date.now() - cached.at < WEEK
  const at = fresh ? cached!.at : Date.now()
  // Каждое найденное фото сохраняется сразу: ушли со страницы — остальные догрузятся в другой раз
  await Promise.all(GENRES.filter(g => !fresh || !cached!.map[g.id]).map(async (g) => {
    try {
      const r = await bridge.catalog.search(g.seed, 'catalog', 3)
      const needle = g.seed.toLowerCase()
      const a = r.artists.find(x => x.name.toLowerCase() === needle) ?? r.artists[0]
      if (!a?.image) return
      art.value = { ...art.value, [g.id]: a.image }
      localStorage.setItem(ART_KEY, JSON.stringify({ at, map: art.value }))
    }
    catch {
      // Нет сети или хранилища — плитка остаётся типографской
    }
  }))
}
onMounted(() => void loadArt())
</script>

<template>
  <section class="browse" :aria-label="t('title')">
    <h2 class="browse__title">{{ t('title') }}</h2>
    <div class="browse__grid">
      <button
        v-for="m in MOODS"
        :key="m.mood"
        type="button"
        class="tile"
        :class="{ 'is-busy': pending === m.mood, 'is-on': isOn(m.mood) }"
        :style="{ '--tile': m.tile }"
        :aria-pressed="isOn(m.mood)"
        @click="playMood(m.mood)"
      >
        <span class="tile__name">{{ tg(`wave.mood.${m.mood}`) }}</span>
        <span class="tile__orb" :style="{ '--o1': MOOD_ORB[m.mood][0], '--o2': MOOD_ORB[m.mood][1], '--o3': MOOD_ORB[m.mood][2] }" aria-hidden="true" />
        <Icon v-if="isOn(m.mood)" name="ph:waveform" class="tile__on" />
      </button>
      <button
        v-for="g in GENRES"
        :key="g.id"
        type="button"
        class="tile"
        :style="{ '--tile': `color-mix(in oklab, ${g.color} 62%, #0b0c10)` }"
        @click="openGenre(g)"
      >
        <span class="tile__name">{{ t(`genre.${g.id}`) }}</span>
        <MusicCoverArt v-if="art[g.id]" :image="art[g.id]" size="small" class="tile__art" />
      </button>
    </div>
  </section>
</template>

<style scoped>
.browse { display: flex; flex-direction: column; gap: 12px; }
.browse__title { margin: 0; font-size: var(--fs-h3); font-weight: 700; letter-spacing: var(--tr-title); }
.browse__grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 16px; }
.tile {
  position: relative;
  height: 112px;
  overflow: hidden;
  padding: 0;
  border: 0;
  border-radius: var(--r-3);
  background: var(--tile);
  text-align: left;
  color: #fff;
  isolation: isolate;
  transition: transform var(--dur-1) var(--ease-out), filter var(--dur-1);
}
.tile::after { content: ''; position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.07); pointer-events: none; }
.tile:hover { filter: brightness(1.08); }
.tile:active { transform: scale(0.97); }
.tile:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
.tile.is-busy { animation: tile-busy 0.9s var(--ease-in-out) infinite alternate; }
.tile__name { position: absolute; left: 16px; top: 14px; right: 40%; z-index: 2; font-size: 17px; font-weight: 700; letter-spacing: -0.01em; line-height: 1.2; text-wrap: balance; }
.tile__orb {
  position: absolute;
  right: -14px;
  bottom: -18px;
  width: 76px;
  height: 76px;
  border-radius: 50%;
  background: radial-gradient(110% 110% at 30% 22%, var(--o3) 0%, var(--o2) 42%, var(--o1) 100%);
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
}
.tile__art {
  position: absolute;
  right: -12px;
  bottom: -6px;
  width: 72px;
  border-radius: var(--r-1);
  box-shadow: -2px 4px 12px -4px rgba(0, 0, 0, 0.45);
  transform: rotate(24deg);
}
.tile__on { position: absolute; left: 16px; bottom: 12px; z-index: 2; font-size: 20px; }
@keyframes tile-busy { to { opacity: 0.7; } }
@media (max-width: 1099px) {
  .browse__grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
}
@media (max-width: 699px) {
  .browse__title { font-size: 20px; }
  .browse__grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .tile { height: 96px; }
  .tile__name { left: 12px; top: 12px; font-size: 16px; }
  .tile__orb { width: 64px; height: 64px; }
  .tile__art { width: 64px; }
  .tile__on { left: 12px; }
}
@media (prefers-reduced-motion: reduce) {
  .tile.is-busy { animation: none; opacity: 0.7; }
}
</style>

<i18n lang="json">
{
  "ru": {
    "title": "Настроения и жанры",
    "genre": {
      "ruRap": "Русский рэп",
      "hyperpop": "Гиперпоп",
      "phonk": "Фонк",
      "alt": "Альтернатива",
      "popPunk": "Поп-панк",
      "electronic": "Электроника",
      "indie": "Инди",
      "rock": "Рок"
    }
  },
  "uk": {
    "title": "Настрої та жанри",
    "genre": {
      "ruRap": "Російський реп",
      "hyperpop": "Гіперпоп",
      "phonk": "Фонк",
      "alt": "Альтернатива",
      "popPunk": "Поп-панк",
      "electronic": "Електроніка",
      "indie": "Інді",
      "rock": "Рок"
    }
  },
  "en": {
    "title": "Moods and genres",
    "genre": {
      "ruRap": "Russian rap",
      "hyperpop": "Hyperpop",
      "phonk": "Phonk",
      "alt": "Alternative",
      "popPunk": "Pop punk",
      "electronic": "Electronic",
      "indie": "Indie",
      "rock": "Rock"
    }
  }
}
</i18n>
