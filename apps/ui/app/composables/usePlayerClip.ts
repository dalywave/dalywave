import type { Ref } from 'vue'
import type { Track } from '~/api/types'
import type { ClipStyle } from '~/stores/prefs'

export type ClipLayout = Exclude<ClipStyle, 'off'>

/** Через столько без движения «Кино» прячет интерфейс */
const IDLE_MS = 3000

/**
 * Клип в полном плеере: как он показан (настройка «Клипы в плеере») и что из этого следует для
 * раскладки. Квадрат — вместо обложки; «Широкий» — целиком в своих пропорциях
 * с подсветкой его цветов; «Фон» — на весь плеер, интерфейс поверх; «Кино» — крупно, интерфейс
 * прячется без движения. Живая обложка альбома — всегда квадратом: это и есть обложка.
 *
 * Раскладка держится, пока про новый трек ничего не известно: смена песни не дёргает плеер
 * между обложкой и клипом. Видео — в `<MusicPlayerClip>`: он же сообщает, что кадры пошли
 * (`setShown`) и какие у клипа пропорции (`setRatio`)
 */
export function usePlayerClip(opts: {
  track: Ref<Track | null>
  open: Ref<boolean>
  /** Блок обложки: «Широкий» подгоняет его высоту под пропорции клипа */
  art: Ref<HTMLElement | null>
  /** Не прятать интерфейс «Кино» (открыто меню, текст песни) */
  busy?: () => boolean
}) {
  const prefs = usePrefsStore()
  const motion = useMotionStore()
  const player = usePlayerStore()

  const enabled = computed(() => prefs.clipStyle !== 'off' && motion.allowed)
  // Видео заказывается, как только трек заиграл: к раскрытию плеера оно уже готово. «Выключено» —
  // не грузится вовсе
  watch([() => opts.track.value?.id, enabled], ([id, on]) => {
    if (on) motion.prefetch(id)
  }, { immediate: true })

  const kind = computed(() => (enabled.value ? motion.kind(opts.track.value) : 'none'))
  const known = ref<ClipLayout | null>(null)
  watchEffect(() => {
    const k = kind.value
    if (k === null) return
    known.value = k === 'clip' ? prefs.clipStyle as ClipLayout : k === 'artwork' ? 'square' : null
  })
  /** Трек ещё готовится или звук пропал — обложка на месте: на ней подсказка и «Пропустить» */
  const layout = computed<ClipLayout | null>(() => (player.pending || player.soundLost ? null : known.value))
  const screen = computed(() => layout.value === 'backdrop' || layout.value === 'cinema')
  const src = computed(() => (layout.value ? motion.src(opts.track.value) : null))
  /** Играет вместе с музыкой: пауза — и клип стоит */
  const playing = computed(() => opts.open.value && player.playing)

  const shown = ref(false)
  const ratio = ref(16 / 9)
  watch([src, layout], () => (shown.value = false))

  // «Широкий»: горизонтальный клип — блок обложки по его пропорциям, вертикальный — в квадрате
  const { width: artWidth } = useElementSize(opts.art)
  const artStyle = computed(() => {
    if (layout.value !== 'wide' || !artWidth.value) return undefined
    return { height: `${Math.round(artWidth.value / Math.max(1, ratio.value))}px` }
  })

  // ---------- «Кино»: интерфейс уходит без движения, возвращается от мыши, касания, клавиши ----------
  const idle = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined
  function wake() {
    if (idle.value) idle.value = false
    clearTimeout(timer)
    if (layout.value !== 'cinema' || !shown.value || !playing.value || opts.busy?.()) return
    timer = setTimeout(() => (idle.value = true), IDLE_MS)
  }
  watch([layout, shown, playing, () => opts.busy?.()], wake)
  useEventListener(window, ['pointermove', 'pointerdown', 'keydown', 'wheel'], () => {
    if (layout.value === 'cinema') wake()
  }, { passive: true, capture: true })
  onBeforeUnmount(() => clearTimeout(timer))

  const classes = computed(() => ({
    [`clip-${layout.value}`]: !!layout.value,
    'clip-land': layout.value === 'wide' && ratio.value > 1,
    'clip-screen': screen.value,
    'clip-shown': shown.value,
    'clip-idle': idle.value,
    // Поверх видео — светлый текст и на светлой теме
    'scheme-dark': screen.value,
  }))

  return reactive({
    layout,
    screen,
    src,
    playing,
    shown,
    ratio,
    idle,
    artStyle,
    classes,
    /** Тёмный фон из видео под текстом песни на телефоне — только там, где клип не на весь экран */
    lyricsBackdrop: computed(() => layout.value === 'square' || layout.value === 'wide'),
    setShown: (v: boolean) => (shown.value = v),
    setRatio: (r: number) => {
      if (Number.isFinite(r) && r > 0) ratio.value = r
    },
    wake,
  })
}

export type PlayerClip = ReturnType<typeof usePlayerClip>
