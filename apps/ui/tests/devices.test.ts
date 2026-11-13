// Список «Где играть» без старых записей того же устройства: bun test (из apps/ui)
import { describe, expect, test } from 'bun:test'
import { withoutStale } from '../app/utils/devices'

const dev = (id: string, o: Partial<{ name: string, kind: 'phone' | 'laptop', current: boolean, online: boolean, lastSeenAt: string | null, active: boolean }> = {}) => ({
  id,
  name: o.name ?? 'iPhone',
  kind: o.kind ?? 'phone',
  current: o.current ?? false,
  online: o.online ?? false,
  lastSeenAt: o.lastSeenAt ?? null,
  active: o.active ?? false,
})

describe('withoutStale', () => {
  test('переустановленный iPhone: остаётся тот, что в сети', () => {
    const list = [dev('new', { online: true }), dev('mac', { name: 'MacBook', kind: 'laptop', current: true, online: true }), dev('old', { lastSeenAt: '2026-11-12T12:00:00Z' })]
    expect(withoutStale(list).map(d => d.id)).toEqual(['new', 'mac'])
  })

  test('не в сети — не показываются', () => {
    const list = [dev('a', { lastSeenAt: '2026-11-10T10:00:00Z' }), dev('deck', { name: 'Stream Deck MacBook', kind: 'laptop', lastSeenAt: '2026-11-12T10:00:00Z' }), dev('on', { name: 'iPad', online: true })]
    expect(withoutStale(list).map(d => d.id)).toEqual(['on'])
  })

  test('это устройство и активное не прячутся', () => {
    const list = [dev('here', { current: true, online: true }), dev('old', { active: true }), dev('other', { online: true })]
    expect(withoutStale(list).map(d => d.id)).toEqual(['here', 'old'])
  })

  test('разные имена — разные устройства', () => {
    const list = [dev('a', { online: true }), dev('b', { name: 'iPad', online: true })]
    expect(withoutStale(list).map(d => d.id)).toEqual(['a', 'b'])
  })
})
