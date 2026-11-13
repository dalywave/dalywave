// Подписи сочетаний клавиш на Mac и на Windows: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { isAppleUa, platformKeys } from '../app/utils/keys'

describe('подписи клавиш', () => {
  test('клавиатура Apple — по строке браузера', () => {
    expect(isAppleUa('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15')).toBe(true)
    expect(isAppleUa('Mozilla/5.0 (iPhone; CPU iPhone OS 27_2 like Mac OS X)')).toBe(true)
    expect(isAppleUa('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0')).toBe(false)
    expect(isAppleUa('Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36')).toBe(false)
  })

  test('на Mac — значки как есть', () => {
    expect(platformKeys(['⌘', '⇧', 'M'], true)).toEqual(['⌘', '⇧', 'M'])
  })

  test('на Windows и Linux — Ctrl, Alt, Shift', () => {
    expect(platformKeys(['⌘', 'K'], false)).toEqual(['Ctrl', 'K'])
    expect(platformKeys(['⌘', '⇧', 'M'], false)).toEqual(['Ctrl', 'Shift', 'M'])
    expect(platformKeys(['⌥', '⇧', '←'], false)).toEqual(['Alt', 'Shift', '←'])
    expect(platformKeys(['Esc'], false)).toEqual(['Esc'])
  })
})
