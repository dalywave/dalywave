// UUID без crypto.randomUUID (сайт по http не с localhost): bun test (из apps/ui)
import { afterEach, describe, expect, test } from 'bun:test'
import { uuid } from '../app/utils/web-api'

const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
// eslint-disable-next-line no-restricted-properties -- тест подменяет сам randomUUID
const native = crypto.randomUUID

afterEach(() => {
  Object.defineProperty(crypto, 'randomUUID', { value: native, configurable: true, writable: true })
})

describe('uuid', () => {
  test('есть crypto.randomUUID — берётся он', () => {
    expect(uuid()).toMatch(UUID_V4)
  })
  test('нет crypto.randomUUID — UUID v4 из getRandomValues, каждый раз новый', () => {
    Object.defineProperty(crypto, 'randomUUID', { value: undefined, configurable: true, writable: true })
    const ids = new Set(Array.from({ length: 1000 }, () => uuid()))
    expect(ids.size).toBe(1000)
    for (const id of ids) expect(id).toMatch(UUID_V4)
  })
})
