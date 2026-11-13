// Имя файла выгрузки и код встраиваемого плеера: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { EMBED_HEIGHT, embedCode, exportFileName } from '../app/utils/export'

describe('имя файла выгрузки', () => {
  test('без запрещённых символов, как на сервере', () => {
    expect(exportFileName('Ночь / день: 2026?', 'x')).toBe('Ночь _ день_ 2026_')
    expect(exportFileName('  ..hidden.  ', 'x')).toBe('hidden')
    expect(exportFileName('a\nb', 'x')).toBe('a b')
    expect(exportFileName('', 'Плейлист')).toBe('Плейлист')
    expect(exportFileName('я'.repeat(200), 'x')).toHaveLength(80)
  })
})

describe('код встраивания', () => {
  test('высота по размеру, кавычки в названии экранированы', () => {
    const code = embedCode('https://wave.daly.rest/embed/album/1?theme=dark', 'large', 'Rock & "Roll"')
    expect(code).toStartWith('<iframe src="https://wave.daly.rest/embed/album/1?theme=dark"')
    expect(code).toContain(`height="${EMBED_HEIGHT.large}"`)
    expect(code).toContain('title="Rock &amp; &quot;Roll&quot; — DalyWave"')
    expect(embedCode('https://x/embed/track/1?a=1&b=2', 'compact', '')).toContain('src="https://x/embed/track/1?a=1&amp;b=2" width="100%" height="152"')
  })
})
