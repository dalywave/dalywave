// Значки Phosphor (MIT), которые рисует плагин: из @iconify-json/ph в src/render/glyphs.gen.ts.
// Берём только нужные — весь набор весит мегабайты. Запуск: bun run glyphs
import icons from '@iconify-json/ph/icons.json' with { type: 'json' }

const NAMES = [
  'play-fill', 'pause-fill', 'skip-forward-fill', 'skip-back-fill', 'play-pause-fill',
  'shuffle-bold', 'repeat-bold', 'repeat-once-bold',
  'speaker-high-bold', 'speaker-low-bold', 'speaker-x-bold', 'speaker-none-bold',
  'heart-bold', 'heart-fill', 'thumbs-down-bold', 'arrow-bend-up-right-bold',
  'plus-bold', 'check-bold', 'playlist-bold', 'list-plus-bold', 'magnifying-glass-bold', 'queue-bold',
  'moon-stars-bold', 'compass-bold', 'devices-bold', 'intersect-bold',
  'link-simple-horizontal-bold', 'wave-sine-bold', 'sparkle-bold', 'vinyl-record-bold',
  'cloud-slash-bold', 'plugs-bold', 'music-notes-bold', 'clock-counter-clockwise-bold', 'chart-line-up-bold',
  'vinyl-record-fill', 'user-bold', 'arrows-left-right-bold', 'fast-forward-bold', 'rewind-bold',
  'laptop-bold', 'desktop-bold', 'device-mobile-bold', 'device-tablet-bold', 'browser-bold', 'television-simple-bold', 'watch-bold',
  'lightning-bold', 'smiley-bold', 'waves-bold', 'cloud-rain-bold', 'infinity-bold', 'sun-horizon-bold', 'car-bold',
  'barbell-bold', 'coffee-bold', 'confetti-bold', 'headphones-bold', 'book-open-bold',
]

const set = icons as { icons: Record<string, { body: string }>, width?: number, height?: number }
const out: string[] = [
  '// Сгенерировано scripts/glyphs.ts из @iconify-json/ph (Phosphor Icons, MIT). Не править руками.',
  '// Все значки — в поле 256×256.',
  'export const GLYPHS = {',
]
for (const name of NAMES) {
  const icon = set.icons[name]
  if (!icon) throw new Error(`нет значка ${name}`)
  out.push(`  '${name}': ${JSON.stringify(icon.body.replace(/currentColor/g, '#fff'))},`)
}
out.push('} as const', '', 'export type GlyphName = keyof typeof GLYPHS', '')
await Bun.write(new URL('../src/render/glyphs.gen.ts', import.meta.url), out.join('\n'))
console.log(`значков: ${NAMES.length}, поле ${set.width ?? 256}`)
