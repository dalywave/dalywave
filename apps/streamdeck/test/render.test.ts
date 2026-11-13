import { describe, expect, test } from 'bun:test'
import { coverKey, iconKey, shrink, statusKey } from '../src/render/keys'
import { listFields, pulseFields, trackFields, volumeFields, volumeGlyph } from '../src/render/strip'
import { fit, textWidth } from '../src/render/svg'

function decode(uri: string): string {
  expect(uri.startsWith('data:image/svg+xml;base64,')).toBe(true)
  return Buffer.from(uri.slice(uri.indexOf(',') + 1), 'base64').toString('utf8')
}

describe('текст', () => {
  test('короткое не трогается, длинное — с многоточием и влезает', () => {
    expect(fit('Пауза', 120, 20)).toBe('Пауза')
    const long = fit('Очень длинное название плейлиста', 120, 20, 600)
    expect(long.endsWith('…')).toBe(true)
    expect(textWidth(long, 20, 600)).toBeLessThanOrEqual(120)
  })

  test('подпись сначала мельчает, потом режется', () => {
    expect(shrink('Сон', 132)).toEqual(['Сон', 21])
    const [mid, size] = shrink('Нет устройства', 132)
    expect(mid).toBe('Нет устройства')
    expect(size).toBeLessThan(21)
    const [cut, small] = shrink('Работа · Спокойное · без слов', 132)
    expect(small).toBe(16)
    expect(cut.endsWith('…')).toBe(true)
  })
})

describe('кнопки', () => {
  test('значок с подписью: SVG 144×144, текст экранирован', () => {
    const svg = decode(iconKey({ glyph: 'play-fill', label: 'A & <B>' }))
    expect(svg).toContain('width="144" height="144"')
    expect(svg).toContain('A &amp; &lt;B&gt;')
    expect(svg).not.toContain('<B>')
  })

  test('включённое — с точкой, выключенное — без', () => {
    expect(decode(iconKey({ glyph: 'shuffle-bold', tone: 'on' }))).toContain('<circle cx="72" cy="124"')
    expect(decode(iconKey({ glyph: 'shuffle-bold', tone: 'off' }))).not.toContain('<circle')
  })

  test('таймер: кольцо и остаток', () => {
    const svg = decode(iconKey({ glyph: 'moon-stars-bold', big: '23:41', ring: 0.5 }))
    expect(svg).toContain('23:41')
    expect(svg).toContain('A58 58 0 0 1 72.00 130.00')
  })

  test('«Сейчас играет»: прогресс по ширине, на паузе — значок «играть»', () => {
    const playing = decode(coverKey({ cover: 'data:image/jpeg;base64,AAAA', progress: 0.25, playing: true }))
    expect(playing).toContain('width="36.0" height="6"')
    expect(playing).toContain('data:image/jpeg;base64,AAAA')
    const paused = decode(coverKey({ cover: null, progress: 0, playing: false }))
    expect(paused).toContain('fill-opacity="0.45"')
  })

  test('пульс затемняет обложку между долями', () => {
    expect(decode(coverKey({ cover: null, progress: 0, playing: true, shade: 0.14 }))).toContain('fill-opacity="0.14"')
    expect(decode(coverKey({ cover: null, progress: 0, playing: true }))).not.toContain('<rect width="144" height="144" fill="#000"')
  })

  test('без подключения — понятная кнопка', () => {
    expect(decode(statusKey('connect', 'Подключите'))).toContain('Подключите')
  })
})

describe('полоса', () => {
  test('трек: прогресс 0…1000, на паузе — затемнение', () => {
    const f = trackFields({ cover: null, title: 'T', subtitle: 'A', progress: 0.5, playing: false })
    expect(f.progress).toEqual({ value: 500 })
    expect((f.state as { enabled: boolean }).enabled).toBe(true)
    expect(f.title).toBe('T')
  })

  test('громкость: проценты и значок по уровню', () => {
    const f = volumeFields({ level: 0.42, muted: false, label: '42 %', track: 'x' })
    expect((f.bar as { value: number }).value).toBe(42)
    expect(volumeGlyph(0.2, false)).toBe('speaker-low-bold')
    expect(volumeGlyph(0.8, false)).toBe('speaker-high-bold')
    expect(volumeGlyph(0.8, true)).toBe('speaker-x-bold')
    expect((volumeFields({ level: 0.42, muted: true, label: '', track: '' }).bar as { value: number }).value).toBe(0)
  })

  test('список: волна без обложки рисуется в цветах настроения', () => {
    const f = listFields({ cover: null, fallback: 'waves-bold', colors: ['#1d8cbc', '#aef0ff'], title: 'Спокойное', subtitle: '', counter: '2 из 7' })
    expect(decode(f.cover as string)).toContain('#aef0ff')
    expect(f.counter).toBe('2 из 7')
  })

  test('пульс: прозрачность шагами по 0,1 и не ярче 0,2', () => {
    expect(pulseFields(0.1)).toEqual({ pulse: { opacity: 0.1 } })
    expect(pulseFields(0.9)).toEqual({ pulse: { opacity: 0.2 } })
    expect(pulseFields(-1)).toEqual({ pulse: { opacity: 0 } })
  })
})
