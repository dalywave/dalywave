// Уведомления: подписи времени, ссылки, ключ VAPID, тихие часы — bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { browserName, minutesToTime, notificationRoute, notificationTime, sameKey, timeToMinutes, vapidKey } from '../app/utils/notifications'

describe('notificationTime', () => {
  const now = new Date(2026, 10, 12, 15, 0, 0)
  const ago = (ms: number) => new Date(now.getTime() - ms).toISOString()
  test('minutes and hours today', () => {
    expect(notificationTime(ago(20_000), now)).toBe('сейчас')
    expect(notificationTime(ago(5 * 60_000), now)).toBe('5 мин')
    expect(notificationTime(ago(3 * 3600_000), now)).toBe('3 ч')
    expect(notificationTime(ago(5 * 60_000), now, 'en')).toBe('5m')
  })
  test('yesterday by calendar day, not by 24 hours', () => {
    expect(notificationTime(new Date(2026, 10, 11, 23, 30).toISOString(), now)).toBe('вчера')
    expect(notificationTime(new Date(2026, 10, 11, 9, 0).toISOString(), now, 'en')).toBe('yesterday')
  })
  test('older dates', () => {
    expect(notificationTime(new Date(2026, 9, 1).toISOString(), now)).toMatch(/1/)
    expect(notificationTime(new Date(2025, 0, 3).toISOString(), now)).toMatch(/2025/)
    expect(notificationTime('not a date', now)).toBe('')
  })
})

describe('notificationRoute', () => {
  test('only paths of this site', () => {
    expect(notificationRoute('/album/0190a')).toBe('/album/0190a')
    expect(notificationRoute('/rewind?year=2026')).toBe('/rewind?year=2026')
    expect(notificationRoute('//evil.example/x')).toBeNull()
    expect(notificationRoute('https://evil.example')).toBeNull()
    expect(notificationRoute('javascript:alert(1)')).toBeNull()
    expect(notificationRoute(null)).toBeNull()
  })
})

describe('vapid key', () => {
  test('base64url decodes and compares', () => {
    // 65 байт открытого ключа P-256: 0x04 и 64 байта
    const bytes = new Uint8Array(65).map((_, i) => (i === 0 ? 4 : i))
    const b64 = btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
    const key = vapidKey(b64)
    expect(key.length).toBe(65)
    expect(key[0]).toBe(4)
    expect(sameKey(bytes.buffer, key)).toBe(true)
    expect(sameKey(new Uint8Array(65).buffer, key)).toBe(false)
    expect(sameKey(null, key)).toBe(false)
  })
})

describe('quiet hours', () => {
  test('minutes ↔ time', () => {
    expect(minutesToTime(1380)).toBe('23:00')
    expect(minutesToTime(480)).toBe('08:00')
    expect(minutesToTime(0)).toBe('00:00')
    expect(timeToMinutes('07:30')).toBe(450)
    expect(timeToMinutes('24:00')).toBeNull()
    expect(timeToMinutes('nope')).toBeNull()
  })
})

test('browserName', () => {
  expect(browserName('Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15')).toBe('Safari, Mac')
  expect(browserName('Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Mobile Safari/537.36')).toBe('Chrome, Android')
})
