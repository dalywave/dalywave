<script setup lang="ts">
import type { OverlayNow } from '~/api/types'
import type { PlateSide, PlateSize, PlateStyle, PlateTrack } from '~/components/overlay/Plate.vue'

// Оверлей «Сейчас играет» для OBS (Browser Source): прозрачная страница с одной плашкой. Вход не
// нужен — секретная ссылка из настроек (`?k=dwo_…`) даёт только «что играет». Вид — параметрами:
// `style=line|card`, `side=left|right`, `size=s|m|l`. Данные — потоком SSE /api/overlay/stream;
// пауза или приватная сессия — плашка тихо уходит. `?demo` — пример без ссылки (вёрстка, предпросмотр)
definePageMeta({ layout: false })
useHead({ title: 'DalyWave', htmlAttrs: { 'data-overlay': '' } })

const route = useRoute()
const pick = <T extends string>(name: string, allowed: readonly T[], fallback: T): T => {
  const v = route.query[name]
  return typeof v === 'string' && (allowed as readonly string[]).includes(v) ? v as T : fallback
}
const look = pick<PlateStyle>('style', ['line', 'card'], 'card')
const side = pick<PlateSide>('side', ['left', 'right'], 'left')
const size = pick<PlateSize>('size', ['s', 'm', 'l'], 'm')
const key = typeof route.query.k === 'string' ? route.query.k : ''
const demo = route.query.demo !== undefined

const track = ref<PlateTrack | null>(null)
const visible = ref(false)
const playing = ref(false)
const positionMs = ref(0)
const durationMs = ref(0)
const at = ref(Date.now())

const DEMO: PlateTrack[] = [
  { id: 'd1', title: 'Float', artist: 'Call Me Karizma', cover: null },
  { id: 'd2', title: 'Midnight City', artist: 'M83', cover: null },
]

function apply(n: OverlayNow) {
  // Время сервера → время этой страницы: позиция досчитывается от момента получения
  track.value = n.track ? { id: n.track.id, title: n.track.title, artist: n.track.artist, cover: n.track.cover } : null
  visible.value = n.visible
  playing.value = n.playing
  positionMs.value = n.positionMs
  durationMs.value = n.durationMs
  at.value = Date.now()
}

let source: EventSource | null = null
let retry: ReturnType<typeof setTimeout> | undefined
let demoTimer: ReturnType<typeof setInterval> | undefined

function connect() {
  source?.close()
  source = new EventSource(`/api/overlay/stream?k=${encodeURIComponent(key)}`)
  source.addEventListener('now', (e) => {
    try {
      apply(JSON.parse((e as MessageEvent<string>).data) as OverlayNow)
    }
    catch {
      // Битое событие — ждём следующее
    }
  })
  source.addEventListener('gone', () => {
    visible.value = false
    source?.close()
    source = null
  })
  // Сеть пропала — EventSource переподключится сам; ссылка не найдена (404) — он сдаётся, пробуем реже
  source.onerror = () => {
    if (source?.readyState !== EventSource.CLOSED) return
    visible.value = false
    clearTimeout(retry)
    retry = setTimeout(connect, 30_000)
  }
}

onMounted(() => {
  if (demo) {
    let i = 0
    const show = () => apply({ visible: true, playing: true, positionMs: 62_000, durationMs: 169_000, at: new Date().toISOString(), track: { ...DEMO[i % DEMO.length]!, cover: null } })
    show()
    demoTimer = setInterval(() => {
      i++
      show()
    }, 6000)
    return
  }
  if (key) connect()
})

onBeforeUnmount(() => {
  source?.close()
  clearTimeout(retry)
  clearInterval(demoTimer)
})
</script>

<template>
  <main class="ov">
    <OverlayPlate
      :track="track"
      :visible="visible"
      :playing="playing"
      :position-ms="positionMs"
      :duration-ms="durationMs"
      :at="at"
      :look="look"
      :side="side"
      :size="size"
    />
  </main>
</template>

<style>
/* Прозрачный фон для OBS: ни фона темы, ни свечения обложки */
html[data-overlay], html[data-overlay] body { background: transparent !important; }
html[data-overlay] .ambient, html[data-overlay] .offline { display: none !important; }
</style>

<style scoped>
.ov { padding: 16px; }
</style>
